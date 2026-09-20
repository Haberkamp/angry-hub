use gpui::{AnyElement, App, Context, Entity, Render, Window, div, prelude::*, px};
use rooter::{Outlet, RouteContext, Router};

use crate::color;
use crate::ui::{Icon, IconName, Segment, SegmentedControl, Spinner};

pub struct Chrome {
    previous_selected: String,
    refreshing: bool,
    on_settings: bool,
}

impl Chrome {
    pub fn new() -> Self {
        Self {
            previous_selected: "prs".into(),
            refreshing: false,
            on_settings: false,
        }
    }

    pub fn set_refreshing(&mut self, refreshing: bool, cx: &mut Context<Self>) {
        if self.refreshing != refreshing {
            self.refreshing = refreshing;
            cx.notify();
        }
    }

    fn set_on_settings(&mut self, on_settings: bool, selected: &str, cx: &mut Context<Self>) {
        if self.on_settings && !on_settings {
            self.previous_selected = selected.to_string();
        }
        if self.on_settings != on_settings {
            self.on_settings = on_settings;
            cx.notify();
        }
    }
}

pub fn main_layout(
    chrome: Entity<Chrome>,
    route: RouteContext,
    _window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let path = route.path();
    let is_settings = path == "/settings";
    let selected = if path == "/activity" {
        "activity"
    } else {
        "prs"
    };
    chrome.update(cx, |chrome, cx| {
        chrome.set_on_settings(is_settings, selected, cx)
    });
    let previous_selected = chrome.read(cx).previous_selected.clone();

    div()
        .id("logged-in")
        .size_full()
        .flex()
        .flex_col()
        .relative()
        .child(
            div()
                .id("pr-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .items_center()
                .child(
                    div()
                        .id("pr-list")
                        .w_full()
                        .max_w(px(560.0))
                        .flex()
                        .flex_col()
                        .pt_16()
                        .pb_16()
                        .px_4()
                        .when(!is_settings, |this| {
                            this.child(
                                div()
                                    .id("view-switcher")
                                    .flex()
                                    .flex_none()
                                    .pl_3()
                                    .pb(px(32.0))
                                    .map(|mut this| {
                                        this.style().align_self = Some(gpui::AlignItems::FlexStart);
                                        this
                                    })
                                    .child(
                                        SegmentedControl::new("main-view")
                                            .segments([
                                                Segment::new("prs", "Pull Requests"),
                                                Segment::new("activity", "Activity"),
                                            ])
                                            .selected(selected)
                                            .previous_selected(previous_selected)
                                            .on_change({
                                                let chrome = chrome.clone();
                                                let from = selected.to_string();
                                                move |id, window, cx| {
                                                    chrome.update(cx, |chrome, cx| {
                                                        chrome.previous_selected = from.clone();
                                                        cx.notify();
                                                    });
                                                    let path = if id == "activity" {
                                                        "/activity"
                                                    } else {
                                                        "/"
                                                    };
                                                    Router::navigate_window(window, cx, path);
                                                }
                                            }),
                                    ),
                            )
                        })
                        .child(Outlet::new()),
                ),
        )
        .child(chrome.clone())
        .into_any_element()
}

impl Render for Chrome {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let on_settings = self.on_settings;
        let icon = if on_settings {
            IconName::Close
        } else {
            IconName::Settings
        };
        let control_id = if on_settings {
            "close-settings"
        } else {
            "open-settings"
        };

        div()
            .id("top-right")
            .absolute()
            .top_4()
            .right_4()
            .flex()
            .items_center()
            .gap_2()
            .when(self.refreshing, |this| {
                this.child(div().id("refreshing").child(Spinner::new("refreshing")))
            })
            .child(
                div()
                    .id(control_id)
                    .size(px(36.0))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(color::gray::s1())
                    .hover(|this| this.bg(color::gray::s3()))
                    .active(|this| this.bg(color::gray::s4()))
                    .cursor_pointer()
                    .rounded_full()
                    .child(Icon::new(icon).size(px(20.0)).color(color::gray::s9()))
                    .on_click(move |_, window, cx| {
                        if on_settings {
                            Router::back_window(window, cx);
                        } else {
                            Router::navigate_window(window, cx, "/settings");
                        }
                    }),
            )
    }
}
