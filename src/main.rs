use std::borrow::Cow;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use gpui::{
    div, prelude::*, px, rgb, size, AnyElement, App, Application, AssetSource, Bounds, Context,
    PromptLevel, Render, Result, SharedString, TitlebarOptions, Window, WindowBounds, WindowOptions,
};

mod button;
mod datasource;
mod github;
mod icon;
mod model;
mod pr_status;
mod tab;

use button::Button;
use datasource::CodeHost;
use icon::{Icon, IconName};
use tab::Tab;

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
    RequestingCode { error: Option<Arc<str>> },
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
}

impl HelloWorld {
    fn load_prs(&mut self, cx: &mut Context<Self>) {
        let host = code_host();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { host.my_pull_requests() })
                .await;
            this.update(cx, |this, _cx| {
                if let AuthState::LoggedIn { prs } = &mut this.auth {
                    *prs = match result {
                        Ok(prs) => PrsState::Loaded(prs),
                        Err(e) => PrsState::Failed(e.message.into()),
                    };
                }
            })
            .ok();
            this.update(cx, |_, cx| cx.notify()).ok();
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
                                this.load_prs(cx);
                            }
                            Err(e) => {
                                this.auth =
                                    AuthState::RequestingCode { error: Some(e.message.into()) };
                            }
                        })
                        .ok();
                        this.update(cx, |_, cx| cx.notify()).ok();
                    })
                    .detach();
                }
                Err(e) => {
                    this.auth = AuthState::RequestingCode { error: Some(e.message.into()) };
                    cx.notify();
                }
            })
            .ok();
            this.update(cx, |_, cx| cx.notify()).ok();
        })
        .detach();
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
                    div().child("Not logged in").into_any_element(),
                    Button::new("login", "Log in with GitHub")
                        .on_click(cx.listener(|state, _, _, cx| state.start_login(cx)))
                        .into_any_element(),
                ]
            }
            AuthState::RequestingCode { error } => {
                let mut children =
                    vec![div().child("Requesting device code...").into_any_element()];
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
                div().child("Waiting for authorization...").into_any_element(),
                div()
                    .text_xl()
                    .child(format!("Code: {}", user_code))
                    .into_any_element(),
                div().child(format!("Visit: {}", verification_uri)).into_any_element(),
            ],
            AuthState::LoggedIn { prs } => match prs {
                PrsState::Loading => vec![div().child("Loading PRs...").into_any_element()],
                PrsState::Failed(e) => vec![
                    div()
                        .text_color(rgb(0xff6666))
                        .child(e.to_string())
                        .into_any_element(),
                    Button::new("retry-prs", "Retry")
                        .on_click(cx.listener(|state, _, _, cx| state.load_prs(cx)))
                        .into_any_element(),
                ],
                PrsState::Loaded(prs) => {
                    let active: Vec<&model::PullRequest> = prs
                        .iter()
                        .filter(|pr| *pr.status() != model::PrStatus::Merged)
                        .collect();

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

                    let selected = self
                        .selected_repo
                        .clone()
                        .filter(|r| repos.contains(r) || r.as_ref() == "all");

                    let visible: Vec<&model::PullRequest> = active
                        .iter()
                        .copied()
                        .filter(|pr| match selected.as_deref() {
                            Some("all") | None => true,
                            Some(repo) => repo == pr.repo.as_str(),
                        })
                        .collect();

                    repo_tabs.push(
                        Tab::new("repo-tab-all", "All")
                            .selected(selected.as_deref() == Some("all"))
                            .on_click(cx.listener(|state, _, _, cx| {
                                state.selected_repo = Some("all".into());
                                cx.notify();
                            }))
                            .into_any_element(),
                    );
                    repo_tabs.extend(repos.iter().enumerate().map(|(ix, repo)| {
                        Tab::new(
                            SharedString::from(format!("repo-tab-{ix}")),
                            repo.to_string(),
                        )
                        .selected(Some(repo.as_ref()) == selected.as_deref())
                        .on_click(cx.listener({
                            let repo = repo.clone();
                            move |state, _, _, cx| {
                                state.selected_repo = Some(repo.clone());
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
                                            ),
                                    )
                                    .child(div().child(pr.title.clone()))
                            })
                            .map(|el| el.into_any_element())
                            .collect()
                    }
                }
            },
        };

        let logout_button = if matches!(self.auth, AuthState::LoggedIn { .. }) {
            Some(
                div()
                    .id("logout-container")
                    .absolute()
                    .top_4()
                    .right_4()
                    .child(
                        Button::new("logout", "")
                            .icon(Icon::new(IconName::Logout).size(px(20.0)))
                            .tertiary()
                            .on_click(cx.listener(|state, _, window, cx| {
                                state.logout(window, cx)
                            })),
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
                        .size_full()
                        .flex_1()
                        .overflow_y_scroll()
                        .flex()
                        .flex_col()
                        .items_center()
                        .pt_16()
                        .child(
                            div()
                                .id("pr-list")
                                .w_full()
                                .max_w(px(560.0))
                                .flex()
                                .flex_col()
                                .px_4()
                                .pb_4()
                                .when(!repo_tabs.is_empty(), |this| {
                                    this.child(
                                        div()
                                            .id("repo-tabs")
                                            .flex()
                                            .flex_wrap()
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
            .children(logout_button)
    }
}

fn main() {
    Application::new()
        .with_assets(Assets {
            base: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets"),
        })
        .run(|cx: &mut App| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(800.0), px(600.0)),
                    cx,
                ))),
                window_min_size: Some(size(px(480.0), px(640.0))),
                titlebar: Some(TitlebarOptions {
                    appears_transparent: true,
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_window, cx| {
                let auth = if code_host().has_saved_session() {
                    AuthState::LoggedIn {
                        prs: PrsState::Loading,
                    }
                } else {
                    AuthState::LoggedOut
                };
                cx.new(|cx| {
                    let mut view = HelloWorld {
                        auth,
                        selected_repo: Some("all".into()),
                    };
                    if matches!(view.auth, AuthState::LoggedIn { .. }) {
                        view.load_prs(cx);
                    }
                    view
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}