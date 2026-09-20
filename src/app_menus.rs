use gpui::{App, KeyBinding, Menu, MenuItem, OsAction, SystemMenuType, actions};

actions!(
    app_menus,
    [
        Quit,
        Hide,
        HideOthers,
        ShowAll,
        Minimize,
        Zoom,
        ToggleFullScreen,
        CloseWindow,
        ShowPullRequests,
        ShowActivity,
        ShowSettings,
        Refresh,
        Cut,
        Copy,
        Paste,
        SelectAll,
    ]
);

pub fn init(cx: &mut App) {
    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.on_action(|_: &Hide, cx| cx.hide());
    cx.on_action(|_: &HideOthers, cx| cx.hide_other_apps());
    cx.on_action(|_: &ShowAll, cx| cx.unhide_other_apps());

    cx.bind_keys([
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-h", Hide, None),
        KeyBinding::new("alt-cmd-h", HideOthers, None),
        KeyBinding::new("cmd-m", Minimize, None),
        KeyBinding::new("cmd-w", CloseWindow, None),
        KeyBinding::new("cmd-1", ShowPullRequests, None),
        KeyBinding::new("cmd-2", ShowActivity, None),
        KeyBinding::new("cmd-,", ShowSettings, None),
        KeyBinding::new("cmd-r", Refresh, None),
        KeyBinding::new("ctrl-cmd-f", ToggleFullScreen, None),
        KeyBinding::new("cmd-x", Cut, None),
        KeyBinding::new("cmd-c", Copy, None),
        KeyBinding::new("cmd-v", Paste, None),
        KeyBinding::new("cmd-a", SelectAll, None),
    ]);

    set_menus(cx, true);
}

pub fn set_menus(cx: &App, include_refresh: bool) {
    let mut view_items = vec![
        MenuItem::action("Pull Requests", ShowPullRequests),
        MenuItem::action("Activity", ShowActivity),
        MenuItem::separator(),
    ];
    if include_refresh {
        view_items.push(MenuItem::action("Refresh", Refresh));
        view_items.push(MenuItem::separator());
    }
    view_items.push(MenuItem::action("Enter Full Screen", ToggleFullScreen));

    cx.set_menus(vec![
        Menu {
            name: "Angry Hub".into(),
            items: vec![
                MenuItem::action("Settings…", ShowSettings),
                MenuItem::separator(),
                MenuItem::os_submenu("Services", SystemMenuType::Services),
                MenuItem::separator(),
                MenuItem::action("Hide Angry Hub", Hide),
                MenuItem::action("Hide Others", HideOthers),
                MenuItem::action("Show All", ShowAll),
                MenuItem::separator(),
                MenuItem::action("Quit Angry Hub", Quit),
            ],
        },
        Menu {
            name: "Edit".into(),
            items: vec![
                MenuItem::os_action("Cut", Cut, OsAction::Cut),
                MenuItem::os_action("Copy", Copy, OsAction::Copy),
                MenuItem::os_action("Paste", Paste, OsAction::Paste),
                MenuItem::separator(),
                MenuItem::os_action("Select All", SelectAll, OsAction::SelectAll),
            ],
        },
        Menu {
            name: "View".into(),
            items: view_items,
        },
        Menu {
            name: "Window".into(),
            items: vec![
                MenuItem::action("Minimize", Minimize),
                MenuItem::action("Zoom", Zoom),
                MenuItem::separator(),
                MenuItem::action("Close Window", CloseWindow),
            ],
        },
    ]);
}
