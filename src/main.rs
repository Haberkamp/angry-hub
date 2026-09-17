use std::borrow::Cow;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use gpui::{
    div, prelude::*, px, rgb, size, App, Application, AssetSource, Bounds, Context, PromptLevel,
    Render, Result, SharedString, TitlebarOptions, Window, WindowBounds, WindowOptions,
};

mod button;
mod github;
mod icon;

use button::Button;
use icon::{Icon, IconName};

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
    Loaded(Vec<github::PullRequest>),
    Failed(Arc<str>),
}

struct HelloWorld {
    auth: AuthState,
}

impl HelloWorld {
    fn load_prs(&mut self, cx: &mut Context<Self>) {
        let Some(token) = github::load_saved_token() else {
            return;
        };
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { github::fetch_my_prs(&token) })
                .await;
            this.update(cx, |this, _cx| {
                if let AuthState::LoggedIn { prs } = &mut this.auth {
                    *prs = match result {
                        Ok(prs) => PrsState::Loaded(prs),
                        Err(e) => PrsState::Failed(e.into()),
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
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { github::request_device_code() })
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
                            .spawn(async move { github::poll_for_token(&code) })
                            .await;
                        this.update(cx, |this, cx| match result {
                            Ok(github::LoginState::Success) => {
                                this.auth = AuthState::LoggedIn {
                                    prs: PrsState::Loading,
                                };
                                this.load_prs(cx);
                            }
                            Err(e) => {
                                this.auth = AuthState::RequestingCode { error: Some(e.into()) };
                            }
                        })
                        .ok();
                        this.update(cx, |_, cx| cx.notify()).ok();
                    })
                    .detach();
                }
                Err(e) => {
                    this.auth = AuthState::RequestingCode { error: Some(e.into()) };
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
        cx.spawn(async move |this, cx| {
            if answer.await == Ok(0) {
                this.update(cx, |this, cx| {
                    github::logout();
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
                    if prs.is_empty() {
                        vec![div().child("No PRs found").into_any_element()]
                    } else {
                        prs.iter()
                            .enumerate()
                            .map(|(ix, pr)| {
                                let state_label = if pr.draft {
                                    format!("{} (draft)", pr.state)
                                } else {
                                    pr.state.clone()
                                };
                                let state_color = if pr.state == "open" {
                                    rgb(0x3fb950)
                                } else {
                                    rgb(0x8b949e)
                                };
                                div()
                                    .id(("pr", ix))
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .py_2()
                                    .child(
                                        div()
                                            .flex()
                                            .gap_2()
                                            .items_baseline()
                                            .child(
                                                div()
                                                    .text_color(state_color)
                                                    .child(state_label),
                                            )
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
                        .p_4()
                        .pt_16()
                        .children(std::mem::take(&mut content)),
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
                let auth = if github::load_saved_token().is_some() {
                    AuthState::LoggedIn {
                        prs: PrsState::Loading,
                    }
                } else {
                    AuthState::LoggedOut
                };
                cx.new(|cx| {
                    let mut view = HelloWorld { auth };
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