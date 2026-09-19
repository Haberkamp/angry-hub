use std::sync::Arc;
use std::time::Duration;

use gpui::{
    Context, Entity, InteractiveElement, IntoElement, ParentElement, Render, Styled, Subscription,
    Timer, Window, div, prelude::*, px, rgb,
};

use crate::code_host;
use crate::layout::main_layout;
use crate::model;
use crate::route::Route;
use crate::router::Router;
use crate::session::Session;
use crate::ui::{Avatar, Button, Spinner};

#[derive(Clone)]
enum ActivityState {
    Loading,
    Loaded(Vec<model::ActivityItem>),
    Failed(Arc<str>),
}

pub struct ActivityView {
    activity: ActivityState,
    fetch_in_flight: bool,
    session: Entity<Session>,
    router: Entity<Router<Route>>,
    _subscriptions: Vec<Subscription>,
}

impl ActivityView {
    pub fn new(
        session: Entity<Session>,
        router: Entity<Router<Route>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut _subscriptions = Vec::new();
        _subscriptions.push(cx.observe(&router, |_, _, cx| cx.notify()));
        _subscriptions.push(cx.observe(&session, |this, session, cx| {
            if session.read(cx).logged_in {
                this.fetch(cx);
            }
        }));
        _subscriptions.push(cx.observe_window_activation(window, |this, window, cx| {
            if window.is_window_active() && this.session.read(cx).logged_in {
                this.fetch(cx);
            }
        }));
        let mut view = Self {
            activity: ActivityState::Loading,
            fetch_in_flight: false,
            session,
            router,
            _subscriptions,
        };
        view.poll(cx);
        if view.session.read(cx).logged_in {
            view.fetch(cx);
        }
        view
    }

    fn poll(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                Timer::after(Duration::from_secs(30)).await;
                this.update(cx, |this, cx| {
                    if this.session.read(cx).logged_in {
                        this.fetch(cx);
                    }
                })
                .ok();
            }
        })
        .detach();
    }

    fn fetch(&mut self, cx: &mut Context<Self>) {
        if self.fetch_in_flight {
            return;
        }
        self.fetch_in_flight = true;
        let host = code_host();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { host.my_pr_activity() })
                .await;
            this.update(cx, |this, _cx| {
                this.fetch_in_flight = false;
                this.activity = match result {
                    Ok(items) => ActivityState::Loaded(items),
                    Err(e) => ActivityState::Failed(e.message.into()),
                };
            })
            .ok();
            this.update(cx, |_, cx| cx.notify()).ok();
        })
        .detach();
    }
}

impl Render for ActivityView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = match &self.activity {
            ActivityState::Loading => div()
                .id("activity")
                .flex()
                .flex_1()
                .items_center()
                .justify_center()
                .child(Spinner::new("activity-loading"))
                .into_any_element(),
            ActivityState::Failed(e) => div()
                .id("activity")
                .flex()
                .flex_col()
                .gap_2()
                .child(div().text_color(rgb(0xff6666)).child(e.to_string()))
                .child(
                    Button::new("retry-activity", "Retry")
                        .on_click(cx.listener(|this, _, _, cx| this.fetch(cx))),
                )
                .into_any_element(),
            ActivityState::Loaded(items) => {
                if items.is_empty() {
                    div()
                        .id("activity")
                        .child("No activity yet")
                        .into_any_element()
                } else {
                    div()
                        .id("activity")
                        .flex()
                        .flex_col()
                        .children(items.iter().enumerate().map(|(ix, item)| {
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
                                        .child(crate::ui::ActivityKindIcon::new(item.kind))
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
                        }))
                        .into_any_element()
                }
            }
        };
        main_layout(
            self.router.clone(),
            self.session.clone(),
            false,
            body,
            cx,
        )
    }
}
