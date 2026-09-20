use gpui::{App, Context, FontWeight, PromptLevel, Render, Window, div, prelude::*, px};

use crate::color::{self, ThemePreference};
use crate::session;
use crate::ui::{Button, Icon, IconName, Segment, SegmentedControl};

pub struct Settings {
    previous_theme: String,
    on_page: bool,
}

impl Settings {
    pub fn new() -> Self {
        Self {
            previous_theme: crate::prefs::Prefs::load_theme().as_id().into(),
            on_page: false,
        }
    }

    pub fn set_on_page(&mut self, on_page: bool, cx: &mut Context<Self>) {
        if on_page && !self.on_page {
            self.previous_theme = color::preference(cx).as_id().into();
        }
        self.on_page = on_page;
    }
}

impl Render for Settings {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = color::preference(cx);
        let selected = theme.as_id();

        div()
            .id("settings")
            .flex()
            .flex_col()
            .items_start()
            .gap_8()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_3xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Settings"),
                    )
                    .child(
                        div()
                            .text_size(px(14.0))
                            .text_color(color::text::secondary(cx))
                            .child("Appearance and account for this device."),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_size(px(16.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Theme"),
                            )
                            .child(
                                div()
                                    .text_size(px(14.0))
                                    .text_color(color::text::secondary(cx))
                                    .child("By default, Angry Hub follows the system appearance."),
                            ),
                    )
                    .child(
                        SegmentedControl::new("theme")
                            .segments([
                                Segment::new("system", "System"),
                                Segment::new("light", "Light"),
                                Segment::new("dark", "Dark"),
                            ])
                            .selected(selected)
                            .previous_selected(self.previous_theme.clone())
                            .on_change(cx.listener(|this, id, _, cx| {
                                this.previous_theme = color::preference(cx).as_id().to_string();
                                color::set_preference(ThemePreference::from_id(id), cx);
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .items_start()
                    .gap_4()
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_size(px(16.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Account"),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .text_size(px(14.0))
                                    .text_color(color::text::secondary(cx))
                                    .child(
                                        "You are signed in with GitHub. Logging out ends the session on this device.",
                                    ),
                            ),
                    )
                    .child(
                        Button::new("logout", "Logout")
                            .icon(Icon::new(IconName::Logout).size(px(16.0)))
                            .destructive()
                            .on_click(cx.listener(|_, _, window, cx| confirm_logout(window, cx))),
                    ),
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
