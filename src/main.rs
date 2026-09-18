use std::borrow::Cow;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, App, Application, AssetSource, Bounds, Context, FontWeight, MouseDownEvent,
    PromptLevel, Render, Result, SharedString, Subscription, Timer, TitlebarOptions, Window,
    WindowBounds, WindowOptions, div, ease_in_out, prelude::*, px, rgb, size,
};

mod button;
mod datasource;
mod github;
mod icon;
mod model;
mod pr_status;
mod prefs;
mod select;
mod spinner;
mod tab;

use button::Button;
use datasource::CodeHost;
use gpui_selectable_text::SelectableText;
use icon::{Icon, IconName};
use prefs::Prefs;
use select::{MultiSelect, SelectOption};
use spinner::Spinner;
use tab::Tab;

const DEFAULT_WINDOW_SIZE: gpui::Size<gpui::Pixels> = size(px(800.0), px(600.0));
const RESTORE_ANIMATION: Duration = Duration::from_millis(250);

fn running_from_app_bundle() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|p| p.to_path_buf()))
        .is_some_and(|macos_dir| macos_dir.file_name().and_then(|s| s.to_str()) == Some("MacOS"))
}

fn init_desktop_notifications() {
    #[cfg(target_os = "macos")]
    {
        if !running_from_app_bundle() {
            return;
        }
        // UNUserNotificationCenter is required on current macOS; the old
        // NSUserNotification path can prompt for permission and still deliver nothing.
        std::thread::spawn(|| {
            let _ = notify_rust::request_auth_blocking();
        });
    }
}

fn show_desktop_notification(summary: impl Into<String>, body: impl Into<String>) {
    let summary = summary.into();
    let body = body.into();
    std::thread::spawn(move || {
        let result = notify_rust::Notification::new()
            .summary(&summary)
            .body(&body)
            .sound_name("default")
            .show();
        if let Err(err) = result {
            eprintln!("failed to show notification: {err}");
        }
    });
}

fn code_host() -> std::sync::Arc<dyn CodeHost> {
    std::sync::Arc::new(github::GithubApi::new())
}

struct Assets {
    base: PathBuf,
}

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        fs::read(self.base.join(path))
            .map(|data| Some(Cow::Owned(data)))
            .map_err(|err| err.into())
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        fs::read_dir(self.base.join(path))
            .map(|entries| {
                entries
                    .filter_map(|entry| {
                        entry
                            .ok()
                            .and_then(|entry| entry.file_name().into_string().ok())
                            .map(SharedString::from)
                    })
                    .collect()
            })
            .map_err(|err| err.into())
    }
}

#[derive(Clone)]
enum AuthState {
    LoggedOut,
    RequestingCode {
        error: Option<Arc<str>>,
    },
    PendingCode {
        user_code: Arc<str>,
        verification_uri: Arc<str>,
    },
    LoggedIn {
        prs: PrsState,
    },
}

#[derive(Clone)]
enum PrsState {
    Loading,
    Loaded(Vec<model::PullRequest>),
    Failed(Arc<str>),
}

struct HelloWorld {
    auth: AuthState,
    selected_repo: Option<Arc<str>>,
    visible_tab_repos: Option<HashSet<String>>,
    visibility_menu_open: bool,
    refreshing: bool,
    loading: bool,
    resize_generation: u64,
    fetch_in_flight: bool,
    _activation_subscription: Option<Subscription>,
}

impl HelloWorld {
    fn new(auth: AuthState, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut view = Self {
            auth,
            selected_repo: Some("all".into()),
            visible_tab_repos: Prefs::load_visible_tab_repos(),
            visibility_menu_open: false,
            refreshing: false,
            loading: false,
            resize_generation: 0,
            fetch_in_flight: false,
            _activation_subscription: None,
        };
        view._activation_subscription =
            Some(cx.observe_window_activation(window, |this, window, cx| {
                if window.is_window_active() && matches!(this.auth, AuthState::LoggedIn { .. }) {
                    this.refresh_prs(cx);
                }
            }));
        if matches!(view.auth, AuthState::LoggedIn { .. }) {
            view.loading = true;
            view.fetch_prs(cx);
        }
        view.poll_prs(cx);
        view
    }

