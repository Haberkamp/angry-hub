use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    AnyElement, ClipboardItem, Context, Entity, PromptLevel, Render, SharedString, Subscription,
    Timer, Window, div, prelude::*,
};

use crate::code_host::code_host;
use crate::json_file;
use crate::layout::Chrome;
use crate::models;
use crate::session::{self, Session};
use crate::ui::color;
use crate::ui::{Button, MultiSelect, Notification, PrItem, SelectOption, Spinner, Tab, WindowExt};

enum PrsState {
    Loading,
    Loaded(Vec<models::PullRequest>),
    Failed(Arc<str>),
}

pub struct PullRequests {
    prs: PrsState,
    selected_repo: Option<Arc<str>>,
    visible_tab_repos: Option<HashSet<String>>,
    visibility_menu_open: bool,
    pr_menu_open: Option<String>,
    closing_pr: Option<String>,
    toggling_draft: Option<String>,
    refreshing: bool,
    loading: bool,
    fetch_in_flight: bool,
    chrome: Entity<Chrome>,
    _activation_subscription: Option<Subscription>,
}

fn load_visible_tab_repos() -> Option<HashSet<String>> {
    if let Some(repos) = json_file::load::<Vec<String>>("visible_tab_repos.json") {
        return Some(repos.into_iter().collect());
    }
    #[derive(serde::Deserialize)]
    struct LegacyPrefs {
        visible_tab_repos: Option<Vec<String>>,
    }
    json_file::load::<LegacyPrefs>("prefs.json")
        .and_then(|stored| stored.visible_tab_repos)
        .map(|repos| repos.into_iter().collect())
}

impl PullRequests {
    pub fn new(chrome: Entity<Chrome>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let prs = if cx.global::<Session>().logged_in {
            match code_host().pull_request_snapshot() {
                Some(prs) => PrsState::Loaded(prs),
                None => PrsState::Loading,
            }
        } else {
            PrsState::Loading
        };
        let mut view = Self {
            prs,
            selected_repo: Some("all".into()),
            visible_tab_repos: load_visible_tab_repos(),
            visibility_menu_open: false,
            pr_menu_open: None,
            closing_pr: None,
            toggling_draft: None,
            refreshing: false,
            loading: false,
            fetch_in_flight: false,
            chrome,
            _activation_subscription: None,
        };
        view._activation_subscription =
            Some(cx.observe_window_activation(window, |this, window, cx| {
                if window.is_window_active() && cx.global::<Session>().logged_in {
                    this.refresh_prs(window, cx);
                }
            }));
        if cx.global::<Session>().logged_in {
            view.loading = matches!(view.prs, PrsState::Loading);
            view.fetch_prs(window, cx);
        }
        view.poll(window, cx);
        view
    }

    fn poll(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.spawn_in(window, async move |this, cx| {
            loop {
                Timer::after(Duration::from_secs(30)).await;
                this.update_in(cx, |this, window, cx| {
                    if cx.global::<Session>().logged_in {
                        this.fetch_prs(window, cx);
                    }
                })
                .ok();
            }
        })
        .detach();
    }

    fn show_refresh_indicator(&self) -> bool {
        self.refreshing && !self.loading
    }

    fn sync_refresh_indicator(&mut self, cx: &mut Context<Self>) {
        let show = self.show_refresh_indicator();
        self.chrome
            .update(cx, |chrome, cx| chrome.set_refreshing(show, cx));
    }

