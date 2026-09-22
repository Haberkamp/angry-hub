use std::time::{Duration, Instant};

use gpui::prelude::*;
use gpui::{
    ClickEvent, Context, IntoElement, MouseButton, Pixels, Render, Size, Window, div, ease_in_out,
    px, size,
};

const RESTORE_ANIMATION: Duration = Duration::from_millis(250);

pub struct Chrome {
    default_size: Size<Pixels>,
    should_move: bool,
    resize_generation: u64,
}

impl Chrome {
    pub fn new(default_size: Size<Pixels>) -> Self {
        Self {
            default_size,
            should_move: false,
            resize_generation: 0,
        }
    }

    fn animate_restore_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let from = window.viewport_size();
        let to = self.default_size;
        if from == to {
            return;
        }

        self.resize_generation = self.resize_generation.wrapping_add(1);
        let generation = self.resize_generation;
        if cx.reduce_motion() {
            window.resize(to);
            return;
        }

        let started = Instant::now();
        cx.on_next_frame(window, move |this, window, cx| {
            this.step_restore(from, to, started, generation, window, cx);
        });
    }

    fn step_restore(
        &mut self,
        from: Size<Pixels>,
        to: Size<Pixels>,
        started: Instant,
        generation: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.resize_generation != generation {
            return;
        }

        let t = (started.elapsed().as_secs_f32() / RESTORE_ANIMATION.as_secs_f32()).min(1.0);
        let e = ease_in_out(t);
        window.resize(size(
            from.width + (to.width - from.width) * e,
            from.height + (to.height - from.height) * e,
        ));

        if t < 1.0 {
            cx.on_next_frame(window, move |this, window, cx| {
                this.step_restore(from, to, started, generation, window, cx);
            });
        }
    }
}

impl Render for Chrome {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("drag-area")
            .w_full()
            .h(px(28.0))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, _| {
                    this.should_move = true;
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| {
                    this.should_move = false;
                }),
            )
            .on_mouse_move(cx.listener(|this, _, window, _| {
                if this.should_move {
                    this.should_move = false;
                    this.resize_generation = this.resize_generation.wrapping_add(1);
                    window.start_window_move();
                }
            }))
            .on_click(cx.listener(|this, event: &ClickEvent, window, cx| {
                if event.click_count() == 2 {
                    this.should_move = false;
                    this.animate_restore_window(window, cx);
                }
            }))
    }
}
