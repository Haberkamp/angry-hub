use std::time::{Duration, Instant};

use gpui::prelude::*;
use gpui::{
    Animation, AnimationExt as _, AnyElement, App, IntoElement, SharedString, div, ease_out_quint,
    px,
};

use crate::color;

const SHOW_FOR: Duration = Duration::from_millis(2500);
const ANIMATION: Duration = Duration::from_millis(180);
const SLIDE: f32 = 12.;
const HEIGHT: f32 = 36.;
const JUMP: Duration = Duration::from_millis(240);
const JUMP_AMPLITUDE: f32 = 6.;

struct Toast {
    key: SharedString,
    message: SharedString,
    generation: u64,
    created_at: Instant,
    closing_since: Option<Instant>,
    jump_at: Option<Instant>,
}

/// One toast at a time per key. A repeat jumps and resets the timer instead of
/// stacking another copy.
pub struct Toaster {
    toasts: Vec<Toast>,
    next_generation: u64,
    ticking: bool,
}

impl Default for Toaster {
    fn default() -> Self {
        Self {
            toasts: Vec::new(),
            next_generation: 0,
            ticking: false,
        }
    }
}

impl Toaster {
    pub fn push(&mut self, key: impl Into<SharedString>, message: impl Into<SharedString>) {
        let key = key.into();
        let message = message.into();
        if let Some(existing) = self.toasts.iter_mut().find(|toast| toast.key == key) {
            existing.message = message;
            existing.generation = existing.generation.wrapping_add(1);
            existing.created_at = Instant::now();
            existing.closing_since = None;
            existing.jump_at = Some(Instant::now());
            return;
        }
        self.next_generation = self.next_generation.wrapping_add(1);
        self.toasts.push(Toast {
            key,
            message,
            generation: self.next_generation,
            created_at: Instant::now(),
            closing_since: None,
            jump_at: None,
        });
    }

    pub fn is_ticking(&self) -> bool {
        self.ticking
    }

    pub fn set_ticking(&mut self, ticking: bool) {
        self.ticking = ticking;
    }

    /// Advances show, exit, and jump timers. Returns whether another tick is needed.
    pub fn tick(&mut self, now: Instant) -> bool {
        for toast in &mut self.toasts {
            if toast.closing_since.is_none() && now.duration_since(toast.created_at) >= SHOW_FOR {
                toast.closing_since = Some(now);
            }
            if toast.jump_at.is_some() && jump_offset(toast.jump_at) == 0. {
                toast.jump_at = None;
            }
        }
        self.toasts.retain(|toast| {
            toast
                .closing_since
                .is_none_or(|since| now.duration_since(since) < ANIMATION)
        });
        !self.toasts.is_empty()
    }

    pub fn render(&self, cx: &App) -> AnyElement {
        div()
            .absolute()
            .bottom(px(16. - SLIDE))
            .left_0()
            .right_0()
            .h(px(HEIGHT + SLIDE))
            .children(self.toasts.iter().map(|toast| toast.render(cx)))
            .into_any_element()
    }
}

impl Toast {
    fn render(&self, cx: &App) -> AnyElement {
        let open = self.closing_since.is_none();
        let animation_id = SharedString::from(if open {
            format!("toast-{}-enter", self.key)
        } else {
            format!("toast-{}-exit-{}", self.key, self.generation)
        });
        let jump = jump_offset(self.jump_at);
        let message = self.message.clone();
        div()
            .w_full()
            .flex()
            .flex_col()
            .items_center()
            .child(
                div()
                    .px_3()
                    .py_2()
                    .rounded(px(8.))
                    .bg(color::gray(12, cx))
                    .text_color(color::gray(1, cx))
                    .text_sm()
                    .whitespace_nowrap()
                    .flex()
                    .items_center()
                    .justify_center()
                    .shadow_md()
                    .child(message),
            )
            .with_animation(
                animation_id,
                Animation::new(ANIMATION).with_easing(ease_out_quint()),
                move |layer, delta| {
                    let t = if open { delta } else { 1. - delta };
                    layer.opacity(t).mt(px((1. - t) * SLIDE + jump))
                },
            )
            .into_any_element()
    }
}

fn jump_offset(jump_at: Option<Instant>) -> f32 {
    let Some(start) = jump_at else {
        return 0.;
    };
    let t = (start.elapsed().as_secs_f32() / JUMP.as_secs_f32()).clamp(0., 1.);
    if t >= 1. {
        0.
    } else {
        -(t * std::f32::consts::PI).sin() * JUMP_AMPLITUDE
    }
}
