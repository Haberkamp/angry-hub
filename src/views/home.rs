use std::rc::Rc;
use std::time::Duration;

use gpui::prelude::*;
use gpui::{
    App, ClipboardItem, Context, Entity, EventEmitter, FontWeight,
    IntoElement, MouseButton, PromptButton, PromptLevel, Render, Transformation,     Window, div, point, px, rgb, size, svg,
};
use gpui_base::StyledExt as _;
use gpui_base::{VirtualListScrollHandle, v_virtual_list};

use crate::toast::Toaster;

use crate::auth::Auth;
use crate::color;
use crate::dropdown::{Dropdown, MenuItem};
use crate::tooltip::Tooltip;
use homestead::Table;

use crate::github::{Github, GithubClient, ReqwestHttp, GITHUB_CLIENT_ID};
use crate::icon;
use crate::keychain::CredentialStore;
use crate::pulls::{self, Event, PullRequest, SyncState, SYNC_ROW};
use crate::sync;

const INSET: f32 = 12.;
const ROW_HEIGHT: f32 = 64.;

pub struct LoggedOut;

const SYNC_EVERY: Duration = Duration::from_secs(10);

struct SyncJob {
    token: String,
    query: String,
    limit: usize,
}

pub struct Home {
    store: homestead::Store<Event>,
    pulls: homestead::Live<homestead::Select<PullRequest>>,
    rows: Vec<PullRequest>,
    scroll: VirtualListScrollHandle,
    menu_open: Option<String>,
    pending: Option<String>,
    toaster: Toaster,
}

