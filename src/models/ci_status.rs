use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CiStatus {
    Success,
    Failure,
    Pending,
    None,
}

impl CiStatus {
    pub fn color(&self, cx: &gpui::App) -> gpui::Hsla {
        match self {
            CiStatus::Success => crate::ui::color::status::success(cx),
            CiStatus::Failure => crate::ui::color::status::failure(cx),
            CiStatus::Pending => crate::ui::color::status::pending(cx),
            CiStatus::None => crate::ui::color::status::none(cx),
        }
    }
}
