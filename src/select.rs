use std::sync::Arc;

use gpui::{
    AnchoredPositionMode, App, ClickEvent, Corner, Hsla, InteractiveElement, IntoElement,
    MouseButton, MouseDownEvent, ParentElement, RenderOnce, SharedString, Styled, Window, anchored,
    deferred, div, point, prelude::*, px, rgb,
};

use crate::icon::{Icon, IconName};

fn h(c: u32) -> Hsla {
    rgb(c).into()
}

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
    fn render(self, window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let open = self.open;
        let trigger_bg = if open { h(0x3d3d3d) } else { h(0x2a2a2a) };
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
            .hover(|this| this.bg(h(0x3d3d3d)))
            .rounded_full()
            .cursor_pointer()
            .child(
                Icon::new(IconName::ChevronDown)
                    .size(px(14.0))
                    .color(h(0xaaaaaa)),
            );

        if let Some(on_toggle_open) = self.on_toggle_open {
            trigger = trigger
                .on_click(move |event: &ClickEvent, window, cx| on_toggle_open(event, window, cx));
        }

        div()
            .id(self.id)
            .relative()
            .flex_none()
            .child(trigger)
            .when(open, |this| {
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

                this.child(deferred(overlay).with_priority(0))
                    .child(deferred(
                        div()
                            .id("repo-visibility-menu")
                            .absolute()
                            .top_full()
                            .left_0()
                            .mt_1()
                            .min_w(px(180.0))
                            .max_h(px(280.0))
                            .overflow_y_scroll()
                            .flex()
                            .flex_col()
                            .py_1()
                            .bg(h(0x2a2a2a))
                            .border_1()
                            .border_color(h(0x444444))
                            .rounded_md()
                            .shadow_md()
                            .occlude()
                            .children(self.options.into_iter().map(|option| {
                                let option_id = option.id.clone();
                                let selected = option.selected;
                                let mut row = div()
                                    .id(option.id)
                                    .px_3()
                                    .py_1()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .cursor_pointer()
                                    .hover(|this| this.bg(h(0x3d3d3d)))
                                    .child(
                                        div()
                                            .size(px(14.0))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .rounded_sm()
                                            .border_1()
                                            .border_color(if selected {
                                                h(0x6ea8fe)
                                            } else {
                                                h(0x555555)
                                            })
                                            .when(selected, |this| {
                                                this.bg(h(0x3b6ea8)).child(
                                                    Icon::new(IconName::CiCheck)
                                                        .size(px(10.0))
                                                        .color(h(0xffffff)),
                                                )
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(13.0))
                                            .text_color(h(0xffffff))
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
                    ))
            })
    }
}