impl Home {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let mut store = pulls::open().expect("open homestead");
        let pulls = store.watch(pulls::visible()).expect("watch pull requests");
        let home = Self {
            store,
            pulls,
            rows: Vec::new(),
            scroll: VirtualListScrollHandle::new(),
            menu_open: None,
            pending: None,
            toaster: Toaster::default(),
        };
        home.start_sync(cx);
        home.age_merged(cx);
        home
    }

    /// Re-run the visible query so a merged pull request drops off five minutes
    /// after it landed, even when sync has nothing new to write.
    fn age_merged(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_secs(30))
                    .await;
                let refreshed = this.update(cx, |this, cx| {
                    match this.store.watch(pulls::visible()) {
                        Ok(pulls) => {
                            this.pulls = pulls;
                            cx.notify();
                        }
                        Err(error) => eprintln!("failed to refresh pull requests: {error}"),
                    }
                });
                if refreshed.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    fn start_sync(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                let job = this.update(cx, |this, _| this.sync_job());
                let Ok(job) = job else {
                    break;
                };
                if let Some(job) = job {
                    let fetched = cx
                        .background_executor()
                        .spawn(async move {
                            let github = GithubClient::new(GITHUB_CLIENT_ID, ReqwestHttp);
                            sync::fetch_authored(&github, &job.token, &job.query, job.limit)
                        })
                        .await;
                    let applied = this.update(cx, |this, cx| {
                        match fetched {
                            Ok(fetched) => {
                                match sync::apply(
                                    &mut this.store,
                                    &fetched.pulls,
                                    fetched.complete,
                                ) {
                                    Ok(merged) => {
                                        for pull in merged {
                                            sync::notify_merged(&pull);
                                        }
                                    }
                                    Err(error) => {
                                        eprintln!("failed to store pull requests: {error}");
                                    }
                                }
                                cx.notify();
                            }
                            Err(error) => {
                                eprintln!("failed to fetch pull requests: {}", error.message());
                            }
                        }
                    });
                    if applied.is_err() {
                        break;
                    }
                }
                cx.background_executor().timer(SYNC_EVERY).await;
            }
        })
        .detach();
    }

    fn sync_job(&mut self) -> Option<SyncJob> {
        let token = Auth::load(&CredentialStore)
            .session()
            .access_token()
            .map(str::to_string)?;
        let watermark = self
            .store
            .watch(SyncState::where_eq("id", SYNC_ROW))
            .ok()?
            .rows()
            .into_iter()
            .next()
            .map(|state| state.watermark);
        Some(SyncJob {
            token,
            query: pulls::search_query(watermark.as_deref()),
            limit: pulls::INITIAL_LIMIT,
        })
    }

    fn toggle_menu(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.pending.is_some() {
            return;
        }
        self.menu_open = if self.menu_open.as_deref() == Some(id) {
            None
        } else {
            Some(id.to_string())
        };
        cx.notify();
    }

    fn copy_branch(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.pending.is_some() {
            return;
        }
        if let Some(branch) = self
            .rows
            .iter()
            .find(|pull| pull.id == id)
            .map(|pull| pull.branch.clone())
        {
            cx.write_to_clipboard(ClipboardItem::new_string(branch));
            self.toaster.push("copy-branch", "Copied branch");
            self.tick_toaster(cx);
        }
        self.menu_open = None;
        cx.notify();
    }

    fn tick_toaster(&mut self, cx: &mut Context<Self>) {
        if self.toaster.is_ticking() {
            return;
        }
        self.toaster.set_ticking(true);
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(32))
                    .await;
                let keep = this.update(cx, |this, cx| {
                    let keep = this.toaster.tick(std::time::Instant::now());
                    cx.notify();
                    keep
                });
                if keep.is_err() || !keep.unwrap_or(false) {
                    this.update(cx, |this, _| this.toaster.set_ticking(false))
                        .ok();
                    break;
                }
            }
        })
        .detach();
    }

    fn close_pr(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(pull) = self.rows.iter().find(|pull| pull.id == id).cloned() else {
            return;
        };
        self.run_pr_action(pull, true, window, cx);
    }

    fn toggle_draft(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(pull) = self.rows.iter().find(|pull| pull.id == id).cloned() else {
            return;
        };
        self.run_pr_action(pull, false, window, cx);
    }

    fn run_pr_action(
        &mut self,
        pull: PullRequest,
        close: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.pending.is_some() {
            return;
        }
        let draft = pull.state != "draft";
        self.pending = Some(pull.id.clone());
        self.menu_open = Some(pull.id.clone());
        cx.notify();
        let Some(token) = Auth::load(&CredentialStore)
            .session()
            .access_token()
            .map(str::to_string)
        else {
            self.finish_action(cx);
            return;
        };
        let url = pull.url.clone();
        cx.spawn_in(window, async move |this, cx| {
            let probe_token = token.clone();
            let repo = pull.repository.clone();
            let restricted = cx
                .background_executor()
                .spawn(async move {
                    let github = GithubClient::new(GITHUB_CLIENT_ID, ReqwestHttp);
                    github
                        .oauth_app_restricted(&probe_token, &repo)
                        .unwrap_or(false)
                })
                .await;
            if restricted {
                this.update_in(cx, |this, window, cx| {
                    this.finish_action(cx);
                    prompt_oauth_restricted(url, close, draft, window, cx);
                })
                .ok();
                return;
            }

            let action_token = token.clone();
            let action_id = pull.id.clone();
            let result = cx
                .background_executor()
                .spawn(async move {
                    let github = GithubClient::new(GITHUB_CLIENT_ID, ReqwestHttp);
                    if close {
                        github.close_pull_request(&action_token, &action_id)
                    } else {
                        github.set_pull_request_draft(&action_token, &action_id, draft)
                    }
                })
                .await;
            match result {
                Ok(()) => {
                    this.update(cx, |this, cx| {
                        let mut updated = pull.clone();
                        updated.state = if close {
                            "closed".into()
                        } else if draft {
                            "draft".into()
                        } else {
                            "open".into()
                        };
                        let event = if close {
                            Event::PrClosed(updated)
                        } else {
                            Event::PrUpdated(updated)
                        };
                        if let Err(error) = this.store.commit(event) {
                            eprintln!("failed to store pull request: {error}");
                        }
                        this.finish_action(cx);
                    })
                    .ok();
                }
                Err(error) if error.is_oauth_app_restricted() => {
                    this.update_in(cx, |this, window, cx| {
                        this.finish_action(cx);
                        prompt_oauth_restricted(url, close, draft, window, cx);
                    })
                    .ok();
                }
                Err(error) if error.is_session_ended() => {
                    this.update_in(cx, |this, window, cx| {
                        this.finish_action(cx);
                        this.logout(window, cx);
                    })
                    .ok();
                }
                Err(error) => {
                    eprintln!("failed to update pull request: {}", error.message());
                    this.update(cx, |this, cx| this.finish_action(cx)).ok();
                }
            }
        })
        .detach();
    }

    fn finish_action(&mut self, cx: &mut Context<Self>) {
        self.pending = None;
        self.menu_open = None;
        cx.notify();
    }

    fn logout(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let mut auth = Auth::load(&CredentialStore);
        auth.logout(&CredentialStore);
        cx.emit(LoggedOut);
    }

}

impl EventEmitter<LoggedOut> for Home {}

