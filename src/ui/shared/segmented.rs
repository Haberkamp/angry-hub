use std::sync::Arc;
use std::time::Duration;

use crate::color;
use gpui::{
    Animation, AnimationExt as _, AnyElement, App, ClickEvent, InteractiveElement, IntoElement,
    ParentElement, RenderOnce, SharedString, Styled, Window, div, ease_out_quint, prelude::*, px,
    relative,
};

type ChangeHandler = Box<dyn Fn(&str, &mut Window, &mut App) + 'static>;

#[derive(Clone)]
pub struct Segment {
    pub id: SharedString,
    pub label: SharedString,
}

impl Segment {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}

#[derive(IntoElement)]
pub struct SegmentedControl {
    id: SharedString,
    segments: Vec<Segment>,
    selected: SharedString,
    previous_selected: Option<SharedString>,
    on_change: Option<ChangeHandler>,
}

impl SegmentedControl {
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            segments: Vec::new(),
            selected: SharedString::from(""),
            previous_selected: None,
            on_change: None,
        }
    }

    pub fn segments(mut self, segments: impl IntoIterator<Item = Segment>) -> Self {
        self.segments = segments.into_iter().collect();
        if self.selected.is_empty()
            && let Some(first) = self.segments.first()
        {
            self.selected = first.id.clone();
        }
        self
    }

    pub fn selected(mut self, id: impl Into<SharedString>) -> Self {
        self.selected = id.into();
        self
    }

    pub fn previous_selected(mut self, id: impl Into<SharedString>) -> Self {
        self.previous_selected = Some(id.into());
        self
    }

    pub fn on_change(mut self, listener: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Box::new(listener));
        self
    }
}

impl RenderOnce for SegmentedControl {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let selected = self.selected.clone();
        let on_change = self.on_change.map(Arc::new);
        let count = self.segments.len().max(1) as f32;
        let to_ix = self
            .segments
            .iter()
            .position(|segment| segment.id.as_ref() == selected.as_ref())
            .unwrap_or(0);
        let from_ix = self
            .previous_selected
            .as_ref()
            .and_then(|id| {
                self.segments
                    .iter()
                    .position(|segment| segment.id.as_ref() == id.as_ref())
            })
            .unwrap_or(to_ix);
        let animate = from_ix != to_ix;
        let animation_id = format!("{}-indicator-{}-{}", self.id, from_ix, to_ix);
        let pill_bg = color::interaction::pressed(cx);
        let track_bg = color::surface::sunken(cx);
        let selected_text = color::text::primary(cx);
        let muted_text = color::text::secondary(cx);

        let indicator: AnyElement = {
            let pill = div()
                .absolute()
                .top(px(0.0))
                .bottom(px(0.0))
                .rounded_full()
                .bg(pill_bg)
                .w(relative(1.0 / count));
            if animate {
                pill.with_animation(
                    SharedString::from(animation_id),
                    Animation::new(Duration::from_millis(220)).with_easing(ease_out_quint()),
                    move |this, delta| {
                        let index = from_ix as f32 + (to_ix as f32 - from_ix as f32) * delta;
                        this.left(relative(index / count))
                    },
                )
                .into_any_element()
            } else {
                pill.left(relative(to_ix as f32 / count)).into_any_element()
            }
        };

        div()
            .id(self.id)
            .flex()
            .flex_none()
            .items_center()
            .p(px(3.0))
            .rounded_full()
            .bg(track_bg)
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(8.0))
                    .child(indicator)
                    .children(self.segments.into_iter().map(|segment| {
                        let is_selected = segment.id.as_ref() == selected.as_ref();
                        let id = segment.id.clone();
                        let on_change = on_change.clone();
                        let mut item = div()
                            .id(segment.id.clone())
                            .relative()
                            .px_3()
                            .py_1()
                            .h(px(32.0))
                            .min_w(px(128.0))
                            .flex()
                            .flex_1()
                            .items_center()
                            .justify_center()
                            .rounded_full()
                            .text_size(px(14.0))
                            .cursor_pointer()
                            .when(is_selected, |this| this.text_color(selected_text))
                            .when(!is_selected, |this| this.text_color(muted_text))
                            .child(segment.label);

                        if let Some(on_change) = on_change {
                            item = item.on_click(move |_: &ClickEvent, window, cx| {
                                on_change(id.as_ref(), window, cx);
                            });
                        }

                        item
                    })),
            )
    }
}
