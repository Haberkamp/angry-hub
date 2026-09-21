use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrStatus {
    Open,
    Draft,
    Closed,
    Merged,
}

impl PrStatus {
    #[allow(dead_code)]
    pub fn label(&self) -> &'static str {
        match self {
            PrStatus::Open => "Open",
            PrStatus::Draft => "Draft",
            PrStatus::Closed => "Closed",
            PrStatus::Merged => "Merged",
        }
    }

    pub fn color(&self, cx: &gpui::App) -> gpui::Hsla {
        match self {
            PrStatus::Open => crate::ui::color::status::open(cx),
            PrStatus::Draft => crate::ui::color::status::draft(cx),
            PrStatus::Closed => crate::ui::color::status::closed(cx),
            PrStatus::Merged => crate::ui::color::status::merged(cx),
        }
    }
}
