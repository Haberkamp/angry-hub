use gpui::Global;

#[derive(Default)]
pub struct Session {
    pub logged_in: bool,
}

impl Global for Session {}
