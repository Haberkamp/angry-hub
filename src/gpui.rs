use std::time::{Duration, Instant};

use gpui::prelude::*;
use gpui::{
    Bounds, ClickEvent, Context, IntoElement, MouseButton, Pixels, Render, Size, TitlebarOptions,
    Window, WindowBounds, WindowOptions, div, ease_in_out, point, px, rgb, size,
};
use gpui_base::{Button, StyledExt as _};

mod color;

const DEFAULT_WINDOW_SIZE: Size<Pixels> = size(px(800.0), px(600.0));
const RESTORE_ANIMATION: Duration = Duration::from_millis(250);

struct Root {
    count: i32,
    should_move: bool,
    resize_generation: u64,
}

impl Root {
    fn drag_area(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
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

    fn animate_restore_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let from = window.viewport_size();
        let to = DEFAULT_WINDOW_SIZE;
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

impl Render for Root {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();

        div()
            .v_flex()
            .size_full()
            .text_color(rgb(0xffffff))
            .child(self.drag_area(cx))
            .child(
                div()
                    .flex_1()
                    .v_flex()
                    .gap_2()
                    .items_center()
                    .justify_center()
                    .child(format!("Count: {}", self.count))
                    .child(
                        Button::new("increment")
                            .px_3()
                            .py_2()
                            .rounded(px(6.))
                            .bg(rgb(0x2563eb))
                            .text_color(rgb(0xffffff))
                            .on_click(move |_, _, cx| {
                                entity.update(cx, |this, cx| {
                                    this.count += 1;
                                    cx.notify();
                                });
                            })
                            .child("Increment"),
                    ),
            )
    }
}

fn main() {
    let app = gpui_platform::application();

    app.run(|cx| {
        gpui_base::init(cx);

        cx.spawn(async move |cx| {
            let options = cx.update(|cx| WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    DEFAULT_WINDOW_SIZE,
                    cx,
                ))),
                window_min_size: Some(size(px(480.0), px(600.0))),
                titlebar: Some(TitlebarOptions {
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(12.0), px(16.0))),
                    ..Default::default()
                }),
                app_owns_titlebar_drag: true,
                ..Default::default()
            });

            cx.open_window(options, |_window, cx| {
                cx.new(|_| Root {
                    count: 0,
                    should_move: false,
                    resize_generation: 0,
                })
            })
            .expect("Failed to open window");
        })
        .detach();
    });
}
