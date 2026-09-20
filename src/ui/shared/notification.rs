use std::time::{Duration, Instant};

use crate::color;
use gpui::{
    AlignContent, AlignItems, Animation, AnimationExt as _, AnyElement, App, Bounds, Context,
    Display, Element, ElementId, FlexDirection, Global, GlobalElementId, InspectorElementId,
    InteractiveElement, IntoElement, LayoutId, Overflow, ParentElement, Pixels, Point, Render,
    SharedString, Size, Style, Styled, Timer, Window, div, ease_out_quint, prelude::*, px,
};

const SHOW_DURATION: Duration = Duration::from_millis(2500);
const PANEL_ANIMATION: Duration = Duration::from_millis(180);
const PANEL_SLIDE: f32 = 12.0;
const FALLBACK_TOAST_HEIGHT: f32 = 36.0;
const JUMP_DURATION: Duration = Duration::from_millis(240);
const JUMP_AMPLITUDE: f32 = 6.0;

#[derive(Clone)]
pub struct Notification {
    message: SharedString,
    key: Option<SharedString>,
}

impl Notification {
    pub fn new() -> Self {
        Self {
            message: SharedString::default(),
            key: None,
        }
    }

    pub fn message(mut self, message: impl Into<SharedString>) -> Self {
        self.message = message.into();
        self
    }

    pub fn key(mut self, key: impl Into<SharedString>) -> Self {
        self.key = Some(key.into());
        self
    }
}

impl From<&str> for Notification {
    fn from(message: &str) -> Self {
        Notification::new().message(SharedString::from(message.to_owned()))
    }
}

impl From<String> for Notification {
    fn from(message: String) -> Self {
        Notification::new().message(message)
    }
}

impl From<SharedString> for Notification {
    fn from(message: SharedString) -> Self {
        Notification::new().message(message)
    }
}

struct ActiveNotification {
    id: u64,
    key: Option<SharedString>,
    message: SharedString,
    generation: u64,
    closing: bool,
    created_at: Instant,
    closing_since: Option<Instant>,
    jump_at: Option<Instant>,
}

pub struct NotificationList {
    next_id: u64,
    items: Vec<ActiveNotification>,
}

struct NotificationCenter(gpui::Entity<NotificationList>);

impl Global for NotificationCenter {}

impl NotificationList {
    pub fn init(cx: &mut App) -> gpui::Entity<Self> {
        let list = cx.new(|_| Self {
            next_id: 0,
            items: Vec::new(),
        });
        cx.set_global(NotificationCenter(list.clone()));
        list
    }

    fn push(&mut self, notification: Notification, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(existing) = self
            .items
            .iter_mut()
            .find(|item| notification.key.is_some() && item.key == notification.key)
        {
            existing.message = notification.message;
            existing.generation += 1;
            existing.closing = false;
            existing.closing_since = None;
            existing.jump_at = Some(Instant::now());
            let id = existing.id;
            let generation = existing.generation;
            cx.notify();
            self.schedule_dismiss(id, generation, window, cx);
            return;
        }

        let id = self.next_id;
        self.next_id += 1;
        self.items.push(ActiveNotification {
            id,
            key: notification.key,
            message: notification.message,
            generation: 0,
            closing: false,
            created_at: Instant::now(),
            closing_since: None,
            jump_at: None,
        });
        cx.notify();
        self.schedule_dismiss(id, 0, window, cx);
    }

    fn schedule_dismiss(
        &mut self,
        id: u64,
        generation: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.spawn_in(window, async move |this, cx| {
            Timer::after(SHOW_DURATION).await;
            this.update(cx, |this, cx| this.begin_dismiss(id, generation, cx))
                .ok();
            Timer::after(PANEL_ANIMATION).await;
            this.update(cx, |this, cx| this.remove(id, generation, cx))
                .ok();
        })
        .detach();
    }

    fn begin_dismiss(&mut self, id: u64, generation: u64, cx: &mut Context<Self>) {
        if let Some(item) = self.items.iter_mut().find(|item| item.id == id) {
            if item.generation != generation {
                return;
            }
            item.closing = true;
            item.closing_since = Some(Instant::now());
            cx.notify();
        }
    }

    fn remove(&mut self, id: u64, generation: u64, cx: &mut Context<Self>) {
        self.items
            .retain(|item| item.id != id || item.generation != generation);
        cx.notify();
    }
}

pub trait WindowExt {
    fn push_notification(&mut self, notification: impl Into<Notification>, cx: &mut App);
}

impl WindowExt for Window {
    fn push_notification(&mut self, notification: impl Into<Notification>, cx: &mut App) {
        let list = cx.global::<NotificationCenter>().0.clone();
        list.update(cx, |list, cx| {
            list.push(notification.into(), self, cx);
        });
    }
}

