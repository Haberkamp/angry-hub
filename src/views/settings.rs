use gpui::{App, Context, FontWeight, PromptLevel, Render, Window, div, prelude::*, px};

use crate::color;
use crate::session;
use crate::ui::{Button, Icon, IconName};

pub struct Settings;

impl Settings {
    pub fn new() -> Self {
        Self
    }
}

impl Render for Settings {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("settings")
            .flex()
            .flex_col()
            .items_start()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_3xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Setting"),
                    )
                    .child(
                        div()
                            .text_size(px(14.0))
                            .text_color(color::gray::s10())
                            .child("You are signed in with GitHub. Logging out ends the session on this device."),
                    ),
            )
            .child(
                Button::new("logout", "Logout")
                    .icon(Icon::new(IconName::Logout).size(px(16.0)))
                    .destructive()
                    .on_click(cx.listener(|_, _, window, cx| confirm_logout(window, cx))),
            )
    }
}

fn confirm_logout(window: &mut Window, cx: &mut App) {
    let answer = window.prompt(
        PromptLevel::Warning,
        "Are you sure you want to log out?",
        None,
        &["Logout", "Cancel"],
        cx,
    );
    let window = window.window_handle();
    cx.spawn(async move |cx| {
        if answer.await == Ok(0) {
            window
                .update(cx, |_, window, cx| {
                    session::force_logout(window, cx);
                })
                .ok();
        }
    })
    .detach();
}
