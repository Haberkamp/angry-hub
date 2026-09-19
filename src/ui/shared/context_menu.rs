use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::color;
use gpui::{
    AnchoredPositionMode, Animation, AnimationExt as _, AnyElement, App, Bounds, ClickEvent,
    Corner, Element, ElementId, GlobalElementId, InspectorElementId, InteractiveElement,
    IntoElement, LayoutId, MouseButton, MouseDownEvent, ParentElement, Pixels, RenderOnce,
    SharedString, Styled, Window, anchored, deferred, div, ease_out_quint, point, prelude::*, px,
};

use super::icon::{Icon, IconName};
use super::spinner::Spinner;

#[derive(Clone)]
pub struct ContextMenuItem {
    pub id: SharedString,
    pub label: SharedString,
    pub loading: bool,
}

impl ContextMenuItem {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            loading: false,
        }
    }

    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }
}

type ToggleOpenHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;
type DismissHandler = Box<dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static>;
type SelectHandler = Arc<dyn Fn(&str, &mut Window, &mut App) + 'static>;

const PANEL_ANIMATION: Duration = Duration::from_millis(180);
const PANEL_REST_MARGIN: f32 = 4.0;
const PANEL_SLIDE: f32 = 8.0;

struct OpenPresence {
    id: ElementId,
    open: bool,
    child: Option<AnyElement>,
}

struct OpenPresenceState {
    shown: bool,
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
        window.with_element_state(global_id.unwrap(), |state, window| {
            let mut state = state.unwrap_or(OpenPresenceState {
                shown: false,
                closing_since: None,
            });

            let visible = if self.open {
                state.shown = true;
                state.closing_since = None;
                true
            } else if state.shown {
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
                false
            };

            let mut element = if visible {
                self.child.take().expect("presence child")
            } else {
                div().into_any_element()
            };
            ((element.request_layout(window, cx), element), state)
        })
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
pub struct ContextMenu {
    id: SharedString,
    open: bool,
    hover_group: Option<SharedString>,
    items: Vec<ContextMenuItem>,
    on_toggle_open: Option<ToggleOpenHandler>,
    on_dismiss: Option<DismissHandler>,
    on_select: Option<SelectHandler>,
}

impl ContextMenu {
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            open: false,
            hover_group: None,
            items: Vec::new(),
            on_toggle_open: None,
            on_dismiss: None,
            on_select: None,
        }
    }

    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    pub fn hover_group(mut self, group: impl Into<SharedString>) -> Self {
        self.hover_group = Some(group.into());
        self
    }

    pub fn items(mut self, items: Vec<ContextMenuItem>) -> Self {
        self.items = items;
        self
    }

    pub fn on_toggle_open(
        mut self,
        listener: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle_open = Some(Box::new(listener));
        self
    }

    pub fn on_dismiss(
        mut self,
        listener: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_dismiss = Some(Box::new(listener));
        self
    }

    pub fn on_select(mut self, listener: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Arc::new(listener));
        self
    }
}

impl RenderOnce for ContextMenu {
    fn render(self, window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let open = self.open;
        let trigger_bg = if open {
            color::surface_hover()
        } else {
            color::surface()
        };
        let on_select = self.on_select;
        let viewport = window.viewport_size();
        let dismiss_id = SharedString::from(format!("{}-dismiss", self.id));
        let menu_id = SharedString::from(format!("{}-menu", self.id));

        let mut trigger = div()
            .id(SharedString::from(format!("{}-trigger", self.id)))
            .size(px(28.0))
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .bg(trigger_bg)
            .hover(|this| this.bg(color::surface_hover()))
            .rounded_full()
            .cursor_pointer()
            .when(!open, |this| {
                let this = this.opacity(0.0);
                if let Some(group) = self.hover_group.clone() {
                    this.group_hover(group, |style| style.opacity(1.0))
                } else {
                    this
                }
            })
            .child(
                Icon::new(IconName::Ellipsis)
                    .size(px(14.0))
                    .color(color::text_secondary()),
            );

        if let Some(on_toggle_open) = self.on_toggle_open {
            trigger = trigger
                .on_click(move |event: &ClickEvent, window, cx| on_toggle_open(event, window, cx));
        }

        let menu_key = self.id.clone();
        let mut dismiss = div()
            .id(dismiss_id)
            .w(viewport.width)
            .h(viewport.height)
            .occlude();
        if let Some(on_dismiss) = self.on_dismiss {
            dismiss = dismiss.on_mouse_down(MouseButton::Left, move |event, window, cx| {
                on_dismiss(event, window, cx)
            });
        }
        let overlay = anchored()
            .position_mode(AnchoredPositionMode::Window)
            .position(point(px(0.0), px(0.0)))
            .anchor(Corner::TopLeft)
            .child(dismiss);

        let animation_id = SharedString::from(format!(
            "{}-{}",
            menu_id,
            if open { "enter" } else { "exit" }
        ));
        let panel = div()
            .id(menu_id)
            .absolute()
            .top_full()
            .right_0()
            .min_w(px(180.0))
            .flex()
            .flex_col()
            .p_1()
            .bg(color::surface())
            .border_1()
            .border_color(color::border())
            .rounded(px(9.0))
            .shadow_md()
            .occlude()
            .children(self.items.into_iter().map(|item| {
                let item_id = item.id.clone();
                let loading = item.loading;
                let spinner_id = SharedString::from(format!("{}-spinner", item_id));
                let mut row = div()
                    .id(item.id)
                    .px_3()
                    .py_1()
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .when(!loading, |this| {
                        this.cursor_pointer().hover(|this| this.bg(color::surface_hover()))
                    })
                    .when(loading, |this| this.cursor_default())
                    .child(
                        div()
                            .text_size(px(13.0))
                            .text_color(color::text())
                            .whitespace_nowrap()
                            .child(item.label),
                    )
                    .when(loading, |this| {
                        this.child(Spinner::new(spinner_id).size(px(12.0)))
                    });

                if let Some(on_select) = on_select.clone()
                    && !loading
                {
                    row = row.on_click({
                        let item_id = item_id.clone();
                        move |_, window, cx| on_select(item_id.as_ref(), window, cx)
                    });
                }

                row
            }))
            .with_animation(
                animation_id,
                Animation::new(PANEL_ANIMATION).with_easing(ease_out_quint()),
                move |this, delta| {
                    let t = if open { delta } else { 1.0 - delta };
                    this.opacity(t)
                        .mt(px(PANEL_REST_MARGIN - (1.0 - t) * PANEL_SLIDE))
                },
            );

        div()
            .id(menu_key.clone())
            .relative()
            .flex_none()
            .child(trigger)
            .child(OpenPresence {
                id: ElementId::from(SharedString::from(format!("{menu_key}-presence"))),
                open,
                child: Some(
                    div()
                        .child(deferred(overlay).with_priority(0))
                        .child(deferred(panel))
                        .into_any_element(),
                ),
            })
    }
}