pub fn logout_button(home: Entity<Home>) -> impl IntoElement {
    div()
        .id("logout")
        .group("logout")
        .absolute()
        .top(px(INSET))
        .right(px(INSET))
        .size(px(40.))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .text_color(rgb(0x6E6E6E))
        .hover(|style| style.bg(rgb(0x222222)).text_color(rgb(0xffffff)))
        .active(|style| style.bg(rgb(0x313131)))
        .on_mouse_down(MouseButton::Left, |_, _, cx| {
            cx.stop_propagation();
        })
        .on_click(move |_, window, cx| {
            let answer = window.prompt(
                PromptLevel::Warning,
                "Log out?",
                Some("Are you sure you want to log out?"),
                &[
                    PromptButton::ok("Log out"),
                    PromptButton::cancel("Cancel"),
                ],
                cx,
            );
            home.update(cx, |_, cx| {
                cx.spawn_in(window, async move |this, cx| {
                    if answer.await == Ok(0) {
                        this.update_in(cx, |this, window, cx| this.logout(window, cx))
                            .ok();
                    }
                })
                .detach();
            });
        })
        .child(
            svg()
                .data(icon::LOGOUT)
                .size(px(16.))
                .with_transformation(Transformation::translate(point(px(-1.), px(0.))))
                .text_color(rgb(0x6E6E6E))
                .group_hover("logout", |style| style.text_color(rgb(0xffffff))),
        )
}

fn status_icon(state: &str) -> &'static [u8] {
    match state {
        "draft" => icon::PR_DRAFT,
        "closed" => icon::PR_CLOSED,
        _ => icon::PULL_REQUEST,
    }
}

fn status_color(state: &str, cx: &App) -> gpui::Hsla {
    match state {
        "draft" => color::status_draft(cx),
        "closed" => color::status_closed(cx),
        "merged" => color::status_merged(cx),
        _ => color::status_open(cx),
    }
}

fn ci_icon(ci: &str) -> Option<(&'static [u8], &'static str)> {
    match ci {
        "success" => Some((icon::CHECK, "CI passed")),
        "failure" => Some((icon::X, "CI failed")),
        "pending" => Some((icon::PENDING, "CI pending")),
        _ => None,
    }
}

fn ci_color(ci: &str, cx: &App) -> gpui::Hsla {
    match ci {
        "failure" => color::status_closed(cx),
        "pending" => color::status_pending(cx),
        _ => color::status_open(cx),
    }
}

fn prompt_oauth_restricted(
    url: String,
    close: bool,
    draft: bool,
    window: &mut Window,
    cx: &mut Context<Home>,
) {
    let (message, detail) = if close {
        (
            "This organization restricts OAuth apps, so Angry Hub can't close the pull request.",
            "Open it on GitHub and close it in the browser?",
        )
    } else if draft {
        (
            "This organization restricts OAuth apps, so Angry Hub can't convert the pull request to a draft.",
            "Open it on GitHub and convert it to a draft in the browser?",
        )
    } else {
        (
            "This organization restricts OAuth apps, so Angry Hub can't mark the pull request as ready for review.",
            "Open it on GitHub and mark it ready for review in the browser?",
        )
    };
    let answer = window.prompt(
        PromptLevel::Warning,
        message,
        Some(detail),
        &[
            PromptButton::ok("Open on GitHub"),
            PromptButton::cancel("Cancel"),
        ],
        cx,
    );
    cx.spawn(async move |this, cx| {
        if answer.await == Ok(0) {
            this.update(cx, |_, cx| cx.open_url(&url)).ok();
        }
    })
    .detach();
}