    pub(crate) fn refresh_prs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.refreshing = true;
        self.sync_refresh_indicator(cx);
        self.fetch_prs(window, cx);
    }

    fn fetch_prs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.fetch_in_flight || !cx.global::<Session>().logged_in {
            return;
        }
        self.fetch_in_flight = true;
        let host = code_host();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { host.my_pull_requests() })
                .await;

            this.update_in(cx, |this, window, cx| {
                this.fetch_in_flight = false;
                this.refreshing = false;
                this.loading = false;
                match result {
                    Ok(prs) => this.prs = PrsState::Loaded(prs),
                    Err(e) if e.is_session_ended() => {
                        session::force_logout(window, cx);
                    }
                    Err(e) => this.prs = PrsState::Failed(e.message.into()),
                }
                this.sync_refresh_indicator(cx);
            })
            .ok();
            this.update(cx, |_, cx| cx.notify()).ok();
        })
        .detach();
    }

    fn repo_tab_visible(&self, repo: &str) -> bool {
        self.visible_tab_repos
            .as_ref()
            .map(|visible| visible.contains(repo))
            .unwrap_or(true)
    }

    fn toggle_repo_tab_visibility(
        &mut self,
        repo: &str,
        all_repos: &[Arc<str>],
        cx: &mut Context<Self>,
    ) {
        let mut visible = self
            .visible_tab_repos
            .clone()
            .unwrap_or_else(|| all_repos.iter().map(|repo| repo.to_string()).collect());
        if !visible.remove(repo) {
            visible.insert(repo.to_string());
        }
        if self.selected_repo.as_deref() == Some(repo) {
            self.selected_repo = Some("all".into());
        }
        json_file::save("visible_tab_repos.json", &{
            let mut repos: Vec<String> = visible.iter().cloned().collect();
            repos.sort();
            repos
        });
        self.visible_tab_repos = Some(visible);
        cx.notify();
    }

    fn pr_action_in_flight(&self) -> bool {
        self.closing_pr.is_some() || self.toggling_draft.is_some()
    }

    fn prompt_action_on_github(
        &mut self,
        url: String,
        message: &'static str,
        detail: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let answer = window.prompt(
            PromptLevel::Warning,
            message,
            Some(detail),
            &["Open on GitHub", "Cancel"],
            cx,
        );
        cx.spawn(async move |this, cx| {
            if answer.await == Ok(0) {
                this.update(cx, |_, cx| cx.open_url(&url)).ok();
            }
        })
        .detach();
    }

    fn prompt_close_on_github(&mut self, url: String, window: &mut Window, cx: &mut Context<Self>) {
        self.prompt_action_on_github(
            url,
            "This organization restricts OAuth apps, so Angry Hub can't close the pull request.",
            "Open it on GitHub and close it in the browser?",
            window,
            cx,
        );
    }

    fn prompt_draft_on_github(
        &mut self,
        url: String,
        draft: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (message, detail) = if draft {
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
        self.prompt_action_on_github(url, message, detail, window, cx);
    }

    fn finish_pr_action(&mut self, cx: &mut Context<Self>) {
        self.closing_pr = None;
        self.toggling_draft = None;
        self.pr_menu_open = None;
        cx.notify();
    }

    fn finish_close_pr(&mut self, cx: &mut Context<Self>) {
        self.finish_pr_action(cx);
    }

    fn close_pr(
        &mut self,
        id: String,
        url: String,
        repo: String,
        menu_key: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.pr_action_in_flight() {
            return;
        }
        self.pr_menu_open = Some(menu_key.clone());
        self.closing_pr = Some(menu_key);
        cx.notify();
        if id.is_empty() {
            self.finish_close_pr(cx);
            self.prompt_close_on_github(url, window, cx);
            return;
        }

        let host = code_host();
        cx.spawn_in(window, async move |this, cx| {
            let probe_host = host.clone();
            let repo_for_probe = repo.clone();
            let restricted = cx
                .background_executor()
                .spawn(async move {
                    probe_host
                        .oauth_app_restricted_from_repo(&repo_for_probe)
                        .unwrap_or(false)
                })
                .await;

            if restricted {
                this.update_in(cx, |this, window, cx| {
                    this.finish_close_pr(cx);
                    this.prompt_close_on_github(url, window, cx);
                })
                .ok();
                return;
            }

            let close_host = host.clone();
            let close_id = id.clone();
            let result = cx
                .background_executor()
                .spawn(async move { close_host.close_pull_request(&close_id) })
                .await;
            match result {
                Ok(()) => {
                    this.update(cx, |this, cx| {
                        if let PrsState::Loaded(prs) = &mut this.prs {
                            prs.retain(|pr| pr.id != id);
                        }
                        this.finish_close_pr(cx);
                    })
                    .ok();
                }
                Err(e) if e.is_oauth_app_restricted() => {
                    this.update_in(cx, |this, window, cx| {
                        this.finish_close_pr(cx);
                        this.prompt_close_on_github(url, window, cx);
                    })
                    .ok();
                }
                Err(e) if e.is_session_ended() => {
                    this.update_in(cx, |this, window, cx| {
                        this.finish_close_pr(cx);
                        session::force_logout(window, cx);
                    })
                    .ok();
                }
                Err(e) => {
                    eprintln!("failed to close pull request: {e}");
                    this.update_in(cx, |this, window, cx| {
                        this.finish_close_pr(cx);
                        this.fetch_prs(window, cx);
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    fn set_pr_draft(
        &mut self,
        id: String,
        url: String,
        repo: String,
        menu_key: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let draft = match &self.prs {
            PrsState::Loaded(prs) => prs
                .iter()
                .find(|pr| pr.id == id)
                .is_none_or(|pr| pr.status != models::PrStatus::Draft),
            _ => true,
        };
        if self.pr_action_in_flight() {
            return;
        }
        self.pr_menu_open = Some(menu_key.clone());
        self.toggling_draft = Some(menu_key);
        cx.notify();
        if id.is_empty() {
            self.finish_pr_action(cx);
            self.prompt_draft_on_github(url, draft, window, cx);
            return;
        }

        let host = code_host();
        cx.spawn_in(window, async move |this, cx| {
            let probe_host = host.clone();
            let repo_for_probe = repo.clone();
            let restricted = cx
                .background_executor()
                .spawn(async move {
                    probe_host
                        .oauth_app_restricted_from_repo(&repo_for_probe)
                        .unwrap_or(false)
                })
                .await;

            if restricted {
                this.update_in(cx, |this, window, cx| {
                    this.finish_pr_action(cx);
                    this.prompt_draft_on_github(url, draft, window, cx);
                })
                .ok();
                return;
            }

            let draft_host = host.clone();
            let draft_id = id.clone();
            let result = cx
                .background_executor()
                .spawn(async move { draft_host.set_pull_request_draft(&draft_id, draft) })
                .await;
            match result {
                Ok(()) => {
                    this.update(cx, |this, cx| {
                        if let PrsState::Loaded(prs) = &mut this.prs
                            && let Some(pr) = prs.iter_mut().find(|pr| pr.id == id)
                        {
                            pr.status = if draft {
                                models::PrStatus::Draft
                            } else {
                                models::PrStatus::Open
                            };
                        }
                        this.finish_pr_action(cx);
                    })
                    .ok();
                }
                Err(e) if e.is_oauth_app_restricted() => {
                    this.update_in(cx, |this, window, cx| {
                        this.finish_pr_action(cx);
                        this.prompt_draft_on_github(url, draft, window, cx);
                    })
                    .ok();
                }
                Err(e) if e.is_session_ended() => {
                    this.update_in(cx, |this, window, cx| {
                        this.finish_pr_action(cx);
                        session::force_logout(window, cx);
                    })
                    .ok();
                }
                Err(e) => {
                    eprintln!("failed to set pull request draft: {e}");
                    this.update_in(cx, |this, window, cx| {
                        this.finish_pr_action(cx);
                        this.fetch_prs(window, cx);
                    })
                    .ok();
                }
            }
        })
        .detach();
    }
}

impl Render for PullRequests {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !cx.global::<Session>().logged_in {
            self.prs = PrsState::Loading;
            self.fetch_in_flight = false;
            self.refreshing = false;
            self.loading = false;
            self.sync_refresh_indicator(cx);
        } else if matches!(self.prs, PrsState::Loading) && !self.fetch_in_flight {
            self.loading = true;
            self.sync_refresh_indicator(cx);
            self.fetch_prs(window, cx);
        }

        let mut repo_tabs: Vec<AnyElement> = Vec::new();
        let content: Vec<AnyElement> = match &self.prs {
            PrsState::Loading => vec![
                div()
                    .flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .child(Spinner::new("prs-loading"))
                    .into_any_element(),
            ],
            PrsState::Failed(e) => vec![
                div()
                    .text_color(color::text::danger(cx))
                    .child(e.to_string())
                    .into_any_element(),
                Button::new("retry-prs", "Retry")
                    .on_click(cx.listener(|this, _, window, cx| this.refresh_prs(window, cx)))
                    .into_any_element(),
            ],
            PrsState::Loaded(prs) => {
                let active: Vec<&models::PullRequest> = prs.iter().collect();

                let repos: Vec<Arc<str>> = {
                    let mut seen: Vec<Arc<str>> = Vec::new();
                    for pr in &active {
                        let repo: Arc<str> = pr.repo.clone().into();
                        if !seen.contains(&repo) {
                            seen.push(repo);
                        }
                    }
                    seen
                };

                let tab_repos: Vec<Arc<str>> = repos
                    .iter()
                    .filter(|repo| self.repo_tab_visible(repo))
                    .cloned()
                    .collect();

                let selected = self
                    .selected_repo
                    .clone()
                    .filter(|r| tab_repos.contains(r) || r.as_ref() == "all")
                    .or_else(|| Some("all".into()));

                let mut visible: Vec<&models::PullRequest> = active
                    .iter()
                    .copied()
                    .filter(|pr| self.repo_tab_visible(&pr.repo))
                    .filter(|pr| match selected.as_deref() {
                        Some("all") | None => true,
                        Some(repo) => repo == pr.repo.as_str(),
                    })
                    .collect();
                visible.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

                if !repos.is_empty() {
                    repo_tabs.push(
                        MultiSelect::new("repo-visibility")
                            .open(self.visibility_menu_open)
                            .options(
                                repos
                                    .iter()
                                    .map(|repo| {
                                        SelectOption::new(
                                            repo.to_string(),
                                            repo.to_string(),
                                            self.repo_tab_visible(repo),
                                        )
                                    })
                                    .collect(),
                            )
                            .on_toggle_open(cx.listener(|this, _, _, cx| {
                                this.visibility_menu_open = !this.visibility_menu_open;
                                this.pr_menu_open = None;
                                cx.notify();
                            }))
                            .on_dismiss(cx.listener(|this, _, _, cx| {
                                this.visibility_menu_open = false;
                                cx.notify();
                            }))
                            .on_toggle_option(cx.listener({
                                let repos = repos.clone();
                                move |this, repo: &str, _, cx| {
                                    this.toggle_repo_tab_visibility(repo, &repos, cx);
                                }
                            }))
                            .into_any_element(),
                    );
                }
                repo_tabs.push(
                    Tab::new("repo-tab-all", "All")
                        .selected(selected.as_deref() == Some("all"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.selected_repo = Some("all".into());
                            this.visibility_menu_open = false;
                            cx.notify();
                        }))
                        .into_any_element(),
                );
                repo_tabs.extend(tab_repos.iter().enumerate().map(|(ix, repo)| {
                    Tab::new(
                        SharedString::from(format!("repo-tab-{ix}")),
                        repo.to_string(),
                    )
                    .selected(Some(repo.as_ref()) == selected.as_deref())
                    .on_click(cx.listener({
                        let repo = repo.clone();
                        move |this, _, _, cx| {
                            this.selected_repo = Some(repo.clone());
                            this.visibility_menu_open = false;
                            cx.notify();
                        }
                    }))
                    .into_any_element()
                }));

                if visible.is_empty() {
                    vec![div().child("No PRs found").into_any_element()]
                } else {
                    visible
                        .iter()
                        .enumerate()
                        .map(|(ix, pr)| {
                            let menu_key = if pr.id.is_empty() {
                                pr.url.clone()
                            } else {
                                pr.id.clone()
                            };
                            let close_id = pr.id.clone();
                            PrItem::new(ix, pr)
                                .menu_open(self.pr_menu_open.as_deref() == Some(&menu_key))
                                .closing(self.closing_pr.as_deref() == Some(&menu_key))
                                .toggling_draft(self.toggling_draft.as_deref() == Some(&menu_key))
                                .on_toggle_menu(cx.listener({
                                    let menu_key = menu_key.clone();
                                    move |this, _, _, cx| {
                                        if this.pr_action_in_flight() {
                                            return;
                                        }
                                        if this.pr_menu_open.as_deref() == Some(&menu_key) {
                                            this.pr_menu_open = None;
                                        } else {
                                            this.pr_menu_open = Some(menu_key.clone());
                                            this.visibility_menu_open = false;
                                        }
                                        cx.notify();
                                    }
                                }))
                                .on_dismiss_menu(cx.listener(|this, _, _, cx| {
                                    if this.pr_action_in_flight() {
                                        return;
                                    }
                                    this.pr_menu_open = None;
                                    cx.notify();
                                }))
                                .on_copy_branch({
                                    let branch = pr.branch.clone();
                                    let entity = cx.entity();
                                    move |window, app| {
                                        entity.update(app, |this, cx| {
                                            cx.write_to_clipboard(ClipboardItem::new_string(
                                                branch.clone(),
                                            ));
                                            this.pr_menu_open = None;
                                            cx.notify();
                                        });
                                        window.push_notification(
                                            Notification::new()
                                                .key("copy-branch")
                                                .message("Copied branch name"),
                                            app,
                                        );
                                    }
                                })
                                .on_close_pr({
                                    let close_id = close_id.clone();
                                    let url = pr.url.clone();
                                    let repo = pr.repo.clone();
                                    let menu_key = menu_key.clone();
                                    let entity = cx.entity();
                                    move |window, app| {
                                        entity.update(app, |this, cx| {
                                            this.close_pr(
                                                close_id.clone(),
                                                url.clone(),
                                                repo.clone(),
                                                menu_key.clone(),
                                                window,
                                                cx,
                                            );
                                        });
                                    }
                                })
                                .on_toggle_draft({
                                    let close_id = close_id.clone();
                                    let url = pr.url.clone();
                                    let repo = pr.repo.clone();
                                    let menu_key = menu_key.clone();
                                    let entity = cx.entity();
                                    move |window, app| {
                                        entity.update(app, |this, cx| {
                                            this.set_pr_draft(
                                                close_id.clone(),
                                                url.clone(),
                                                repo.clone(),
                                                menu_key.clone(),
                                                window,
                                                cx,
                                            );
                                        });
                                    }
                                })
                                .into_any_element()
                        })
                        .collect()
                }
            }
        };

        div()
            .id("prs-page")
            .flex()
            .flex_col()
            .when(matches!(self.prs, PrsState::Loading), |this| this.flex_1())
            .when(!repo_tabs.is_empty(), |this| {
                let opacity = if window.is_window_active() {
                    1.0
                } else {
                    crate::layout::INACTIVE_HEADER_OPACITY
                };
                this.child(
                    div()
                        .id("repo-tabs")
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .pl_3()
                        .pb_4()
                        .opacity(opacity)
                        .children(repo_tabs),
                )
            })
            .children(content)
    }
}
