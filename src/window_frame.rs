use std::path::PathBuf;
use std::rc::Rc;

use gpui::{
    App, Bounds, DisplayId, Pixels, PlatformDisplay, Window, WindowBounds, point, px, size,
};
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

#[derive(Clone, Deserialize, Serialize, PartialEq)]
struct Frame {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    #[serde(default)]
    placement: Placement,
    #[serde(default)]
    display_uuid: Option<String>,
    #[serde(default)]
    rel_x: Option<f32>,
    #[serde(default)]
    rel_y: Option<f32>,
}

pub struct RestoredWindow {
    pub bounds: WindowBounds,
    pub display_id: Option<DisplayId>,
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

fn from_bounds(bounds: WindowBounds, display: Option<&dyn PlatformDisplay>) -> Frame {
    let rect = bounds.get_bounds();
    let (display_uuid, rel_x, rel_y) = match display {
        Some(display) => {
            let origin = display.bounds().origin;
            (
                display.uuid().ok().map(|uuid| uuid.to_string()),
                Some(f32::from(rect.origin.x - origin.x)),
                Some(f32::from(rect.origin.y - origin.y)),
            )
        }
        None => (None, None, None),
    };

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
        display_uuid,
        rel_x,
        rel_y,
    }
}

fn display_uuid(display: &dyn PlatformDisplay) -> Option<String> {
    display.uuid().ok().map(|uuid| uuid.to_string())
}

fn find_display_by_uuid(cx: &App, uuid: &str) -> Option<Rc<dyn PlatformDisplay>> {
    cx.displays()
        .into_iter()
        .find(|display| display_uuid(display.as_ref()).as_deref() == Some(uuid))
}

fn constrain_to_display(bounds: Bounds<Pixels>, screen: Bounds<Pixels>) -> Bounds<Pixels> {
    let width = bounds
        .size
        .width
        .max(MIN_WINDOW_SIZE.width)
        .min(screen.size.width);
    let height = bounds
        .size
        .height
        .max(MIN_WINDOW_SIZE.height)
        .min(screen.size.height);
    let size = size(width, height);

    let min_x = screen.origin.x;
    let min_y = screen.origin.y;
    let max_x = (screen.origin.x + screen.size.width - size.width).max(min_x);
    let max_y = (screen.origin.y + screen.size.height - size.height).max(min_y);

    Bounds {
        origin: point(
            bounds.origin.x.clamp(min_x, max_x),
            bounds.origin.y.clamp(min_y, max_y),
        ),
        size,
    }
}

fn bounds_on_display(frame: &Frame, display: &dyn PlatformDisplay) -> Bounds<Pixels> {
    let screen = display.bounds();
    let size = size(
        px(frame.width.max(f32::from(MIN_WINDOW_SIZE.width))),
        px(frame.height.max(f32::from(MIN_WINDOW_SIZE.height))),
    );
    let origin = match (frame.rel_x, frame.rel_y) {
        (Some(rel_x), Some(rel_y)) => {
            point(screen.origin.x + px(rel_x), screen.origin.y + px(rel_y))
        }
        _ => point(px(frame.x), px(frame.y)),
    };
    constrain_to_display(Bounds { origin, size }, screen)
}

fn target_display(cx: &App, frame: &Frame) -> Option<Rc<dyn PlatformDisplay>> {
    if let Some(uuid) = frame.display_uuid.as_deref() {
        return find_display_by_uuid(cx, uuid).or_else(|| cx.primary_display());
    }

    let absolute = Bounds {
        origin: point(px(frame.x), px(frame.y)),
        size: size(px(frame.width), px(frame.height)),
    };
    cx.displays()
        .into_iter()
        .find(|display| display.bounds().intersects(&absolute))
        .or_else(|| cx.primary_display())
}

fn with_placement(placement: Placement, bounds: Bounds<Pixels>) -> WindowBounds {
    match placement {
        Placement::Windowed => WindowBounds::Windowed(bounds),
        Placement::Maximized => WindowBounds::Maximized(bounds),
        Placement::Fullscreen => WindowBounds::Fullscreen(bounds),
    }
}

pub fn persist(window: &Window, cx: &App) {
    let display = window.display(cx);
    let frame = from_bounds(window.window_bounds(), display.as_deref());
    if load().as_ref() == Some(&frame) {
        return;
    }
    save(frame);
}

pub fn restore(cx: &App) -> RestoredWindow {
    let Some(frame) = load() else {
        return RestoredWindow {
            bounds: WindowBounds::Windowed(Bounds::centered(None, DEFAULT_WINDOW_SIZE, cx)),
            display_id: cx.primary_display().map(|display| display.id()),
        };
    };

    let display = target_display(cx, &frame);
    let display_id = display.as_ref().map(|display| display.id());
    let bounds = match display.as_deref() {
        Some(display) => bounds_on_display(&frame, display),
        None => Bounds::centered(None, DEFAULT_WINDOW_SIZE, cx),
    };

    RestoredWindow {
        bounds: with_placement(frame.placement, bounds),
        display_id,
    }
}
