use std::sync::Arc;

use gpui::{ClipboardItem, Context, FontWeight, Render, Window, div, prelude::*, px};
use rooter::Router;

use crate::color;
use crate::datasource::code_host;
use crate::session::Session;
use crate::ui::{Button, Icon, IconName, Tooltip};

const OTP_TOOLTIP: &str = "Click to copy";
const OTP_COPIED_TOOLTIP: &str = "Copied to clipboard";

enum LoginState {
    LoggedOut,
    RequestingCode {
        error: Option<Arc<str>>,
    },
    PendingCode {
        user_code: Arc<str>,
        verification_uri: Arc<str>,
    },
}

pub struct Login {
    state: LoginState,
    otp_copied: bool,
}

impl Login {
    pub fn new() -> Self {
        Self {
            state: LoginState::LoggedOut,
            otp_copied: false,
        }
    }

    fn copy_otp(&mut self, code: &str, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(code.to_string()));
        self.otp_copied = true;
        cx.notify();
    }

    fn start_login(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.state = LoginState::RequestingCode { error: None };
        cx.notify();
        let host = code_host();
        let poll_host = host.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { host.start_login() })
                .await;
            this.update_in(cx, |this, window, cx| match result {
                Ok(code) => {
                    this.state = LoginState::PendingCode {
                        user_code: code.user_code.clone().into(),
                        verification_uri: code.verification_uri.clone().into(),
                    };
                    cx.notify();
                    cx.spawn_in(window, async move |this, cx| {
                        let result = cx
                            .background_executor()
                            .spawn(async move { poll_host.await_login(&code) })
                            .await;
                        this.update_in(cx, |this, window, cx| match result {
                            Ok(_) => {
                                this.state = LoginState::LoggedOut;
                                cx.global_mut::<Session>().logged_in = true;
                                Router::navigate_window(window, cx, "/");
                            }
                            Err(e) => {
                                if matches!(this.state, LoginState::PendingCode { .. }) {
                                    this.state = LoginState::RequestingCode {
                                        error: Some(e.message.into()),
                                    };
                                }
                            }
                        })
                        .ok();
                        this.update(cx, |_, cx| cx.notify()).ok();
                    })
                    .detach();
                }
                Err(e) => {
                    this.state = LoginState::RequestingCode {
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
}

impl Render for Login {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let children: Vec<_> = match &self.state {
            LoginState::LoggedOut => vec![
                Button::new("login", "Log in with GitHub")
                    .icon(Icon::new(IconName::Github).size(px(16.0)))
                    .on_click(cx.listener(|this, _, window, cx| this.start_login(window, cx)))
                    .into_any_element(),
            ],
            LoginState::RequestingCode { error } => {
                let mut children = vec![
                    Button::new("login", "Log in with GitHub")
                        .loading(true)
                        .into_any_element(),
                ];
                if let Some(e) = error {
                    children.push(
                        div()
                            .text_color(color::red::s9())
                            .child(e.to_string())
                            .into_any_element(),
                    );
                    children.push(
                        Button::new("retry", "Retry")
                            .on_click(
                                cx.listener(|this, _, window, cx| this.start_login(window, cx)),
                            )
                            .into_any_element(),
                    );
                }
                children
            }
            LoginState::PendingCode {
                user_code,
                verification_uri,
            } => vec![
                Tooltip::new(
                    "otp-tooltip",
                    if self.otp_copied {
                        OTP_COPIED_TOOLTIP
                    } else {
                        OTP_TOOLTIP
                    },
                )
                .offset(px(4.0))
                .on_close(cx.listener(|this, _, _, cx| {
                    if this.otp_copied {
                        this.otp_copied = false;
                        cx.notify();
                    }
                }))
                .child({
                    let code = user_code.clone();
                    let label = user_code.to_string();
                    div()
                        .id("otp-code")
                        .px_3()
                        .py_1()
                        .rounded_md()
                        .text_3xl()
                        .font_weight(FontWeight::SEMIBOLD)
                        .cursor_pointer()
                        .hover(|this| this.bg(color::gray::s3()))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.copy_otp(&code, cx);
                        }))
                        .child(label)
                })
                .into_any_element(),
                div()
                    .id("verification-actions")
                    .w_full()
                    .max_w(px(280.0))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        Button::new("open-verification", "Open verification page")
                            .full()
                            .on_click(cx.listener({
                                let verification_uri = verification_uri.clone();
                                move |_, _, _, cx| {
                                    cx.open_url(&verification_uri);
                                }
                            })),
                    )
                    .child(
                        Button::new("go-back", "Go back")
                            .tertiary()
                            .full()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.state = LoginState::LoggedOut;
                                this.otp_copied = false;
                                cx.notify();
                            })),
                    )
                    .into_any_element(),
            ],
        };

        div()
            .id("login")
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_4()
            .children(children)
    }
}
