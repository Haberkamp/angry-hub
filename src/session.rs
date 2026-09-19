use gpui::{App, Global, Window};
use rooter::Router;

use crate::datasource::code_host;

#[derive(Default)]
pub struct Session {
    pub logged_in: bool,
}

impl Global for Session {}

pub fn force_logout(window: &mut Window, cx: &mut App) {
    code_host().logout();
    cx.global_mut::<Session>().logged_in = false;
    Router::navigate_window(window, cx, "/login");
}
