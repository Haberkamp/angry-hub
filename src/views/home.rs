use gpui::prelude::*;
use gpui::{
    Context, Entity, EventEmitter, IntoElement, MouseButton, PromptButton, PromptLevel, Render,
    Window, div, px, rgb, svg,
};
use gpui_base::StyledExt as _;

use crate::auth::Auth;
use crate::icon;
use crate::keychain::KeychainStore;

const INSET: f32 = 12.;

pub struct LoggedOut;

pub struct Home;

impl Home {
    fn logout(&mut self, cx: &mut Context<Self>) {
        let mut auth = Auth::load(&KeychainStore);
        auth.logout(&KeychainStore);
        cx.emit(LoggedOut);
    }

}

impl EventEmitter<LoggedOut> for Home {}

pub fn logout_button(home: Entity<Home>) -> impl IntoElement {
    div()
        .id("logout")
        .group("logout")
        .absolute()
        .top(px(INSET))
        .right(px(INSET))
        .size(px(40.))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .text_color(rgb(0x6E6E6E))
        .hover(|style| style.bg(rgb(0x222222)).text_color(rgb(0xffffff)))
        .active(|style| style.bg(rgb(0x313131)))
        .on_mouse_down(MouseButton::Left, |_, _, cx| {
            cx.stop_propagation();
        })
        .on_click(move |_, window, cx| {
            let answer = window.prompt(
                PromptLevel::Warning,
                "Log out?",
                Some("Are you sure you want to log out?"),
                &[
                    PromptButton::ok("Log out"),
                    PromptButton::cancel("Cancel"),
                ],
                cx,
            );
            home.update(cx, |_, cx| {
                cx.spawn(async move |this, cx| {
                    if answer.await == Ok(0) {
                        this.update(cx, |this, cx| this.logout(cx)).ok();
                    }
                })
                .detach();
            });
        })
        .child(
            svg()
                .data(icon::LOGOUT)
                .size(px(16.))
                .text_color(rgb(0x6E6E6E))
                .group_hover("logout", |style| style.text_color(rgb(0xffffff))),
        )
}

impl Render for Home {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_1()
            .size_full()
            .v_flex()
            .items_center()
            .justify_center()
            .child("Home")
    }
}