fn pull_row(pull: PullRequest, home: &Home, cx: &mut Context<Home>) -> impl IntoElement {
    let url = pull.url.clone();
    let meta = color::gray(11, cx);
    let show_approvals = pull.required_approvals > 0;
    let ci = ci_icon(&pull.ci);
    let menu_id = pull.id.clone();
    let pending = home.pending.as_deref() == Some(pull.id.as_str());
    let open = home.menu_open.as_deref() == Some(pull.id.as_str());
    let draft_label = if pull.state == "draft" {
        "Ready for review"
    } else {
        "Convert to draft"
    };
    div()
        .id(gpui::ElementId::Name(pull.id.clone().into()))
        .group("pr-row")
        .w_full()
        .flex()
        .items_center()
        .gap(px(8.))
        .py(px(8.))
        .px(px(12.))
        .rounded(px(6.))
        .hover(|style| style.bg(rgb(0x222222)))
        .child(
            div()
                .id(gpui::ElementId::Name(format!("open-{}", pull.id).into()))
                .flex_1()
                .min_w_0()
                .v_flex()
                .gap(px(4.))
                .on_click(move |_, _, cx| cx.open_url(&url))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .min_w_0()
                        .child(
                            svg()
                                .data(status_icon(&pull.state))
                                .size(px(20.))
                                .flex_none()
                                .text_color(status_color(&pull.state, cx)),
                        )
                        .child(
                            div()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_color(rgb(0xffffff))
                                .child(pull.title.clone()),
                        )
                        .when(pull.has_conflicts, |row| {
                            row.child(
                                Tooltip::new(format!("pr-conflicts-{}", pull.id), "Merge conflicts")
                                    .child(
                                        svg()
                                            .data(icon::MERGE_CONFLICTS)
                                            .size(px(16.))
                                            .flex_none()
                                            .text_color(color::status_pending(cx)),
                                    ),
                            )
                        }),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .ml(px(28.))
                        .text_size(px(12.))
                        .text_color(meta)
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(4.))
                                .child(pull.repository.clone())
                                .child(format!("#{}", pull.number))
                                .when(show_approvals, |meta| {
                                    meta.child("·").child("Approvals").child(format!(
                                        "{}/{}",
                                        pull.approvals, pull.required_approvals
                                    ))
                                }),
                        )
                        .children(ci.map(|(data, label)| {
                            Tooltip::new(format!("pr-ci-{}", pull.id), label).child(
                                svg()
                                    .data(data)
                                    .size(px(14.))
                                    .flex_none()
                                    .text_color(ci_color(&pull.ci, cx)),
                            )
                        })),
                ),
        )
        .child({
            let entity = cx.entity();
            Dropdown::new(format!("pr-menu-{menu_id}"))
                .open(open)
                .disabled(pending)
                .items(vec![
                    MenuItem::new("copy-branch", "Copy branch name"),
                    MenuItem::new("toggle-draft", draft_label),
                    MenuItem::new("close", "Close pull request"),
                ])
                .on_toggle({
                    let entity = entity.clone();
                    let menu_id = menu_id.clone();
                    move |_, cx| {
                        entity.update(cx, |this, cx| this.toggle_menu(&menu_id, cx));
                    }
                })
                .on_dismiss({
                    let entity = entity.clone();
                    move |_, cx| {
                        entity.update(cx, |this, cx| {
                            if this.pending.is_none() {
                                this.menu_open = None;
                                cx.notify();
                            }
                        });
                    }
                })
                .on_select({
                    let entity = entity.clone();
                    move |item, window, cx| {
                        let item = item.to_string();
                        let menu_id = menu_id.clone();
                        entity.update(cx, |this, cx| match item.as_str() {
                            "copy-branch" => this.copy_branch(&menu_id, cx),
                            "toggle-draft" => this.toggle_draft(menu_id, window, cx),
                            "close" => this.close_pr(menu_id, window, cx),
                            _ => {}
                        });
                    }
                })
        })
}

impl Render for Home {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.rows = self.pulls.rows();
        let empty = self.rows.is_empty();
        let count = self.rows.len();
        let sizes = Rc::new(vec![size(px(560.), px(ROW_HEIGHT)); count]);
        div()
            .relative()
            .flex_1()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .pt(px(96.))
                    .px(px(24.))
                    .pb(px(16.))
                    .child(
                        div()
                            .w_full()
                            .max_w(px(560.))
                            .mx_auto()
                            .text_3xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0xffffff))
                            .child("Pull Requests"),
                    ),
            )
            .child(if empty {
                div()
                    .w_full()
                    .px(px(24.))
                    .child(
                        div()
                            .w_full()
                            .max_w(px(560.))
                            .mx_auto()
                            .text_color(rgb(0x6E6E6E))
                            .child("No pull requests yet"),
                    )
                    .into_any_element()
            } else {
                v_virtual_list(
                    cx.entity(),
                    "pulls",
                    sizes,
                    |home, range, _, cx| {
                        range
                            .filter_map(|index| home.rows.get(index).cloned())
                            .map(|pull| {
                                div()
                                    .w_full()
                                    .h(px(ROW_HEIGHT))
                                    .px(px(24.))
                                    .child(
                                        div()
                                            .w_full()
                                            .max_w(px(560.))
                                            .mx_auto()
                                            .child(pull_row(pull, home, cx)),
                                    )
                            })
                            .collect()
                    },
                )
                .track_scroll(&self.scroll)
                .flex_1()
                .pb(px(24.))
                .into_any_element()
            })
            .child(self.toaster.render(cx))
    }
}