    fn poll_prs(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                Timer::after(Duration::from_secs(30)).await;
                this.update(cx, |this, cx| {
                    if matches!(this.auth, AuthState::LoggedIn { .. }) {
                        this.fetch_prs(cx);
                    }
                })
                .ok();
            }
        })
        .detach();
    }

    fn refresh_prs(&mut self, cx: &mut Context<Self>) {
        self.refreshing = true;
        self.fetch_prs(cx);
    }

    fn fetch_prs(&mut self, cx: &mut Context<Self>) {
        if self.fetch_in_flight {
            return;
        }
        self.fetch_in_flight = true;
        let previous_urls: Option<HashSet<String>> = if let AuthState::LoggedIn {
            prs: PrsState::Loaded(prs),
        } = &self.auth
        {
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
                this.loading = false;
                if let AuthState::LoggedIn { prs } = &mut this.auth {
                    *prs = match result {
                        Ok(prs) => PrsState::Loaded(prs),
                        Err(e) => PrsState::Failed(e.message.into()),
                    };
                }
            })
            .ok();
            this.update(cx, |_, cx| cx.notify()).ok();

            for pr in merged {
                show_desktop_notification(
                    "Pull request merged",
                    &format!("{} · {}", pr.title, pr.repo),
                );
            }
        })
        .detach();
    }

    fn start_login(&mut self, cx: &mut Context<Self>) {
        self.auth = AuthState::RequestingCode { error: None };
        cx.notify();
        let host = code_host();
        let poll_host = host.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { host.start_login() })
                .await;
            this.update(cx, |this, cx| match result {
                Ok(code) => {
                    this.auth = AuthState::PendingCode {
                        user_code: code.user_code.clone().into(),
                        verification_uri: code.verification_uri.clone().into(),
                    };
                    cx.notify();
                    cx.spawn(async move |this, cx| {
                        let result = cx
                            .background_executor()
                            .spawn(async move { poll_host.await_login(&code) })
                            .await;
                        this.update(cx, |this, cx| match result {
                            Ok(_) => {
                                this.auth = AuthState::LoggedIn {
                                    prs: PrsState::Loading,
                                };
                                this.loading = true;
                                this.fetch_prs(cx);
                            }
                            Err(e) => {
                                this.auth = AuthState::RequestingCode {
                                    error: Some(e.message.into()),
                                };
                            }
                        })
                        .ok();
                        this.update(cx, |_, cx| cx.notify()).ok();
                    })
                    .detach();
                }
                Err(e) => {
                    this.auth = AuthState::RequestingCode {
                        error: Some(e.message.into()),
                    };
                    cx.notify();
                }
            })
            .ok();
            this.update(cx, |_, cx| cx.notify()).ok();
        })
        .detach();
    }

    fn animate_restore_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let from = window.viewport_size();
        let to = DEFAULT_WINDOW_SIZE;
        if from == to {
            return;
        }

        self.resize_generation = self.resize_generation.wrapping_add(1);
        let generation = self.resize_generation;
        let started = Instant::now();

        cx.spawn_in(window, async move |this, cx| {
            loop {
                let t =
                    (started.elapsed().as_secs_f32() / RESTORE_ANIMATION.as_secs_f32()).min(1.0);
                let e = ease_in_out(t);
                let width = from.width + (to.width - from.width) * e;
                let height = from.height + (to.height - from.height) * e;
                let keep_going = this
                    .update_in(cx, |this, window, _cx| {
                        if this.resize_generation != generation {
                            false
                        } else {
                            window.resize(size(width, height));
                            true
                        }
                    })
                    .unwrap_or(false);
                if !keep_going || t >= 1.0 {
                    break;
                }
                Timer::after(Duration::from_millis(8)).await;
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
        if self.selected_repo.as_deref() == Some(repo) {
            self.selected_repo = Some("all".into());
        }
        Prefs::save_visible_tab_repos(&visible);
        self.visible_tab_repos = Some(visible);
        cx.notify();
    }

    fn logout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let answer = window.prompt(
            PromptLevel::Warning,
            "Are you sure you want to log out?",
            None,
            &["Logout", "Cancel"],
            cx,
        );
        let host = code_host();
        cx.spawn(async move |this, cx| {
            if answer.await == Ok(0) {
                this.update(cx, |this, cx| {
                    host.logout();
                    this.auth = AuthState::LoggedOut;
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }
}

impl Render for HelloWorld {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut repo_tabs: Vec<AnyElement> = Vec::new();
        let mut content = match &self.auth {
            AuthState::LoggedOut => {
                vec![
                    Button::new("login", "Log in with GitHub")
                        .on_click(cx.listener(|state, _, _, cx| state.start_login(cx)))
                        .into_any_element(),
                ]
            }
            AuthState::RequestingCode { error } => {
                let mut children = vec![
                    Button::new("login", "Log in with GitHub")
                        .is_loading(true)
                        .into_any_element(),
                ];
                if let Some(e) = error {
                    children.push(
                        div()
                            .text_color(rgb(0xff6666))
                            .child(e.to_string())
                            .into_any_element(),
                    );
                    children.push(
                        Button::new("retry", "Retry")
                            .on_click(cx.listener(|state, _, _, cx| state.start_login(cx)))
                            .into_any_element(),
                    );
                }
                children
            }
            AuthState::PendingCode {
                user_code,
                verification_uri,
            } => vec![
                div()
                    .text_3xl()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(SelectableText::new("otp-code", user_code.to_string()))
                    .into_any_element(),
                Button::new("open-verification", "Open verification page")
                    .on_click(cx.listener({
                        let verification_uri = verification_uri.clone();
                        move |_, _, _, cx| {
                            cx.open_url(&verification_uri);
                        }
                    }))
                    .into_any_element(),
            ],
            AuthState::LoggedIn { prs } => match prs {
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
                        .text_color(rgb(0xff6666))
                        .child(e.to_string())
                        .into_any_element(),
                    Button::new("retry-prs", "Retry")
                        .on_click(cx.listener(|state, _, _, cx| state.refresh_prs(cx)))
                        .into_any_element(),
                ],
                PrsState::Loaded(prs) => {
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
                        .filter(|repo| self.repo_tab_visible(repo))
                        .cloned()
                        .collect();

                    let selected = self
                        .selected_repo
                        .clone()
                        .filter(|r| tab_repos.contains(r) || r.as_ref() == "all")
                        .or_else(|| Some("all".into()));

                    let mut visible: Vec<&model::PullRequest> = active
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
                                .on_toggle_open(cx.listener(|state, _, _, cx| {
                                    state.visibility_menu_open = !state.visibility_menu_open;
                                    cx.notify();
                                }))
                                .on_dismiss(cx.listener(|state, _, _, cx| {
                                    state.visibility_menu_open = false;
                                    cx.notify();
                                }))
                                .on_toggle_option(cx.listener({
                                    let repos = repos.clone();
                                    move |state, repo: &str, _, cx| {
                                        state.toggle_repo_tab_visibility(repo, &repos, cx);
                                    }
                                }))
                                .into_any_element(),
                        );
                    }
                    repo_tabs.push(
                        Tab::new("repo-tab-all", "All")
                            .selected(selected.as_deref() == Some("all"))
                            .on_click(cx.listener(|state, _, _, cx| {
                                state.selected_repo = Some("all".into());
                                state.visibility_menu_open = false;
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
                            move |state, _, _, cx| {
                                state.selected_repo = Some(repo.clone());
                                state.visibility_menu_open = false;
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
                                let url = pr.url.clone();
                                div()
                                    .id(("pr", ix))
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .py_2()
                                    .px_3()
                                    .rounded_md()
                                    .hover(|this| this.bg(rgb(0x2a2a2a)))
                                    .cursor_pointer()
                                    .on_click(move |_, _window, cx| cx.open_url(&url))
                                    .child(
                                        div()
                                            .flex()
                                            .gap_2()
                                            .items_center()
                                            .child(pr_status::PrStatusIcon::new(
                                                pr.status().clone(),
                                            ))
                                            .child(pr_status::PrStatusLabel::new(
                                                pr.status().clone(),
                                            ))
                                            .child(
                                                div()
                                                    .text_color(rgb(0x8b949e))
                                                    .child(pr.repo.clone()),
                                            )
                                            .child(pr_status::CiStatusIcon::new(pr.ci)),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .gap_2()
                                            .items_center()
                                            .child(div().child(pr.title.clone())),
                                    )
                            })
                            .map(|el| el.into_any_element())
                            .collect()
                    }
                }
            },
        };

        let top_right = if matches!(self.auth, AuthState::LoggedIn { .. }) {
            Some(
                div()
                    .id("top-right")
                    .absolute()
                    .top_4()
                    .right_4()
                    .flex()
                    .items_center()
                    .gap_2()
                    .when(self.refreshing && !self.loading, |this| {
                        this.child(div().id("refreshing").child(Spinner::new("refreshing")))
                    })
                    .child(
                        Button::new("logout", "")
                            .icon(Icon::new(IconName::Logout).size(px(20.0)))
                            .tertiary()
                            .on_click(cx.listener(|state, _, window, cx| state.logout(window, cx))),
                    ),
            )
        } else {
            None
        };

        div()
            .id("root")
            .size_full()
            .flex()
            .flex_col()
            .relative()
            .bg(rgb(0x1e1e1e))
            .text_color(rgb(0xffffff))
            .when(!matches!(self.auth, AuthState::LoggedIn { .. }), |this| {
                this.items_center().justify_center().gap_4()
            })
            .when(matches!(self.auth, AuthState::LoggedIn { .. }), |this| {
                this.child(
                    div()
                        .id("pr-scroll")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .flex()
                        .flex_col()
                        .items_center()
                        .child(
                            div()
                                .id("pr-list")
                                .w_full()
                                .max_w(px(560.0))
                                .flex()
                                .flex_col()
                                .when(
                                    matches!(
                                        self.auth,
                                        AuthState::LoggedIn {
                                            prs: PrsState::Loading
                                        }
                                    ),
                                    |this| this.flex_1(),
                                )
                                .when(
                                    !matches!(
                                        self.auth,
                                        AuthState::LoggedIn {
                                            prs: PrsState::Loading
                                        }
                                    ),
                                    |this| this.pt_16().pb_16(),
                                )
                                .px_4()
                                .when(!repo_tabs.is_empty(), |this| {
                                    this.child(
                                        div()
                                            .id("repo-tabs-title")
                                            .pl_3()
                                            .pb_3()
                                            .text_2xl()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child("Your Pull Requests"),
                                    )
                                })
                                .when(!repo_tabs.is_empty(), |this| {
                                    this.child(
                                        div()
                                            .id("repo-tabs")
                                            .flex()
                                            .flex_wrap()
                                            .items_center()
                                            .gap_2()
                                            .pl_3()
                                            .pb_4()
                                            .children(std::mem::take(&mut repo_tabs)),
                                    )
                                })
                                .children(std::mem::take(&mut content)),
                        ),
                )
            })
            .when(!matches!(self.auth, AuthState::LoggedIn { .. }), |this| {
                this.children(content)
            })
            .children(top_right)
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    const TITLEBAR_HEIGHT: gpui::Pixels = px(28.0);
                    if event.click_count == 2 && event.position.y < TITLEBAR_HEIGHT {
                        this.animate_restore_window(window, cx);
                    }
                }),
            )
    }
}

