use gpui::{IntoElement, ParentElement, Styled, div, prelude::*};

/// Centered shell for routes without the main chrome (login).
pub fn bare_layout(child: impl IntoElement) -> impl IntoElement {
    div()
        .id("bare-layout")
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_4()
        .child(child)
}
