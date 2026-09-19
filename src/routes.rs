use gpui::{App, Entity, Window};
use rooter::{GuardResult, RouteContext, RouterConfig};

use crate::layout::{Chrome, main_layout};
use crate::session::Session;
use crate::views::{activity::Activity, login::Login, pull_requests::PullRequests};

pub fn routes(
    login: Entity<Login>,
    pull_requests: Entity<PullRequests>,
    activity: Entity<Activity>,
    chrome: Entity<Chrome>,
) -> RouterConfig {
    RouterConfig::new()
        .route("/login", move || login.clone())
        .guard(guest_only)
        .group("/", |routes| {
            routes
                .layout({
                    let chrome = chrome.clone();
                    move |route: RouteContext, window: &mut Window, cx: &mut App| {
                        main_layout(chrome.clone(), route, window, cx)
                    }
                })
                .index(move || pull_requests.clone())
                .route("activity", move || activity.clone())
        })
        .guard(authenticated)
}

fn authenticated(_window: &mut Window, cx: &mut App) -> GuardResult {
    if cx.global::<Session>().logged_in {
        GuardResult::Allow
    } else {
        GuardResult::Redirect("/login".into())
    }
}

fn guest_only(_window: &mut Window, cx: &mut App) -> GuardResult {
    if cx.global::<Session>().logged_in {
        GuardResult::Redirect("/".into())
    } else {
        GuardResult::Allow
    }
}
