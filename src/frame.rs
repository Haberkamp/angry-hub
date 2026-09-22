use gpui::{
    App, Bounds, Context, DisplayId, Global, Pixels, Size, Window, point, px, size,
};
use serde::{Deserialize, Serialize};

pub struct Placement {
    pub window_id: u64,
    pub bounds: Bounds<Pixels>,
    pub display_id: Option<DisplayId>,
}

pub fn restore(default_size: Size<Pixels>, cx: &mut App) -> Vec<Placement> {
    cx.set_global(WindowFrames::load());
    let default = (
        f32::from(default_size.width),
        f32::from(default_size.height),
    );
    cx.global::<WindowFrames>()
        .restore(&connected_screens(cx), default)
        .into_iter()
        .map(|window| Placement {
            display_id: display_id(cx, &window.screen_id),
            window_id: window.window_id,
            bounds: Bounds {
                origin: point(px(window.x), px(window.y)),
                size: size(px(window.width), px(window.height)),
            },
        })
        .collect()
}

pub fn observe<T: 'static>(window_id: u64, window: &mut Window, cx: &mut Context<T>) {
    window.on_window_should_close(cx, move |window, cx| {
        remember(window_id, window, cx);
        true
    });
    cx.observe_window_bounds(window, move |_, window, cx| {
        remember(window_id, window, cx);
    })
    .detach();
}

fn remember(window_id: u64, window: &Window, cx: &mut App) {
    let Some(display) = window.display(cx) else {
        return;
    };
    let Ok(screen_id) = display.uuid() else {
        return;
    };
    let origin = display.bounds().origin;
    let bounds = window.bounds();
    let connected = connected_screens(cx)
        .into_iter()
        .map(|screen| screen.id)
        .collect::<Vec<_>>();
    let frames = cx.global_mut::<WindowFrames>();
    frames.remember(
        window_id,
        &screen_id.to_string(),
        Frame {
            rel_x: f32::from(bounds.origin.x - origin.x),
            rel_y: f32::from(bounds.origin.y - origin.y),
            width: f32::from(bounds.size.width),
            height: f32::from(bounds.size.height),
        },
        &connected,
    );
    frames.save();
}

fn connected_screens(cx: &App) -> Vec<Screen> {
    let primary = cx.primary_display().map(|display| display.id());
    cx.displays()
        .into_iter()
        .filter_map(|display| {
            let id = display.uuid().ok()?.to_string();
            let bounds = display.bounds();
            Some(Screen {
                id,
                x: f32::from(bounds.origin.x),
                y: f32::from(bounds.origin.y),
                width: f32::from(bounds.size.width),
                height: f32::from(bounds.size.height),
                primary: primary == Some(display.id()),
            })
        })
        .collect()
}

fn display_id(cx: &App, screen_id: &str) -> Option<DisplayId> {
    cx.displays().into_iter().find_map(|display| {
        let id = display.uuid().ok()?.to_string();
        (id == screen_id).then(|| display.id())
    })
}

#[derive(Clone, Debug, PartialEq)]
struct Screen {
    id: String,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    primary: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Frame {
    rel_x: f32,
    rel_y: f32,
    width: f32,
    height: f32,
}

#[derive(Clone, Debug, PartialEq)]
struct RestoredWindow {
    window_id: u64,
    screen_id: String,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
struct WindowFrames {
    windows: Vec<SavedWindow>,
}

impl Global for WindowFrames {}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct SavedWindow {
    id: u64,
    home_screen: String,
    frames: Vec<SavedFrame>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct SavedFrame {
    screen_id: String,
    rel_x: f32,
    rel_y: f32,
    width: f32,
    height: f32,
}

impl WindowFrames {
    fn load() -> Self {
        let Ok(contents) = std::fs::read_to_string(config_path()) else {
            return Self::default();
        };
        serde_json::from_str(&contents).unwrap_or_default()
    }

    fn save(&self) {
        let path = config_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }

    fn remember(
        &mut self,
        window_id: u64,
        screen_id: &str,
        frame: Frame,
        connected_screen_ids: &[String],
    ) {
        let frame = SavedFrame {
            screen_id: screen_id.to_string(),
            rel_x: frame.rel_x,
            rel_y: frame.rel_y,
            width: frame.width,
            height: frame.height,
        };
        let Some(window) = self.windows.iter_mut().find(|window| window.id == window_id) else {
            self.windows.push(SavedWindow {
                id: window_id,
                home_screen: screen_id.to_string(),
                frames: vec![frame],
            });
            return;
        };

        if let Some(existing) = window
            .frames
            .iter_mut()
            .find(|saved| saved.screen_id == screen_id)
        {
            *existing = frame;
        } else {
            window.frames.push(frame);
        }

        let home_connected = connected_screen_ids
            .iter()
            .any(|id| id == &window.home_screen);
        if home_connected {
            window.home_screen = screen_id.to_string();
        }
    }

