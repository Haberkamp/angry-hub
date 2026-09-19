use crate::color;
use gpui::{
    Animation, AnimationExt, App, Hsla, IntoElement, Pixels, Radians, RenderOnce, SharedString,
    Styled, Transformation, Window, px, svg,
};

pub const SPINNER_PATH: &str = "icons/spinner.svg";

#[derive(IntoElement)]
pub struct Spinner {
    id: SharedString,
    size: Pixels,
    color: Hsla,
}

impl Spinner {
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            size: px(16.0),
            color: color::gray::s10(),
        }
    }

    #[allow(dead_code)]
    pub fn size(mut self, size: impl Into<Pixels>) -> Self {
        self.size = size.into();
        self
    }

    #[allow(dead_code)]
    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = color.into();
        self
    }
}

impl RenderOnce for Spinner {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        svg()
            .path(SPINNER_PATH)
            .size(self.size)
            .flex_none()
            .text_color(self.color)
            .with_animation(
                self.id,
                Animation::new(std::time::Duration::from_millis(900)).repeat(),
                |svg, delta| {
                    svg.with_transformation(Transformation::rotate(Radians(
                        delta * 2.0 * std::f32::consts::PI,
                    )))
                },
            )
    }
}
