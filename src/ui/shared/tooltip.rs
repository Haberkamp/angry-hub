use std::time::{Duration, Instant};

use crate::color;
use gpui::{
    Animation, AnimationExt as _, AnyElement, App, Bounds, Element, ElementId, GlobalElementId,
    InspectorElementId, InteractiveElement, IntoElement, LayoutId, ParentElement, Pixels,
    RenderOnce, SharedString, Styled, Window, div, ease_out_quint, prelude::*, px, svg,
};

const DEFAULT_OFFSET: f32 = 4.0;
const DEFAULT_SHOW_DELAY: Duration = Duration::from_millis(200);
const PANEL_ANIMATION: Duration = Duration::from_millis(180);
const PANEL_SLIDE: f32 = 8.0;
const ARROW_PATH: &str = "icons/tooltip_arrow.svg";

type HoverHandler = Box<dyn Fn(&bool, &mut Window, &mut App) + 'static>;
type CloseHandler = Box<dyn Fn(&(), &mut Window, &mut App) + 'static>;

struct OpenPresence {
    id: ElementId,
    open: bool,
    show_delay: Duration,
    on_close: Option<CloseHandler>,
    child: Option<AnyElement>,
}

struct OpenPresenceState {
    shown: bool,
    opening_since: Option<Instant>,
    closing_since: Option<Instant>,
}

impl IntoElement for OpenPresence {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for OpenPresence {
    type RequestLayoutState = AnyElement;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
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
        let (layout, closed) = window.with_element_state(global_id.unwrap(), |state, window| {
            let mut state = state.unwrap_or(OpenPresenceState {
                shown: false,
                opening_since: None,
                closing_since: None,
            });
            let mut closed = false;

            let visible = if self.open {
                state.closing_since = None;
                if state.shown {
                    state.opening_since = None;
                    true
                } else {
                    let started = state.opening_since.get_or_insert_with(Instant::now);
                    if started.elapsed() >= self.show_delay {
                        state.shown = true;
                        state.opening_since = None;
                        true
                    } else {
                        window.request_animation_frame();
                        false
                    }
                }
            } else {
                state.opening_since = None;
                if state.shown {
                    let started = state.closing_since.get_or_insert_with(Instant::now);
                    if started.elapsed() < PANEL_ANIMATION {
                        window.request_animation_frame();
                        true
                    } else {
                        state.shown = false;
                        state.closing_since = None;
                        closed = true;
                        false
                    }
                } else {
                    false
                }
            };

            let mut element = if visible {
                self.child.take().expect("presence child")
            } else {
                div().into_any_element()
            };
            (
                ((element.request_layout(window, cx), element), closed),
                state,
            )
        });

        if closed && let Some(on_close) = &self.on_close {
            on_close(&(), window, cx);
        }

        layout
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        element: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        element.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        element: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        element.paint(window, cx);
    }
}

#[derive(IntoElement)]
pub struct Tooltip {
    id: SharedString,
    label: SharedString,
    offset: f32,
    show_delay: Duration,
    open: bool,
    on_hover: Option<HoverHandler>,
    on_close: Option<CloseHandler>,
    child: Option<AnyElement>,
}

impl Tooltip {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            offset: DEFAULT_OFFSET,
            show_delay: DEFAULT_SHOW_DELAY,
            open: false,
            on_hover: None,
            on_close: None,
            child: None,
        }
    }

    pub fn offset(mut self, offset: impl Into<Pixels>) -> Self {
        self.offset = f32::from(offset.into());
        self
    }

    #[allow(dead_code)]
    pub fn show_delay(mut self, delay: Duration) -> Self {
        self.show_delay = delay;
        self
    }

    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    pub fn on_hover(mut self, listener: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_hover = Some(Box::new(listener));
        self
    }

    pub fn on_close(mut self, listener: impl Fn(&(), &mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Box::new(listener));
        self
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.child = Some(child.into_any_element());
        self
    }
}

impl RenderOnce for Tooltip {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let open = self.open;
        let rest = self.offset;
        let tooltip_id = SharedString::from(format!("{}-label", self.id));
        let animation_id = SharedString::from(format!(
            "{}-{}",
            tooltip_id,
            if open { "enter" } else { "exit" }
        ));
        let bg = color::gray::s12();
        let fg = color::gray::s1();
        let presence_id = ElementId::from(SharedString::from(format!("{}-presence", self.id)));

        let bubble = div()
            .id(tooltip_id)
            .absolute()
            .bottom_full()
            .left_0()
            .right_0()
            .flex()
            .flex_col()
            .items_center()
            .child(
                div()
                    .px_2()
                    .py_1()
                    .rounded(px(6.0))
                    .bg(bg)
                    .text_xs()
                    .text_color(fg)
                    .whitespace_nowrap()
                    .child(self.label),
            )
            .child(
                svg()
                    .path(ARROW_PATH)
                    .w(px(10.0))
                    .h(px(6.0))
                    .mt(px(-1.0))
                    .flex_none()
                    .text_color(bg),
            )
            .with_animation(
                animation_id,
                Animation::new(PANEL_ANIMATION).with_easing(ease_out_quint()),
                move |this, delta| {
                    let t = if open { delta } else { 1.0 - delta };
                    this.opacity(t).mb(px(rest - (1.0 - t) * PANEL_SLIDE))
                },
            );

        let mut root = div()
            .id(self.id)
            .relative()
            .flex_none()
            .children(self.child)
            .child(OpenPresence {
                id: presence_id,
                open,
                show_delay: self.show_delay,
                on_close: self.on_close,
                child: Some(bubble.into_any_element()),
            });

        if let Some(on_hover) = self.on_hover {
            root = root.on_hover(move |hovered, window, cx| on_hover(hovered, window, cx));
        }

        root
    }
}