impl Render for NotificationList {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let toast_bg = color::surface::inverse(cx);
        let toast_fg = color::text::inverse(cx);
        div()
            .id("notification-layer")
            .absolute()
            .bottom(px(16.0))
            .left_0()
            .right_0()
            .flex()
            .flex_col()
            .items_center()
            .gap_2()
            .children(self.items.iter().map(|item| {
                let open = item.closing_since.is_none();
                let animation_id = SharedString::from(format!(
                    "notification-{}-{}",
                    item.id,
                    if open { "enter" } else { "exit" }
                ));
                StackToast {
                    id: item.id,
                    created_at: item.created_at,
                    closing_since: item.closing_since,
                    jump_at: item.jump_at,
                    child: div()
                        .id(SharedString::from(format!("notification-{}", item.id)))
                        .flex_shrink_0()
                        .px_3()
                        .py_2()
                        .rounded(px(8.0))
                        .bg(toast_bg)
                        .text_color(toast_fg)
                        .text_sm()
                        .shadow_md()
                        .occlude()
                        .relative()
                        .child(item.message.clone())
                        .with_animation(
                            animation_id,
                            Animation::new(PANEL_ANIMATION).with_easing(ease_out_quint()),
                            move |this, delta| {
                                let t = if open { delta } else { 1.0 - delta };
                                this.opacity(t).top(px((1.0 - t) * PANEL_SLIDE))
                            },
                        )
                        .into_any_element(),
                }
            }))
    }
}

struct StackToast {
    id: u64,
    created_at: Instant,
    closing_since: Option<Instant>,
    jump_at: Option<Instant>,
    child: AnyElement,
}

struct StackToastState {
    natural_height: Option<Pixels>,
}

struct StackToastLayout {
    child: AnyElement,
    child_layout_id: LayoutId,
}

impl IntoElement for StackToast {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for StackToast {
    type RequestLayoutState = StackToastLayout;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some(ElementId::from(SharedString::from(format!(
            "notification-stack-{}",
            self.id
        ))))
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        if toast_still_animating(self.created_at, self.closing_since, self.jump_at) {
            window.request_animation_frame();
        }

        let child_layout_id = self.child.request_layout(window, cx);
        let child = std::mem::replace(&mut self.child, div().into_any_element());
        let created_at = self.created_at;
        let closing_since = self.closing_since;

        let layout_id = window.with_element_state(global_id.unwrap(), |state, window| {
            let state = state.unwrap_or(StackToastState {
                natural_height: None,
            });
            let t = size_factor(created_at, closing_since);
            let height = state.natural_height.unwrap_or(px(FALLBACK_TOAST_HEIGHT)) * t;
            let layout_id = window.request_layout(
                Style {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    justify_content: Some(AlignContent::FlexStart),
                    align_items: Some(AlignItems::Center),
                    overflow: Point {
                        x: Overflow::Visible,
                        y: Overflow::Hidden,
                    },
                    size: Size {
                        width: gpui::Length::Auto,
                        height: height.into(),
                    },
                    ..Default::default()
                },
                [child_layout_id],
                cx,
            );
            (layout_id, state)
        });

        (
            layout_id,
            StackToastLayout {
                child,
                child_layout_id,
            },
        )
    }

    fn prepaint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let child_bounds = window.layout_bounds(layout.child_layout_id);
        window.with_element_state(global_id.unwrap(), |state, window| {
            let mut state = state.unwrap_or(StackToastState {
                natural_height: None,
            });
            let height = child_bounds.size.height;
            if height > px(0.5) {
                state.natural_height = Some(
                    state
                        .natural_height
                        .map_or(height, |previous| previous.max(height)),
                );
            }
            let offset = Point::new(px(0.0), px(jump_offset(self.jump_at)));
            window.with_element_offset(offset, |window| {
                layout.child.prepaint(window, cx);
            });
            ((), state)
        });
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        layout: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        layout.child.paint(window, cx);
    }
}

fn size_factor(created_at: Instant, closing_since: Option<Instant>) -> f32 {
    if let Some(since) = closing_since {
        1.0 - eased_progress(since, PANEL_ANIMATION)
    } else {
        eased_progress(created_at, PANEL_ANIMATION)
    }
}

fn toast_still_animating(
    created_at: Instant,
    closing_since: Option<Instant>,
    jump_at: Option<Instant>,
) -> bool {
    let t = size_factor(created_at, closing_since);
    t < 1.0 || jump_offset(jump_at) != 0.0
}

fn jump_offset(jump_at: Option<Instant>) -> f32 {
    let Some(start) = jump_at else {
        return 0.0;
    };
    let t = (start.elapsed().as_secs_f32() / JUMP_DURATION.as_secs_f32()).clamp(0.0, 1.0);
    if t >= 1.0 {
        return 0.0;
    }
    -(t * std::f32::consts::PI).sin() * JUMP_AMPLITUDE
}

fn eased_progress(start: Instant, duration: Duration) -> f32 {
    let t = (start.elapsed().as_secs_f32() / duration.as_secs_f32()).clamp(0.0, 1.0);
    ease_out_quint()(t)
}
