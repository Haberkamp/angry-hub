use std::path::PathBuf;

use gpui::{App, Bounds, Pixels, Window, WindowBounds, point, px, size};
use serde::{Deserialize, Serialize};

pub const DEFAULT_WINDOW_SIZE: gpui::Size<Pixels> = size(px(800.0), px(600.0));
pub const MIN_WINDOW_SIZE: gpui::Size<Pixels> = size(px(480.0), px(600.0));

#[derive(Clone, Copy, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Placement {
    #[default]
    Windowed,
    Maximized,
    Fullscreen,
}

#[derive(Clone, Copy, Deserialize, Serialize, PartialEq)]
struct Frame {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    #[serde(default)]
    placement: Placement,
}

fn frame_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("angry-hub")
        .join("window.json")
}

fn load() -> Option<Frame> {
    let contents = std::fs::read_to_string(frame_path()).ok()?;
    serde_json::from_str(&contents).ok()
}

fn save(frame: Frame) {
    let path = frame_path();
    let _ = std::fs::create_dir_all(path.parent().unwrap());
    if let Ok(json) = serde_json::to_string_pretty(&frame) {
        let _ = std::fs::write(path, json);
    }
}

fn from_bounds(bounds: WindowBounds) -> Frame {
    let rect = bounds.get_bounds();
    Frame {
        x: f32::from(rect.origin.x),
        y: f32::from(rect.origin.y),
        width: f32::from(rect.size.width),
        height: f32::from(rect.size.height),
        placement: match bounds {
            WindowBounds::Windowed(_) => Placement::Windowed,
            WindowBounds::Maximized(_) => Placement::Maximized,
            WindowBounds::Fullscreen(_) => Placement::Fullscreen,
        },
    }
}

fn is_on_a_display(bounds: Bounds<Pixels>, cx: &App) -> bool {
    cx.displays()
        .iter()
        .any(|display| display.bounds().intersects(&bounds))
}

pub fn persist(window: &Window) {
    let frame = from_bounds(window.window_bounds());
    if load() == Some(frame) {
        return;
    }
    save(frame);
}

pub fn restored_bounds(cx: &App) -> WindowBounds {
    let Some(frame) = load() else {
        return WindowBounds::Windowed(Bounds::centered(None, DEFAULT_WINDOW_SIZE, cx));
    };

    let size = size(
        px(frame.width.max(f32::from(MIN_WINDOW_SIZE.width))),
        px(frame.height.max(f32::from(MIN_WINDOW_SIZE.height))),
    );
    let bounds = Bounds {
        origin: point(px(frame.x), px(frame.y)),
        size,
    };
    let bounds = if is_on_a_display(bounds, cx) {
        bounds
    } else {
        Bounds::centered(None, size, cx)
    };

    match frame.placement {
        Placement::Windowed => WindowBounds::Windowed(bounds),
        Placement::Maximized => WindowBounds::Maximized(bounds),
        Placement::Fullscreen => WindowBounds::Fullscreen(bounds),
    }
}
