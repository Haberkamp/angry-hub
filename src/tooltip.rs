use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::prelude::*;
use gpui::{
    Animation, AnimationExt as _, AnyElement, App, Bounds, Element, ElementId, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, Pixels, SharedString, Window, div, ease_out_quint,
    px, svg,
};
use gpui_base::Tooltip as BaseTooltip;

use crate::icon;

const SHOW_DELAY: Duration = Duration::from_millis(200);
const PANEL_ANIMATION: Duration = Duration::from_millis(180);
const PANEL_SLIDE: f32 = 8.0;
const OFFSET: f32 = 4.0;

pub struct Tooltip {
    id: SharedString,
    label: SharedString,
    child: Option<AnyElement>,
}

impl Tooltip {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            child: None,
        }
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.child = Some(child.into_element().into_any_element());
        self
    }
}

impl IntoElement for Tooltip {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

struct PresenceState {
    hovered: Rc<Cell<bool>>,
    shown: bool,
    opening_since: Option<Instant>,
    closing_since: Option<Instant>,
}

impl Element for Tooltip {
    type RequestLayoutState = AnyElement;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone().into())
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        global_id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let label = self.label.clone();
        let host_id = self.id.clone();
        window.with_element_state(global_id.unwrap(), |state, window| {
            let mut state = state.unwrap_or(PresenceState {
                hovered: Rc::new(Cell::new(false)),
                shown: false,
                opening_since: None,
                closing_since: None,
            });
            let open = state.hovered.get();
            let visible = if open {
                state.closing_since = None;
                if state.shown {
                    state.opening_since = None;
                    true
                } else {
                    let started = state.opening_since.get_or_insert_with(Instant::now);
                    if started.elapsed() >= SHOW_DELAY {
                        state.shown = true;
                        state.opening_since = None;
                        true
                    } else {
                        window.request_animation_frame();
                        false
                    }
                }
            } else if state.shown {
                state.opening_since = None;
                let started = state.closing_since.get_or_insert_with(Instant::now);
                if started.elapsed() < PANEL_ANIMATION {
                    window.request_animation_frame();
                    true
                } else {
                    state.shown = false;
                    state.closing_since = None;
                    false
                }
            } else {
                state.opening_since = None;
                false
            };

            let hovered = state.hovered.clone();
            let mut root = div()
                .id(host_id.clone())
                .relative()
                .on_hover(move |is_hovered, window, _| {
                    if hovered.get() != *is_hovered {
                        hovered.set(*is_hovered);
                        window.refresh();
                    }
                })
                .children(self.child.take());

            if visible {
                root = root.child(bubble(host_id, label, open));
            }

            let mut element = root.into_any_element();
            ((element.request_layout(window, cx), element), state)
        })
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        element: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        element.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        element: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        element.paint(window, cx);
    }
}

fn bubble(host_id: SharedString, label: SharedString, open: bool) -> AnyElement {
    let animation_id =
        SharedString::from(format!("{host_id}-{}", if open { "enter" } else { "exit" }));
    div()
        .absolute()
        .bottom_full()
        .left_0()
        .right_0()
        .flex()
        .flex_col()
        .items_center()
        .child(
            BaseTooltip::new("tooltip-bubble")
                .px_2()
                .py_1()
                .rounded(px(6.))
                .bg(rgb_light())
                .text_color(rgb_dark())
                .text_size(px(12.))
                .whitespace_nowrap()
                .child(label),
        )
        .child(
            svg()
                .data(icon::TOOLTIP_ARROW)
                .w(px(10.))
                .h(px(6.))
                .mt(px(-1.))
                .text_color(rgb_light()),
        )
        .with_animation(
            animation_id,
            Animation::new(PANEL_ANIMATION).with_easing(ease_out_quint()),
            move |this, delta| {
                let t = if open { delta } else { 1.0 - delta };
                this.opacity(t).mb(px(OFFSET - (1.0 - t) * PANEL_SLIDE))
            },
        )
        .into_any_element()
}

fn rgb_light() -> gpui::Hsla {
    gpui::rgb(0xEEEEEE).into()
}

fn rgb_dark() -> gpui::Hsla {
    gpui::rgb(0x111111).into()
}
