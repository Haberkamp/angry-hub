use gpui::{AnyElement, App, Context, Entity, Render, Window, div, prelude::*, px};
use rooter::{Outlet, RouteContext, Router};

use crate::color;
use crate::ui::{Icon, IconName, Segment, SegmentedControl, Spinner};
use crate::views::settings::Settings;

pub struct Chrome {
    selected: String,
    previous_selected: String,
    refreshing: bool,
    on_settings: bool,
}

impl Chrome {
    pub fn new() -> Self {
        Self {
            selected: "prs".into(),
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

    fn sync_view(&mut self, selected: &str, on_settings: bool, cx: &mut Context<Self>) {
        let leaving_settings = self.on_settings && !on_settings;
        if self.on_settings != on_settings {
            self.on_settings = on_settings;
            cx.notify();
        }
        if on_settings {
            return;
        }
        if leaving_settings {
            self.selected = selected.to_string();
            self.previous_selected = selected.to_string();
            return;
        }
        if self.selected != selected {
            self.previous_selected = self.selected.clone();
            self.selected = selected.to_string();
        }
    }
}

pub fn main_layout(
    chrome: Entity<Chrome>,
    settings: Entity<Settings>,
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
    chrome.update(cx, |chrome, cx| chrome.sync_view(selected, is_settings, cx));
    settings.update(cx, |settings, cx| settings.set_on_page(is_settings, cx));
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
                                            .on_change(|id, window, cx| {
                                                let path = if id == "activity" {
                                                    "/activity"
                                                } else {
                                                    "/"
                                                };
                                                Router::navigate_window(window, cx, path);
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
        let rest_bg = color::surface::default(cx);
        let hover_bg = color::interaction::hovered(cx);
        let active_bg = color::interaction::pressed(cx);
        let icon_color = color::icon::subtle(cx);

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
                    .bg(rest_bg)
                    .hover(|this| this.bg(hover_bg))
                    .active(|this| this.bg(active_bg))
                    .cursor_pointer()
                    .rounded_full()
                    .child(Icon::new(icon).size(px(20.0)).color(icon_color))
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
