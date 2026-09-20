use std::sync::Arc;
use std::time::Duration;

use gpui::{Context, Render, Subscription, Timer, Window, div, prelude::*, px};

use crate::color;
use crate::datasource::code_host;
use crate::model;
use crate::session::{self, Session};
use crate::ui::{
    ActivityKindIcon, Avatar, Button, Spinner, list_text_max_width, measure_line, truncate_line,
};

enum ActivityState {
    Loading,
    Loaded(Vec<model::ActivityItem>),
    Failed(Arc<str>),
}

pub struct Activity {
    activity: ActivityState,
    fetch_in_flight: bool,
    _activation_subscription: Option<Subscription>,
}

impl Activity {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut view = Self {
            activity: ActivityState::Loading,
            fetch_in_flight: false,
            _activation_subscription: None,
        };
        view._activation_subscription =
            Some(cx.observe_window_activation(window, |this, window, cx| {
                if window.is_window_active() && cx.global::<Session>().logged_in {
                    this.fetch_activity(window, cx);
                }
            }));
        if cx.global::<Session>().logged_in {
            view.fetch_activity(window, cx);
        }
        view.poll(window, cx);
        view
    }

    fn poll(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.spawn_in(window, async move |this, cx| {
            loop {
                Timer::after(Duration::from_secs(30)).await;
                this.update_in(cx, |this, window, cx| {
                    if cx.global::<Session>().logged_in {
                        this.fetch_activity(window, cx);
                    }
                })
                .ok();
            }
        })
        .detach();
    }

    pub(crate) fn refresh_activity(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.fetch_activity(window, cx);
    }

    fn fetch_activity(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.fetch_in_flight || !cx.global::<Session>().logged_in {
            return;
        }
        self.fetch_in_flight = true;
        let host = code_host();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { host.my_pr_activity() })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.fetch_in_flight = false;
                match result {
                    Ok(items) => this.activity = ActivityState::Loaded(items),
                    Err(e) if e.is_session_ended() => {
                        session::force_logout(window, cx);
                    }
                    Err(e) => this.activity = ActivityState::Failed(e.message.into()),
                }
            })
            .ok();
            this.update(cx, |_, cx| cx.notify()).ok();
        })
        .detach();
    }
}

impl Render for Activity {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let row_hover = color::interaction::hovered(cx);
        let headline_color = color::text::primary(cx);
        let meta = color::text::secondary(cx);
        if !cx.global::<Session>().logged_in {
            self.activity = ActivityState::Loading;
            self.fetch_in_flight = false;
        } else if matches!(self.activity, ActivityState::Loading) && !self.fetch_in_flight {
            self.fetch_activity(window, cx);
        }

        let content: Vec<_> = match &self.activity {
            ActivityState::Loading => vec![
                div()
                    .flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .child(Spinner::new("activity-loading"))
                    .into_any_element(),
            ],
            ActivityState::Failed(e) => vec![
                div()
                    .text_color(color::text::danger(cx))
                    .child(e.to_string())
                    .into_any_element(),
                Button::new("retry-activity", "Retry")
                    .on_click(cx.listener(|this, _, window, cx| this.fetch_activity(window, cx)))
                    .into_any_element(),
            ],
            ActivityState::Loaded(items) => {
                if items.is_empty() {
                    vec![div().child("No activity yet").into_any_element()]
                } else {
                    items
                        .iter()
                        .enumerate()
                        .map(|(ix, item)| {
                            let url = item.url.clone();
                            let number = format!("#{}", item.number);
                            let show_avatar = matches!(
                                item.kind,
                                model::ActivityKind::Comment
                                    | model::ActivityKind::Approved
                                    | model::ActivityKind::ChangesRequested
                            );
                            let prefix = match item.kind {
                                model::ActivityKind::Merged => "Merged ".to_string(),
                                model::ActivityKind::Closed => "Closed ".to_string(),
                                model::ActivityKind::Reopened => "Reopened ".to_string(),
                                model::ActivityKind::Comment => {
                                    format!("{} commented on ", item.actor)
                                }
                                model::ActivityKind::Approved => {
                                    format!("{} approved ", item.actor)
                                }
                                model::ActivityKind::ChangesRequested => {
                                    format!("{} requested changes on ", item.actor)
                                }
                            };
                            let extra_reserved = px(20.0) // status icon
                                + px(8.0) // gap after icon
                                + if show_avatar {
                                    px(16.0) + px(4.0)
                                } else {
                                    px(0.0)
                                };
                            let available = list_text_max_width(window, extra_reserved);
                            let prefix_width = measure_line(&prefix, window);
                            let quoted_max = (available - prefix_width).max(px(24.0));
                            let quoted_title = truncate_line(
                                format!("\"{}\"", item.pr_title.trim()),
                                quoted_max,
                                window,
                            );
                            let headline = format!("{prefix}{quoted_title}");
                            div()
                                .id(("activity", ix))
                                .w_full()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .py_2()
                                .px_3()
                                .rounded_md()
                                .hover(|this| this.bg(row_hover))
                                .on_click(move |_, _window, cx| cx.open_url(&url))
                                .child(
                                    div()
                                        .flex()
                                        .gap_2()
                                        .items_center()
                                        .min_w_0()
                                        .child(ActivityKindIcon::new(item.kind))
                                        .child(
                                            div()
                                                .flex_1()
                                                .flex()
                                                .items_center()
                                                .gap(px(4.0))
                                                .min_w_0()
                                                .overflow_hidden()
                                                .text_color(headline_color)
                                                .when(show_avatar, |this| {
                                                    this.child(
                                                        Avatar::new(item.avatar_url.clone())
                                                            .size(px(16.0)),
                                                    )
                                                })
                                                .child(
                                                    div()
                                                        .min_w_0()
                                                        .overflow_hidden()
                                                        .whitespace_nowrap()
                                                        .child(headline),
                                                ),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(4.0))
                                        .ml(px(28.0))
                                        .text_size(px(12.0))
                                        .text_color(meta)
                                        .child(item.repo.clone())
                                        .child(number),
                                )
                        })
                        .map(|el| el.into_any_element())
                        .collect()
                }
            }
        };

        div()
            .id("activity-page")
            .flex()
            .flex_col()
            .when(matches!(self.activity, ActivityState::Loading), |this| {
                this.flex_1()
            })
            .children(content)
    }
}
