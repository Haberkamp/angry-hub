use std::time::Duration;

use gpui::prelude::*;
use gpui::{
    App, Context, Entity, EventEmitter, IntoElement, MouseButton, PromptButton, PromptLevel,
    Render, Transformation, Window, div, point, px, rgb, svg,
};
use gpui_base::StyledExt as _;

use crate::auth::Auth;
use crate::color;
use crate::tooltip::Tooltip;
use homestead::Table;

use crate::github::{GithubClient, ReqwestHttp, GITHUB_CLIENT_ID};
use crate::icon;
use crate::keychain::KeychainStore;
use crate::pulls::{self, Event, PullRequest, SyncState, SYNC_ROW};
use crate::sync;

const INSET: f32 = 12.;

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
}

impl Home {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let mut store = pulls::open().expect("open homestead");
        let pulls = store
            .watch(PullRequest::query().order_by_desc("updated_at"))
            .expect("watch pull requests");
        let home = Self { store, pulls };
        home.start_sync(cx);
        home
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
                                if let Err(error) = sync::apply(
                                    &mut this.store,
                                    &fetched.pulls,
                                    fetched.complete,
                                ) {
                                    eprintln!("failed to store pull requests: {error}");
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
        let token = Auth::load(&KeychainStore)
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

    fn logout(&mut self, cx: &mut Context<Self>) {
        let mut auth = Auth::load(&KeychainStore);
        auth.logout(&KeychainStore);
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
                cx.spawn(async move |this, cx| {
                    if answer.await == Ok(0) {
                        this.update(cx, |this, cx| this.logout(cx)).ok();
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

fn grouped(mut pulls: Vec<PullRequest>) -> Vec<PullRequest> {
    pulls.sort_by(|left, right| {
        group_rank(&left.state)
            .cmp(&group_rank(&right.state))
            .then_with(|| right.updated_at.cmp(&left.updated_at))
    });
    pulls
}

fn group_rank(state: &str) -> u8 {
    match state {
        "open" => 0,
        "draft" => 1,
        _ => 2,
    }
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

fn pull_row(pull: PullRequest, cx: &App) -> impl IntoElement {
    let url = pull.url.clone();
    let meta = color::gray(11, cx);
    let show_approvals = pull.required_approvals > 0;
    let ci = ci_icon(&pull.ci);
    div()
        .id(gpui::ElementId::Name(pull.id.clone().into()))
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
}

impl Render for Home {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pulls = self.pulls.rows();
        let empty = pulls.is_empty();
        div()
            .id("pulls")
            .flex_1()
            .size_full()
            .overflow_y_scroll()
            .pt(px(56.))
            .px(px(24.))
            .pb(px(24.))
            .child(
                div()
                    .w_full()
                    .max_w(px(560.))
                    .mx_auto()
                    .v_flex()
                    .gap(px(8.))
                    .children(if empty {
                        vec![div()
                            .text_color(rgb(0x6E6E6E))
                            .child("No pull requests yet")
                            .into_any_element()]
                    } else {
                        grouped(pulls)
                            .into_iter()
                            .map(|pull| pull_row(pull, cx).into_any_element())
                            .collect()
                    }),
            )
    }
}