fn asset_base() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(macos_dir) = exe.parent() {
            if macos_dir.file_name().and_then(|s| s.to_str()) == Some("MacOS") {
                if let Some(contents) = macos_dir.parent() {
                    let bundled = contents.join("Resources").join("assets");
                    if bundled.exists() {
                        return bundled;
                    }
                }
            }
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets")
}

fn main() {
    init_desktop_notifications();
    Application::new()
        .with_assets(Assets { base: asset_base() })
        .run(|cx: &mut App| {
            gpui_selectable_text::register_keyboard_bridge(cx).detach();
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        DEFAULT_WINDOW_SIZE,
                        cx,
                    ))),
                    window_min_size: Some(size(px(480.0), px(600.0))),
                    titlebar: Some(TitlebarOptions {
                        appears_transparent: true,
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| {
                    let auth = if code_host().has_saved_session() {
                        let prs = match github::PrsCache::load() {
                            Some(prs) if !prs.is_empty() => PrsState::Loaded(prs),
                            _ => PrsState::Loading,
                        };
                        AuthState::LoggedIn { prs }
                    } else {
                        AuthState::LoggedOut
                    };
                    cx.new(|cx| HelloWorld::new(auth, window, cx))
                },
            )
            .unwrap();
            cx.activate(true);
        });
}
