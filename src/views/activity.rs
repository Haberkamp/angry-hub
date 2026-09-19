use std::sync::Arc;
use std::time::Duration;

use gpui::{Context, Render, Subscription, Timer, Window, div, prelude::*, px, rgb};

use crate::datasource::code_host;
use crate::model;
use crate::session::{self, Session};
use crate::ui::{ActivityKindIcon, Avatar, Button, Spinner};

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
                    .text_color(rgb(0xff6666))
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
                            let headline = format!("{} {}", item.actor, item.kind.label());
                            let avatar = item.avatar_url.clone();
                            div()
                                .id(("activity", ix))
                                .flex()
                                .flex_col()
                                .gap_1()
                                .py_2()
                                .px_3()
                                .rounded_md()
                                .hover(|this| this.bg(rgb(0x2a2a2a)))
                                .cursor_pointer()
                                .on_click(move |_, _window, cx| cx.open_url(&url))
                                .child(
                                    div()
                                        .flex()
                                        .gap_2()
                                        .items_center()
                                        .text_color(rgb(0x8b949e))
                                        .child(ActivityKindIcon::new(item.kind))
                                        .child(Avatar::new(avatar).size(px(20.0)))
                                        .child(div().child(headline))
                                        .child(div().child(item.repo.clone())),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .gap_2()
                                        .items_center()
                                        .child(div().child(item.pr_title.clone())),
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
