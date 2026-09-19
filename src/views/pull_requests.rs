use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    AnyElement, Context, Entity, IntoElement, ParentElement, PromptLevel, Render, SharedString,
    Styled, Subscription, Timer, Window, div, prelude::*, rgb,
};

use crate::code_host;
use crate::github;
use crate::layout::main_layout;
use crate::model;
use crate::prefs::Prefs;
use crate::route::{PullRequestsPage, Route};
use crate::router::Router;
use crate::session::Session;
use crate::show_desktop_notification;
use crate::ui::{Button, MultiSelect, PrItem, SelectOption, Spinner, Tab};

#[derive(Clone)]
enum PrsState {
    Loading,
    Loaded(Vec<model::PullRequest>),
    Failed(Arc<str>),
}

pub struct PullRequestsView {
    prs: PrsState,
    visible_tab_repos: Option<HashSet<String>>,
    visibility_menu_open: bool,
    pr_menu_open: Option<String>,
    closing_pr: Option<String>,
    pub refreshing: bool,
    fetch_in_flight: bool,
    session: Entity<Session>,
    router: Entity<Router<Route>>,
    _subscriptions: Vec<Subscription>,
}

impl PullRequestsView {
    pub fn new(
        session: Entity<Session>,
        router: Entity<Router<Route>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let initial = match github::PrsCache::load() {
            Some(prs) if !prs.is_empty() => PrsState::Loaded(prs),
            _ => PrsState::Loading,
        };
        let mut _subscriptions = Vec::new();
        _subscriptions.push(cx.observe(&router, |this, _, cx| {
            this.dismiss_overlays();
            cx.notify();
        }));
        _subscriptions.push(cx.observe(&session, |this, session, cx| {
            if session.read(cx).logged_in {
                this.fetch(cx);
            }
        }));
        _subscriptions.push(cx.observe_window_activation(window, |this, window, cx| {
            if window.is_window_active() && this.session.read(cx).logged_in {
                this.refresh(cx);
            }
        }));
        let mut view = Self {
            prs: initial,
            visible_tab_repos: Prefs::load_visible_tab_repos(),
            visibility_menu_open: false,
            pr_menu_open: None,
            closing_pr: None,
            refreshing: false,
            fetch_in_flight: false,
            session,
            router,
            _subscriptions,
        };
        view.poll(cx);
        if view.session.read(cx).logged_in {
            view.fetch(cx);
        }
        view
    }

    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self.refreshing = true;
        self.fetch(cx);
    }

    fn dismiss_overlays(&mut self) {
        self.visibility_menu_open = false;
        self.pr_menu_open = None;
        self.closing_pr = None;
    }

    fn poll(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                Timer::after(Duration::from_secs(30)).await;
                this.update(cx, |this, cx| {
                    if this.session.read(cx).logged_in {
                        this.fetch(cx);
                    }
                })
                .ok();
            }
        })
        .detach();
    }

    fn fetch(&mut self, cx: &mut Context<Self>) {
        if self.fetch_in_flight {
            return;
        }
        self.fetch_in_flight = true;
        let previous_urls: Option<HashSet<String>> = if let PrsState::Loaded(prs) = &self.prs {
            Some(prs.iter().map(|pr| pr.url.clone()).collect())
        } else {
            None
        };
        let host = code_host();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn({
                    let host = host.clone();
                    async move { host.my_pull_requests() }
                })
                .await;

            let merged = match (&result, previous_urls) {
                (Ok(current), Some(previous)) => {
                    let disappeared: Vec<String> = previous
                        .into_iter()
                        .filter(|url| !current.iter().any(|pr| &pr.url == url))
                        .collect();
                    if disappeared.is_empty() {
                        Vec::new()
                    } else {
                        cx.background_executor()
                            .spawn(async move {
                                host.merged_pull_requests(&disappeared).unwrap_or_default()
                            })
                            .await
                    }
                }
                _ => Vec::new(),
            };

            this.update(cx, |this, _cx| {
                this.fetch_in_flight = false;
                this.refreshing = false;
                this.prs = match result {
                    Ok(prs) => PrsState::Loaded(prs),
                    Err(e) => PrsState::Failed(e.message.into()),
                };
            })
            .ok();
            this.update(cx, |_, cx| cx.notify()).ok();

            for pr in merged {
                show_desktop_notification(
                    "Pull request merged",
                    format!("{} · {}", pr.title, pr.repo),
                );
            }
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
        let logged_in = self.session.read(cx).logged_in;
        if self.router.read(cx).current().selected_repo() == Some(repo) {
            self.router.update(cx, |router, cx| {
                router.replace(Route::pull_requests(), Session::allow(logged_in), cx);
            });
        }
        Prefs::save_visible_tab_repos(&visible);
        self.visible_tab_repos = Some(visible);
        cx.notify();
    }

    fn prompt_close_on_github(&mut self, url: String, window: &mut Window, cx: &mut Context<Self>) {
        let answer = window.prompt(
            PromptLevel::Warning,
            "This organization restricts OAuth apps, so Angry Hub can't close the pull request.",
            Some("Open it on GitHub and close it in the browser?"),
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

    fn finish_close_pr(&mut self, cx: &mut Context<Self>) {
        self.closing_pr = None;
        self.pr_menu_open = None;
        cx.notify();
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
        if self.closing_pr.is_some() {
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
                Err(e) => {
                    eprintln!("failed to close pull request: {e}");
                    this.update(cx, |this, cx| {
                        this.finish_close_pr(cx);
                        this.fetch(cx);
                    })
                    .ok();
                }
            }
        })
        .detach();
    }
}

