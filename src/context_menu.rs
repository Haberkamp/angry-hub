use std::sync::Arc;

use gpui::{
    AnchoredPositionMode, App, ClickEvent, Corner, Hsla, InteractiveElement, IntoElement,
    MouseButton, MouseDownEvent, ParentElement, RenderOnce, SharedString, Styled, Window, anchored,
    deferred, div, point, prelude::*, px, rgb,
};

use crate::icon::{Icon, IconName};
use crate::spinner::Spinner;

fn h(c: u32) -> Hsla {
    rgb(c).into()
}

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
        let trigger_bg = if open { h(0x3d3d3d) } else { h(0x2a2a2a) };
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
            .hover(|this| this.bg(h(0x3d3d3d)))
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

                this.child(deferred(overlay).with_priority(0))
                    .child(deferred(
                        div()
                            .id(menu_id)
                            .absolute()
                            .top_full()
                            .right_0()
                            .mt_1()
                            .min_w(px(180.0))
                            .flex()
                            .flex_col()
                            .p_1()
                            .bg(h(0x2a2a2a))
                            .border_1()
                            .border_color(h(0x444444))
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
                                        this.cursor_pointer().hover(|this| this.bg(h(0x3d3d3d)))
                                    })
                                    .when(loading, |this| this.cursor_default())
                                    .child(
                                        div()
                                            .text_size(px(13.0))
                                            .text_color(h(0xffffff))
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
                            })),
                    ))
            })
    }
}
