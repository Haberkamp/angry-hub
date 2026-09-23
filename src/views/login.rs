use std::time::Duration;

use gpui::prelude::*;
use gpui::{
    ClipboardItem, Context, EventEmitter, FontWeight, IntoElement, Render, Window, div, px, rgb,
    svg,
};
use gpui_base::StyledExt as _;

use crate::button::Button;
use crate::icon;
use crate::tooltip::Tooltip;

use crate::auth::Auth;
use crate::github::{DevicePoll, GITHUB_CLIENT_ID, Github, GithubClient, ReqwestHttp};
use crate::keychain::KeychainStore;

const OTP_TOOLTIP: &str = "Click to copy";
const OTP_COPIED_TOOLTIP: &str = "Copied to clipboard";
const OTP_COPIED_FOR: Duration = Duration::from_secs(2);
pub struct LoggedIn;

enum Step {
    Idle,
    Waiting {
        user_code: String,
        verification_uri: String,
    },
}

pub struct Login {
    step: Step,
    error: Option<String>,
    generation: u64,
    otp_copied: bool,
    otp_reset: u64,
}

impl Login {
    pub fn new() -> Self {
        Self {
            step: Step::Idle,
            error: None,
            generation: 0,
            otp_copied: false,
            otp_reset: 0,
        }
    }

    fn copy_otp(&mut self, code: &str, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(code.to_string()));
        self.otp_copied = true;
        self.otp_reset = self.otp_reset.wrapping_add(1);
        let reset = self.otp_reset;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(OTP_COPIED_FOR).await;
            this.update(cx, |login, cx| {
                if login.otp_reset != reset {
                    return;
                }
                login.otp_copied = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn back(&mut self, cx: &mut Context<Self>) {
        self.generation = self.generation.wrapping_add(1);
        self.step = Step::Idle;
        self.error = None;
        cx.notify();
    }

    fn start(&mut self, cx: &mut Context<Self>) {
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        self.error = None;
        cx.spawn(async move |this, cx| {
            let started = cx
                .background_spawn(async move {
                    GithubClient::new(GITHUB_CLIENT_ID, ReqwestHttp).start_device_flow()
                })
                .await;
            this.update(cx, |login, cx| match started {
                Ok(code) => {
                    let device_code = code.device_code.clone();
                    let interval = code.interval_secs;
                    login.step = Step::Waiting {
                        user_code: code.user_code,
                        verification_uri: code.verification_uri,
                    };
                    cx.notify();
                    login.poll(device_code, interval, generation, cx);
                }
                Err(error) => {
                    login.error = Some(error.message().to_string());
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn poll(
        &mut self,
        device_code: String,
        mut interval: u64,
        generation: u64,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            loop {
                let waited = interval;
                let device_code = device_code.clone();
                let polled = cx
                    .background_spawn(async move {
                        std::thread::sleep(Duration::from_secs(waited));
                        GithubClient::new(GITHUB_CLIENT_ID, ReqwestHttp)
                            .poll_device_flow(&device_code)
                    })
                    .await;
                let stop = this
                    .update(cx, |login, cx| {
                        if login.generation != generation {
                            return true;
                        }
                        match polled {
                            Ok(DevicePoll::Pending) => false,
                            Ok(DevicePoll::SlowDown) => {
                                interval += 5;
                                false
                            }
                            Ok(DevicePoll::Approved(tokens)) => {
                                match login.finish(tokens) {
                                    Ok(()) => cx.emit(LoggedIn),
                                    Err(error) => login.error = Some(error),
                                }
                                cx.notify();
                                true
                            }
                            Ok(DevicePoll::Denied) => {
                                login.fail("GitHub denied the login", cx);
                                true
                            }
                            Ok(DevicePoll::Expired) => {
                                login.fail("The login code expired", cx);
                                true
                            }
                            Err(error) => {
                                login.fail(error.message(), cx);
                                true
                            }
                        }
                    })
                    .unwrap_or(true);
                if stop {
                    break;
                }
            }
        })
        .detach();
    }

    fn finish(&mut self, tokens: crate::session::Tokens) -> Result<(), String> {
        let mut auth = Auth::new();
        auth.login(tokens, &KeychainStore)
    }

    fn fail(&mut self, message: &str, cx: &mut Context<Self>) {
        self.step = Step::Idle;
        self.error = Some(message.to_string());
        cx.notify();
    }
}

impl EventEmitter<LoggedIn> for Login {}

impl Render for Login {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut column = div()
            .flex_1()
            .size_full()
            .v_flex()
            .gap_4()
            .items_center()
            .justify_center()
            .text_color(rgb(0xffffff));

        column = match &self.step {
            Step::Idle => column.child(
                Button::primary("login")
                    .label("Log in with GitHub")
                    .icon(icon::GITHUB)
                    .on_click(cx.listener(|this, _, _, cx| this.start(cx))),
            ),
            Step::Waiting {
                user_code,
                verification_uri,
            } => {
                let verification_uri = verification_uri.clone();
                let code = user_code.clone();
                let copied = self.otp_copied;
                let tooltip = if copied {
                    OTP_COPIED_TOOLTIP
                } else {
                    OTP_TOOLTIP
                };
                column.child(
                    div()
                        .w(px(280.))
                        .v_flex()
                        .gap(px(24.))
                        .items_center()
                        .child(
                            Tooltip::new("otp-tooltip", tooltip).child({
                                let code = code.clone();
                                let shown = code.clone();
                                div()
                                    .id("otp-code")
                                    .relative()
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .text_3xl()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .hover(|style| style.bg(rgb(0x222222)))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.copy_otp(&code, cx);
                                    }))
                                    .child(div().when(copied, |code| code.opacity(0.)).child(shown))
                                    .when(copied, |code| {
                                        code.child(
                                            div()
                                                .absolute()
                                                .inset_0()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .child(
                                                    svg()
                                                        .data(icon::CHECK)
                                                        .size(px(28.))
                                                        .text_color(rgb(0xffffff)),
                                                ),
                                        )
                                    })
                            }),
                        )
                        .child(
                            div()
                                .w_full()
                                .v_flex()
                                .gap_2()
                                .child(
                            Button::primary("open-github")
                                .full()
                                .label("Open GitHub")
                                .on_click(move |_, _, cx| {
                                    cx.open_url(&verification_uri);
                                }),
                        )
                        .child(
                            Button::secondary("back")
                                .full()
                                .label("Back")
                                .icon(icon::CHEVRON_LEFT)
                                .on_click(cx.listener(|this, _, _, cx| this.back(cx))),
                        ),
                        ),
                )
            }
        };

        if let Some(error) = &self.error {
            column = column.child(error.clone());
        }
        column
    }
}