impl Render for PullRequestsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let refreshing = self.refreshing;
        let body = match &self.prs {
            PrsState::Loading => div()
                .id("prs")
                .flex()
                .flex_1()
                .items_center()
                .justify_center()
                .child(Spinner::new("prs-loading"))
                .into_any_element(),
            PrsState::Failed(e) => div()
                .id("prs")
                .flex()
                .flex_col()
                .gap_2()
                .child(div().text_color(rgb(0xff6666)).child(e.to_string()))
                .child(
                    Button::new("retry-prs", "Retry")
                        .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                )
                .into_any_element(),
            PrsState::Loaded(prs) => loaded(self, prs, cx).into_any_element(),
        };
        main_layout(
            self.router.clone(),
            self.session.clone(),
            refreshing,
            body,
            cx,
        )
    }
}

fn loaded(
    view: &PullRequestsView,
    prs: &[model::PullRequest],
    cx: &mut Context<PullRequestsView>,
) -> gpui::Stateful<gpui::Div> {
    let route = view.router.read(cx).current().clone();
    let logged_in = view.session.read(cx).logged_in;
    let active: Vec<&model::PullRequest> = prs.iter().collect();

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
        .filter(|repo| view.repo_tab_visible(repo))
        .cloned()
        .collect();

    let selected: Option<Arc<str>> = match &route {
        Route::PullRequests(PullRequestsPage::Repo(repo)) if tab_repos.contains(repo) => {
            Some(repo.clone())
        }
        Route::PullRequests(_) => Some("all".into()),
        Route::Login | Route::Activity => None,
    };

    let mut visible: Vec<&model::PullRequest> = active
        .iter()
        .copied()
        .filter(|pr| view.repo_tab_visible(&pr.repo))
        .filter(|pr| match selected.as_deref() {
            Some("all") | None => true,
            Some(repo) => repo == pr.repo.as_str(),
        })
        .collect();
    visible.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

    let mut tabs: Vec<AnyElement> = Vec::new();
    if !repos.is_empty() {
        tabs.push(
            MultiSelect::new("repo-visibility")
                .open(view.visibility_menu_open)
                .options(
                    repos
                        .iter()
                        .map(|repo| {
                            SelectOption::new(
                                repo.to_string(),
                                repo.to_string(),
                                view.repo_tab_visible(repo),
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
    tabs.push(
        Tab::new("repo-tab-all", "All")
            .selected(selected.as_deref() == Some("all"))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.router.update(cx, |router, cx| {
                    router.replace(Route::pull_requests(), Session::allow(logged_in), cx);
                });
            }))
            .into_any_element(),
    );
    tabs.extend(tab_repos.iter().enumerate().map(|(ix, repo)| {
        Tab::new(
            SharedString::from(format!("repo-tab-{ix}")),
            repo.to_string(),
        )
        .selected(Some(repo.as_ref()) == selected.as_deref())
        .on_click(cx.listener({
            let repo = repo.clone();
            move |this, _, _, cx| {
                this.router.update(cx, |router, cx| {
                    router.replace(
                        Route::PullRequests(PullRequestsPage::Repo(repo.clone())),
                        Session::allow(logged_in),
                        cx,
                    );
                });
            }
        }))
        .into_any_element()
    }));

    let content: Vec<AnyElement> = if visible.is_empty() {
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
                    .menu_open(view.pr_menu_open.as_deref() == Some(&menu_key))
                    .closing(view.closing_pr.as_deref() == Some(&menu_key))
                    .on_toggle_menu(cx.listener({
                        let menu_key = menu_key.clone();
                        move |this, _, _, cx| {
                            if this.closing_pr.is_some() {
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
                        if this.closing_pr.is_some() {
                            return;
                        }
                        this.pr_menu_open = None;
                        cx.notify();
                    }))
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
                    .into_any_element()
            })
            .collect()
    };

    div()
        .id("prs")
        .flex()
        .flex_col()
        .child(
            div()
                .id("repo-tabs")
                .flex()
                .flex_wrap()
                .items_center()
                .gap_2()
                .pl_3()
                .pb_4()
                .children(tabs),
        )
        .children(content)
}