    fn restore(&self, connected: &[Screen], default_size: (f32, f32)) -> Vec<RestoredWindow> {
        let Some(primary) = connected
            .iter()
            .find(|screen| screen.primary)
            .or(connected.first())
        else {
            return Vec::new();
        };
        if self.windows.is_empty() {
            return vec![centered(1, primary, default_size)];
        }

        self.windows
            .iter()
            .map(|window| {
                if let Some(screen) = connected
                    .iter()
                    .find(|screen| screen.id == window.home_screen)
                    && let Some(frame) = window.frame_on(&screen.id)
                {
                    return placed(window.id, screen, frame);
                }
                if let Some(screen) = connected
                    .iter()
                    .find(|screen| window.frame_on(&screen.id).is_some())
                    && let Some(frame) = window.frame_on(&screen.id)
                {
                    return placed(window.id, screen, frame);
                }
                centered(window.id, primary, default_size)
            })
            .collect()
    }
}

impl SavedWindow {
    fn frame_on(&self, screen_id: &str) -> Option<&SavedFrame> {
        self.frames.iter().find(|frame| frame.screen_id == screen_id)
    }
}

fn placed(window_id: u64, screen: &Screen, frame: &SavedFrame) -> RestoredWindow {
    RestoredWindow {
        window_id,
        screen_id: screen.id.clone(),
        x: screen.x + frame.rel_x,
        y: screen.y + frame.rel_y,
        width: frame.width,
        height: frame.height,
    }
}

fn centered(window_id: u64, screen: &Screen, default_size: (f32, f32)) -> RestoredWindow {
    RestoredWindow {
        window_id,
        screen_id: screen.id.clone(),
        x: screen.x + (screen.width - default_size.0) / 2.0,
        y: screen.y + (screen.height - default_size.1) / 2.0,
        width: default_size.0,
        height: default_size.1,
    }
}

fn config_path() -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("angry-hub")
        .join("windows.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(id: &str, x: f32, y: f32, width: f32, height: f32, primary: bool) -> Screen {
        Screen {
            id: id.to_string(),
            x,
            y,
            width,
            height,
            primary,
        }
    }

    fn laptop() -> Screen {
        screen("laptop", 0.0, 0.0, 1440.0, 900.0, true)
    }

    fn external() -> Screen {
        screen("external", 1440.0, 0.0, 2560.0, 1440.0, false)
    }

    #[test]
    fn restores_a_default_window_when_nothing_is_saved() {
        let frames = WindowFrames::default();
        let restored = frames.restore(&[laptop()], (800.0, 600.0));

        assert_eq!(
            restored,
            vec![RestoredWindow {
                window_id: 1,
                screen_id: "laptop".into(),
                x: 320.0,
                y: 150.0,
                width: 800.0,
                height: 600.0,
            }]
        );
    }

    #[test]
    fn restores_position_and_size_on_the_same_screen() {
        let mut frames = WindowFrames::default();
        frames.remember(
            1,
            "laptop",
            Frame {
                rel_x: 40.0,
                rel_y: 80.0,
                width: 900.0,
                height: 700.0,
            },
            &["laptop".into()],
        );

        let restored = frames.restore(&[laptop()], (800.0, 600.0));

        assert_eq!(
            restored,
            vec![RestoredWindow {
                window_id: 1,
                screen_id: "laptop".into(),
                x: 40.0,
                y: 80.0,
                width: 900.0,
                height: 700.0,
            }]
        );
    }

    #[test]
    fn restores_each_window_on_its_own_screen() {
        let mut frames = WindowFrames::default();
        let both = ["laptop".into(), "external".into()];
        frames.remember(
            1,
            "laptop",
            Frame {
                rel_x: 10.0,
                rel_y: 20.0,
                width: 800.0,
                height: 600.0,
            },
            &both,
        );
        frames.remember(
            2,
            "external",
            Frame {
                rel_x: 30.0,
                rel_y: 40.0,
                width: 1200.0,
                height: 800.0,
            },
            &both,
        );

        let mut restored = frames.restore(&[laptop(), external()], (800.0, 600.0));
        restored.sort_by_key(|window| window.window_id);

        assert_eq!(
            restored,
            vec![
                RestoredWindow {
                    window_id: 1,
                    screen_id: "laptop".into(),
                    x: 10.0,
                    y: 20.0,
                    width: 800.0,
                    height: 600.0,
                },
                RestoredWindow {
                    window_id: 2,
                    screen_id: "external".into(),
                    x: 1470.0,
                    y: 40.0,
                    width: 1200.0,
                    height: 800.0,
                },
            ]
        );
    }

    #[test]
    fn keeps_a_screens_position_when_that_screen_is_unplugged() {
        let mut frames = WindowFrames::default();
        frames.remember(
            1,
            "external",
            Frame {
                rel_x: 100.0,
                rel_y: 200.0,
                width: 900.0,
                height: 700.0,
            },
            &["laptop".into(), "external".into()],
        );
        frames.remember(
            1,
            "laptop",
            Frame {
                rel_x: 5.0,
                rel_y: 5.0,
                width: 800.0,
                height: 600.0,
            },
            &["laptop".into()],
        );

        let parked = frames.restore(&[laptop()], (800.0, 600.0));
        assert_eq!(parked[0].screen_id, "laptop");

        let restored = frames.restore(&[laptop(), external()], (800.0, 600.0));
        assert_eq!(
            restored,
            vec![RestoredWindow {
                window_id: 1,
                screen_id: "external".into(),
                x: 1540.0,
                y: 200.0,
                width: 900.0,
                height: 700.0,
            }]
        );
    }
}
