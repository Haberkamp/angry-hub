use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::prelude::*;
use gpui::{
    Anchor, Animation, AnimationExt as _, App, Bounds, IntoElement, MouseButton, Pixels,
    RenderOnce, SharedString, Window, div, ease_out_quint, px, rgb, svg,
};
use gpui_base::{ElementExt as _, Popup};

use crate::color;
use crate::icon;

const PANEL_ANIMATION: Duration = Duration::from_millis(180);

#[derive(Clone)]
pub struct MenuItem {
    pub id: SharedString,
    pub label: SharedString,
}

impl MenuItem {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}

#[derive(Clone)]
struct MenuPresence {
    shown: bool,
    closing_at: Option<Instant>,
}

impl Default for MenuPresence {
    fn default() -> Self {
        Self {
            shown: false,
            closing_at: None,
        }
    }
}

#[derive(IntoElement)]
pub struct Dropdown {
    id: SharedString,
    open: bool,
    disabled: bool,
    skip_exit_animation: bool,
    items: Vec<MenuItem>,
    on_toggle: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_dismiss: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_select: Option<Arc<dyn Fn(&str, &mut Window, &mut App) + 'static>>,
}

impl Dropdown {
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            open: false,
            disabled: false,
            skip_exit_animation: false,
            items: Vec::new(),
            on_toggle: None,
            on_dismiss: None,
            on_select: None,
        }
    }

    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// While an action is in flight, every item ignores clicks.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Drop the panel immediately instead of playing the exit animation.
    pub fn skip_exit_animation(mut self, skip: bool) -> Self {
        self.skip_exit_animation = skip;
        self
    }

    pub fn items(mut self, items: Vec<MenuItem>) -> Self {
        self.items = items;
        self
    }

    pub fn on_toggle(mut self, listener: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Box::new(listener));
        self
    }

    pub fn on_dismiss(mut self, listener: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_dismiss = Some(Box::new(listener));
        self
    }

    pub fn on_select(mut self, listener: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Arc::new(listener));
        self
    }
}

impl RenderOnce for Dropdown {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let open = self.open;
        let disabled = self.disabled;
        let skip_exit_animation = self.skip_exit_animation;
        let presence = window.use_keyed_state(self.id.clone(), cx, |_, _| MenuPresence::default());
        let mut shown = presence.read(cx).clone();
        let visible = if open {
            shown.shown = true;
            shown.closing_at = None;
            true
        } else if skip_exit_animation {
            shown.shown = false;
            shown.closing_at = None;
            false
        } else if shown.shown {
            let started = shown.closing_at.get_or_insert_with(Instant::now);
            if started.elapsed() < PANEL_ANIMATION {
                window.request_animation_frame();
                true
            } else {
                shown.shown = false;
                shown.closing_at = None;
                false
            }
        } else {
            false
        };
        presence.update(cx, |state, _| *state = shown);

        let trigger_bounds = Rc::new(Cell::new(Bounds::<Pixels>::default()));
        let mut trigger = div()
            .id(SharedString::from(format!("{}-trigger", self.id)))
            .size(px(28.))
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .rounded_full()
            .when(open, |this| this.bg(rgb(0x313131)))
            .hover(|this| this.bg(rgb(0x313131)))
            .when(!open, |this| {
                this.opacity(0.)
                    .group_hover("pr-row", |style| style.opacity(1.))
            })
            .on_prepaint({
                let trigger_bounds = trigger_bounds.clone();
                move |bounds, _, _| trigger_bounds.set(bounds)
            })
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                svg()
                    .data(icon::ELLIPSIS)
                    .size(px(14.))
                    .text_color(rgb(0xB4B4B4)),
            );
        if let Some(on_toggle) = self.on_toggle {
            trigger = trigger.on_click(move |_, window, cx| on_toggle(window, cx));
        }

        let animation_id = SharedString::from(format!(
            "{}-{}",
            self.id,
            if open { "enter" } else { "exit" }
        ));
        let on_select = self.on_select;
        let on_dismiss = self.on_dismiss;
        let panel = div()
            .min_w(px(180.))
            .flex()
            .flex_col()
            .p_1()
            .bg(color::gray(2, cx))
            .border_1()
            .border_color(color::gray(6, cx))
            .rounded(px(9.))
            .shadow_md()
            .occlude()
            .when_some(on_dismiss, |panel, on_dismiss| {
                let trigger_bounds = trigger_bounds.clone();
                panel.on_mouse_down_out(move |_, window, cx| {
                    // The trigger sits outside the panel, so this also fires
                    // for the click that should close it. Leave that to toggle.
                    if trigger_bounds.get().contains(&window.mouse_position()) {
                        return;
                    }
                    on_dismiss(window, cx);
                })
            })
            .children(self.items.into_iter().map(|item| {
                let item_id = item.id.clone();
                div()
                    .id(item.id)
                    .px_3()
                    .py_1()
                    .rounded(px(4.))
                    .flex()
                    .items_center()
                    .text_size(px(13.))
                    .whitespace_nowrap()
                    .text_color(if disabled {
                        rgb(0x6E6E6E)
                    } else {
                        rgb(0xFFFFFF)
                    })
                    .when(!disabled, |row| {
                        row.hover(|row| row.bg(rgb(0x313131))).when_some(
                            on_select.clone(),
                            |row, on_select| {
                                let item_id = item_id.clone();
                                row.on_click(move |_, window, cx| {
                                    cx.stop_propagation();
                                    on_select(item_id.as_ref(), window, cx);
                                })
                            },
                        )
                    })
                    .child(item.label)
            }))
            .with_animation(
                animation_id,
                Animation::new(PANEL_ANIMATION).with_easing(ease_out_quint()),
                move |panel, delta| {
                    let t = if open { delta } else { 1.0 - delta };
                    // Starts 8px above the resting spot and moves down into it.
                    panel.opacity(t).mt(px((t - 1.0) * 8.))
                },
            );

        Popup::new(self.id, trigger)
            .anchor(Anchor::TopRight)
            .flex_none()
            .when(visible, |popup| {
                popup.content(div().mt(px(4.)).child(panel))
            })
    }
}
