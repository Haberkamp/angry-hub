use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivityKind {
    Merged,
    Closed,
    Reopened,
    Comment,
    Approved,
    ChangesRequested,
}

impl ActivityKind {
    #[allow(dead_code)]
    pub fn label(&self) -> &'static str {
        match self {
            ActivityKind::Merged => "merged",
            ActivityKind::Closed => "closed",
            ActivityKind::Reopened => "reopened",
            ActivityKind::Comment => "commented",
            ActivityKind::Approved => "approved",
            ActivityKind::ChangesRequested => "requested changes",
        }
    }

    pub fn color(&self, cx: &gpui::App) -> gpui::Hsla {
        match self {
            ActivityKind::Merged => crate::ui::color::status::merged(cx),
            ActivityKind::Closed => crate::ui::color::status::closed(cx),
            ActivityKind::Reopened => crate::ui::color::status::reopened(cx),
            ActivityKind::Comment => crate::ui::color::status::comment(cx),
            ActivityKind::Approved => crate::ui::color::status::approved(cx),
            ActivityKind::ChangesRequested => crate::ui::color::status::changes_requested(cx),
        }
    }
}
