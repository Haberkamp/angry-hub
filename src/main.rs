use gpui::{
    div, prelude::*, px, rgb, size, App, Application, Bounds, Context, Render, TitlebarOptions,
    Window, WindowBounds, WindowOptions,
};
use std::sync::Arc;

mod github;

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
                            Ok(github::LoginState::Failure(msg)) => {
                                this.auth =
                                    AuthState::RequestingCode { error: Some(msg.into()) };
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

    fn logout(&mut self, cx: &mut Context<Self>) {
        github::logout();
        self.auth = AuthState::LoggedOut;
        cx.notify();
    }
}

fn button(
    text: &str,
    cx: &Context<HelloWorld>,
    on_click: impl Fn(&mut HelloWorld, &mut Window, &mut Context<HelloWorld>) + 'static,
) -> impl IntoElement {
    div()
        .id(gpui::SharedString::from(text.to_string()))
        .px_4()
        .py_2()
        .bg(rgb(0x2d2d2d))
        .hover(|this| this.bg(rgb(0x3d3d3d)))
        .active(|this| this.bg(rgb(0x444444)))
        .border_1()
        .border_color(rgb(0x555555))
        .rounded_md()
        .cursor_pointer()
        .text_color(rgb(0xffffff))
        .child(text.to_string())
        .on_click(cx.listener(move |state, _, window, cx| {
            on_click(state, window, cx)
        }))
}

impl Render for HelloWorld {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match &self.auth {
            AuthState::LoggedOut => {
                vec![div()
                    .child("Not logged in")
                    .into_any_element(), button("Log in with GitHub", cx, |s, _, cx| s.start_login(cx)).into_any_element()]
            }
            AuthState::RequestingCode { error } => {
                let mut children = vec![div().child("Requesting device code...").into_any_element()];
                if let Some(e) = error {
                    children.push(div().text_color(rgb(0xff6666)).child(e.to_string()).into_any_element());
                    children.push(button("Retry", cx, |s, _, cx| s.start_login(cx)).into_any_element());
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
            AuthState::LoggedIn => vec![button("Logout", cx, |s, _, cx| s.logout(cx)).into_any_element()],
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_4()
            .bg(rgb(0x1e1e1e))
            .text_color(rgb(0xffffff))
            .children(content)
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(800.0), px(600.0)),
                    cx,
                ))),
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