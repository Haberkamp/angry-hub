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
    LoggedIn,
}

struct HelloWorld {
    auth: AuthState,
}

impl HelloWorld {
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
                        this.update(cx, |this, _cx| match result {
                            Ok(github::LoginState::Success) => {
                                this.auth = AuthState::LoggedIn;
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
        let content = match &self.auth {
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
            AuthState::LoggedIn => vec![],
        };

        let logout_button = if matches!(self.auth, AuthState::LoggedIn) {
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
            .items_center()
            .justify_center()
            .gap_4()
            .relative()
            .bg(rgb(0x1e1e1e))
            .text_color(rgb(0xffffff))
            .children(content)
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
                cx.new(|_| HelloWorld {
                    auth: if github::load_saved_token().is_some() {
                        AuthState::LoggedIn
                    } else {
                        AuthState::LoggedOut
                    },
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}