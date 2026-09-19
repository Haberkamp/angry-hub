use gpui::{
    div, prelude::*, rgb, Context, Entity, FontWeight, IntoElement, ParentElement, Render, Styled,
    Window,
};

use gpui_selectable_text::SelectableText;

use crate::code_host;
use crate::layout::bare_layout;
use crate::route::Route;
use crate::router::Router;
use crate::session::Session;
use crate::ui::Button;

use std::sync::Arc;

enum LoginPhase {
    LoggedOut,
    RequestingCode {
        error: Option<Arc<str>>,
    },
    PendingCode {
        user_code: Arc<str>,
        verification_uri: Arc<str>,
    },
}

pub struct LoginView {
    phase: LoginPhase,
    session: Entity<Session>,
    router: Entity<Router<Route>>,
}

impl LoginView {
    pub fn new(session: Entity<Session>, router: Entity<Router<Route>>) -> Self {
        Self {
            phase: LoginPhase::LoggedOut,
            session,
            router,
        }
    }

    fn start_login(&mut self, cx: &mut Context<Self>) {
        self.phase = LoginPhase::RequestingCode { error: None };
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
                    this.phase = LoginPhase::PendingCode {
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
                                this.phase = LoginPhase::LoggedOut;
                                this.session.update(cx, |session, cx| {
                                    session.logged_in = true;
                                    cx.notify();
                                });
                                let logged_in = true;
                                this.router.update(cx, |router, cx| {
                                    router.enforce(Session::allow(logged_in), cx);
                                });
                            }
                            Err(e) => {
                                this.phase = LoginPhase::RequestingCode {
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
                    this.phase = LoginPhase::RequestingCode {
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

impl Render for LoginView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = match &self.phase {
            LoginPhase::LoggedOut => div()
                .id("login")
                .flex()
                .flex_col()
                .items_center()
                .gap_4()
                .child(
                    Button::new("login", "Log in with GitHub")
                        .on_click(cx.listener(|this, _, _, cx| this.start_login(cx))),
                ),
            LoginPhase::RequestingCode { error } => {
                let mut col = div()
                    .id("login")
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_4()
                    .child(
                        Button::new("login", "Log in with GitHub").loading(true),
                    );
                if let Some(e) = error.clone() {
                    col = col
                        .child(div().text_color(rgb(0xff6666)).child(e.to_string()))
                        .child(
                            Button::new("retry", "Retry")
                                .on_click(cx.listener(|this, _, _, cx| this.start_login(cx))),
                        );
                }
                col
            }
            LoginPhase::PendingCode {
                user_code,
                verification_uri,
            } => {
                let verification_uri = verification_uri.clone();
                div()
                    .id("login")
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_4()
                    .child(
                        div()
                            .text_3xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(SelectableText::new("otp-code", user_code.to_string())),
                    )
                    .child(
                        Button::new("open-verification", "Open verification page").on_click(
                            cx.listener(move |_, _, _, cx| {
                                cx.open_url(&verification_uri);
                            }),
                        ),
                    )
            }
        };
        bare_layout(body)
    }
}
