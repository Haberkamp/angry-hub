use gpui::{AnyElement, App, Context, Entity, PromptLevel, Render, Window, div, prelude::*, px};
use rooter::{Outlet, RouteContext, Router};

use crate::datasource::code_host;
use crate::session::Session;
use crate::ui::{Button, Icon, IconName, Segment, SegmentedControl, Spinner};

pub struct Chrome {
    previous_selected: String,
    refreshing: bool,
}

impl Chrome {
    pub fn new() -> Self {
        Self {
            previous_selected: "prs".into(),
            refreshing: false,
        }
    }

    pub fn set_refreshing(&mut self, refreshing: bool, cx: &mut Context<Self>) {
        if self.refreshing != refreshing {
            self.refreshing = refreshing;
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
    let selected = if route.path() == "/activity" {
        "activity"
    } else {
        "prs"
    };
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
                        .child(
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
                        .child(Outlet::new()),
                ),
        )
        .child(chrome.clone())
        .into_any_element()
}

impl Render for Chrome {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                Button::new("logout", "")
                    .icon(Icon::new(IconName::Logout).size(px(20.0)))
                    .tertiary()
                    .on_click(cx.listener(|_, _, window, cx| logout(window, cx))),
            )
    }
}

fn logout(window: &mut Window, cx: &mut App) {
    let answer = window.prompt(
        PromptLevel::Warning,
        "Are you sure you want to log out?",
        None,
        &["Logout", "Cancel"],
        cx,
    );
    let host = code_host();
    let window = window.window_handle();
    cx.spawn(async move |cx| {
        if answer.await == Ok(0) {
            host.logout();
            window
                .update(cx, |_, window, cx| {
                    cx.global_mut::<Session>().logged_in = false;
                    Router::navigate_window(window, cx, "/login");
                })
                .ok();
        }
    })
    .detach();
}
