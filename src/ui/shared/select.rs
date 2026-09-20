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

#[derive(Clone)]
pub struct SelectOption {
    pub id: SharedString,
    pub label: SharedString,
    pub selected: bool,
}

impl SelectOption {
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        selected: bool,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            selected,
        }
    }
}

type ToggleOpenHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;
type DismissHandler = Box<dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static>;
type ToggleOptionHandler = Arc<dyn Fn(&str, &mut Window, &mut App) + 'static>;

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
pub struct MultiSelect {
    id: SharedString,
    open: bool,
    options: Vec<SelectOption>,
    on_toggle_open: Option<ToggleOpenHandler>,
    on_dismiss: Option<DismissHandler>,
    on_toggle_option: Option<ToggleOptionHandler>,
}

impl MultiSelect {
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            open: false,
            options: Vec::new(),
            on_toggle_open: None,
            on_dismiss: None,
            on_toggle_option: None,
        }
    }

    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    pub fn options(mut self, options: Vec<SelectOption>) -> Self {
        self.options = options;
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

    pub fn on_toggle_option(
        mut self,
        listener: impl Fn(&str, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle_option = Some(Arc::new(listener));
        self
    }
}

impl RenderOnce for MultiSelect {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let open = self.open;
        let trigger_bg = if open {
            color::interaction::pressed(cx)
        } else {
            color::surface::sunken(cx)
        };
        let trigger_hover = color::interaction::pressed(cx);
        let chevron = color::icon::subtle(cx);
        let overlay_bg = color::surface::overlay(cx);
        let overlay_border = color::border::default(cx);
        let row_hover = color::interaction::pressed(cx);
        let selected_fill = color::bg::selected(cx);
        let selected_icon = color::icon::on_solid(cx);
        let unselected_bg = color::surface::sunken(cx);
        let unselected_border = color::border::strong(cx);
        let option_text = color::text::primary(cx);
        let on_toggle_option = self.on_toggle_option;
        let viewport = window.viewport_size();

        let mut trigger = div()
            .id(SharedString::from(format!("{}-trigger", self.id)))
            .size(px(28.0))
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .bg(trigger_bg)
            .hover(|this| this.bg(trigger_hover))
            .rounded_full()
            .child(
                Icon::new(IconName::ChevronDown)
                    .size(px(14.0))
                    .color(chevron),
            );

        if let Some(on_toggle_open) = self.on_toggle_open {
            trigger = trigger
                .on_click(move |event: &ClickEvent, window, cx| on_toggle_open(event, window, cx));
        }

        let select_id = self.id.clone();
        let mut dismiss = div()
            .id("repo-visibility-dismiss")
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

        let panel_id = SharedString::from(format!("{}-menu", select_id));
        let animation_id = SharedString::from(format!(
            "{}-{}",
            panel_id,
            if open { "enter" } else { "exit" }
        ));
        let panel = div()
            .id(panel_id)
            .absolute()
            .top_full()
            .left_0()
            .min_w(px(180.0))
            .max_h(px(280.0))
            .flex()
            .flex_col()
            .p_1()
            .bg(overlay_bg)
            .border_1()
            .border_color(overlay_border)
            .rounded(px(9.0))
            .shadow_md()
            .occlude()
            .child(
                div()
                    .id("repo-visibility-menu-scroll")
                    .max_h(px(270.0))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .children(self.options.into_iter().map(|option| {
                        let option_id = option.id.clone();
                        let selected = option.selected;
                        let mut row = div()
                            .id(option.id)
                            .px_3()
                            .py_1()
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .gap_2()
                            .hover(|this| this.bg(row_hover))
                            .child(
                                div()
                                    .size(px(14.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_sm()
                                    .border_1()
                                    .border_color(if selected {
                                        selected_fill
                                    } else {
                                        unselected_border
                                    })
                                    .bg(if selected {
                                        selected_fill
                                    } else {
                                        unselected_bg
                                    })
                                    .when(selected, |this| {
                                        this.child(
                                            Icon::new(IconName::CiCheck)
                                                .size(px(10.0))
                                                .color(selected_icon),
                                        )
                                    }),
                            )
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .text_color(option_text)
                                    .whitespace_nowrap()
                                    .child(option.label),
                            );

                        if let Some(on_toggle_option) = on_toggle_option.clone() {
                            row = row.on_click({
                                let option_id = option_id.clone();
                                move |_, window, cx| {
                                    on_toggle_option(option_id.as_ref(), window, cx)
                                }
                            });
                        }

                        row
                    })),
            )
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
            .id(select_id.clone())
            .relative()
            .flex_none()
            .child(trigger)
            .child(OpenPresence {
                id: ElementId::from(SharedString::from(format!("{select_id}-presence"))),
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
