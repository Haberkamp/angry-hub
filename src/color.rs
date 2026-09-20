use gpui::{App, Global, Hsla, Window, WindowAppearance, rgb};

fn h(c: u32) -> Hsla {
    rgb(c).into()
}

const GRAY_LIGHT: [u32; 12] = [
    0xFCFCFC, 0xF9F9F9, 0xF0F0F0, 0xE8E8E8, 0xE0E0E0, 0xD9D9D9, 0xCECECE, 0xBBBBBB, 0x8D8D8D,
    0x838383, 0x646464, 0x202020,
];
const GRAY_DARK: [u32; 12] = [
    0x111111, 0x191919, 0x222222, 0x2A2A2A, 0x313131, 0x3A3A3A, 0x484848, 0x606060, 0x6E6E6E,
    0x7B7B7B, 0xB4B4B4, 0xEEEEEE,
];

const RED_LIGHT: [u32; 12] = [
    0xFFFCFC, 0xFFF7F7, 0xFEEBEC, 0xFFDBDC, 0xFFCDCE, 0xFDBDBE, 0xF4A9AA, 0xEB8E90, 0xE5484D,
    0xDC3E42, 0xCE2C31, 0x641723,
];
const RED_DARK: [u32; 12] = [
    0x191111, 0x201314, 0x3B1219, 0x500F1C, 0x611623, 0x72232D, 0x8C333A, 0xB54548, 0xE5484D,
    0xEC5D5E, 0xFF9592, 0xFFD1D9,
];

const GREEN_LIGHT: [u32; 12] = [
    0xFBFEFC, 0xF4FBF6, 0xE6F6EB, 0xD6F1DF, 0xC4E8D1, 0xADDDC0, 0x8ECEAA, 0x5BB98B, 0x30A46C,
    0x2B9A66, 0x218358, 0x193B2D,
];
const GREEN_DARK: [u32; 12] = [
    0x0E1512, 0x121B17, 0x132D21, 0x113B29, 0x174933, 0x20573E, 0x28684A, 0x2F7C57, 0x30A46C,
    0x3CB179, 0x4CC38A, 0xB1F1CB,
];

const BLUE_LIGHT: [u32; 12] = [
    0xFBFDFF, 0xF4FAFF, 0xE6F4FE, 0xD5EFFF, 0xC2E5FF, 0xACD8FC, 0x8EC8F6, 0x5EB1EF, 0x0090FF,
    0x0588F0, 0x0D74CE, 0x113264,
];
const BLUE_DARK: [u32; 12] = [
    0x0D1520, 0x111927, 0x0D2847, 0x003362, 0x004074, 0x104D87, 0x205D9E, 0x2870BD, 0x0090FF,
    0x3B9EFF, 0x70B8FF, 0xC2E6FF,
];

const VIOLET_LIGHT: [u32; 12] = [
    0xFDFCFE, 0xFAF8FF, 0xF4F0FE, 0xEBE4FF, 0xE1D9FF, 0xD4CAFE, 0xC2B5F5, 0xAA99EC, 0x6E56CF,
    0x654DC4, 0x6550B9, 0x2F265F,
];
const VIOLET_DARK: [u32; 12] = [
    0x14121F, 0x1B1525, 0x291F43, 0x33255B, 0x3C2E69, 0x473876, 0x56468B, 0x6958AD, 0x6E56CF,
    0x7D66D9, 0xBAA7FF, 0xE2DDFE,
];

const AMBER_LIGHT: [u32; 12] = [
    0xFEFDFB, 0xFEFBE9, 0xFFF7C2, 0xFFEE9C, 0xFBE577, 0xF3D673, 0xE9C162, 0xE2A336, 0xFFC53D,
    0xFFBA18, 0xAB6400, 0x4F3422,
];
const AMBER_DARK: [u32; 12] = [
    0x16120C, 0x1D180F, 0x302008, 0x3F2700, 0x4D3000, 0x5C3D05, 0x714F19, 0x8F6424, 0xFFC53D,
    0xFFD60A, 0xFFCA16, 0xFFE7B3,
];

fn step(light: &[u32; 12], dark: &[u32; 12], n: usize, cx: &App) -> Hsla {
    let i = n.saturating_sub(1).min(11);
    h(if is_dark(cx) { dark[i] } else { light[i] })
}

fn gray(n: usize, cx: &App) -> Hsla {
    step(&GRAY_LIGHT, &GRAY_DARK, n, cx)
}

fn red(n: usize, cx: &App) -> Hsla {
    step(&RED_LIGHT, &RED_DARK, n, cx)
}

fn green(n: usize, cx: &App) -> Hsla {
    step(&GREEN_LIGHT, &GREEN_DARK, n, cx)
}

fn blue(n: usize, cx: &App) -> Hsla {
    step(&BLUE_LIGHT, &BLUE_DARK, n, cx)
}

fn violet(n: usize, cx: &App) -> Hsla {
    step(&VIOLET_LIGHT, &VIOLET_DARK, n, cx)
}

fn amber(n: usize, cx: &App) -> Hsla {
    step(&AMBER_LIGHT, &AMBER_DARK, n, cx)
}

pub fn white() -> Hsla {
    h(0xFFFFFF)
}

fn is_dark(cx: &App) -> bool {
    cx.try_global::<Theme>()
        .map(Theme::is_dark)
        .unwrap_or(false)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemePreference {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemePreference {
    pub fn as_id(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    pub fn from_id(id: &str) -> Self {
        match id {
            "light" => Self::Light,
            "dark" => Self::Dark,
            _ => Self::System,
        }
    }
}

#[derive(Clone)]
pub struct Theme {
    pub preference: ThemePreference,
    pub system_dark: bool,
}

impl Global for Theme {}

impl Theme {
    pub fn is_dark(&self) -> bool {
        match self.preference {
            ThemePreference::System => self.system_dark,
            ThemePreference::Light => false,
            ThemePreference::Dark => true,
        }
    }
}

fn appearance_is_dark(appearance: WindowAppearance) -> bool {
    matches!(
        appearance,
        WindowAppearance::Dark | WindowAppearance::VibrantDark
    )
}

pub fn init_theme(window: &Window, cx: &mut App) {
    cx.set_global(Theme {
        preference: crate::prefs::Prefs::load_theme(),
        system_dark: appearance_is_dark(window.appearance()),
    });
}

pub fn sync_system_appearance(window: &Window, cx: &mut App) {
    let system_dark = appearance_is_dark(window.appearance());
    let Some(theme) = cx.try_global::<Theme>() else {
        return;
    };
    if theme.system_dark == system_dark {
        return;
    }
    let mut next = theme.clone();
    next.system_dark = system_dark;
    cx.set_global(next);
}

pub fn set_preference(preference: ThemePreference, cx: &mut App) {
    crate::prefs::Prefs::save_theme(preference);
    let system_dark = cx
        .try_global::<Theme>()
        .map(|t| t.system_dark)
        .unwrap_or(false);
    cx.set_global(Theme {
        preference,
        system_dark,
    });
}

pub fn preference(cx: &App) -> ThemePreference {
    cx.try_global::<Theme>()
        .map(|t| t.preference)
        .unwrap_or_default()
}

pub mod surface {
    use super::*;

    pub fn default(cx: &App) -> Hsla {
        if is_dark(cx) { gray(1, cx) } else { white() }
    }

    pub fn sunken(cx: &App) -> Hsla {
        if is_dark(cx) {
            gray(3, cx)
        } else {
            gray(2, cx)
        }
    }

    pub fn overlay(cx: &App) -> Hsla {
        if is_dark(cx) { gray(2, cx) } else { white() }
    }

    pub fn inverse(cx: &App) -> Hsla {
        gray(12, cx)
    }
}

pub mod interaction {
    use super::*;

    pub fn hovered(cx: &App) -> Hsla {
        gray(3, cx)
    }

    pub fn pressed(cx: &App) -> Hsla {
        if is_dark(cx) {
            gray(5, cx)
        } else {
            gray(4, cx)
        }
    }
}

pub mod border {
    use super::*;

    pub fn default(cx: &App) -> Hsla {
        gray(6, cx)
    }

    pub fn strong(cx: &App) -> Hsla {
        gray(8, cx)
    }
}

pub mod text {
    use super::*;

    pub fn primary(cx: &App) -> Hsla {
        gray(12, cx)
    }

    pub fn secondary(cx: &App) -> Hsla {
        gray(10, cx)
    }

    pub fn inverse(cx: &App) -> Hsla {
        gray(1, cx)
    }

    pub fn danger(cx: &App) -> Hsla {
        red(9, cx)
    }
}

pub mod icon {
    use super::*;

    pub fn default(cx: &App) -> Hsla {
        gray(12, cx)
    }

    pub fn subtle(cx: &App) -> Hsla {
        gray(9, cx)
    }

    #[allow(dead_code)]
    pub fn inverse(cx: &App) -> Hsla {
        gray(1, cx)
    }

    pub fn on_solid(_cx: &App) -> Hsla {
        white()
    }
}

pub mod bg {
    use super::*;

    pub fn selected(cx: &App) -> Hsla {
        blue(9, cx)
    }
}

pub mod button {
    use super::*;

    pub mod primary {
        use super::*;

        pub fn bg(cx: &App) -> Hsla {
            gray(12, cx)
        }

        pub fn bg_hover(cx: &App) -> Hsla {
            gray(11, cx)
        }

        pub fn bg_active(cx: &App) -> Hsla {
            gray(10, cx)
        }

        pub fn fg(cx: &App) -> Hsla {
            gray(1, cx)
        }
    }

    pub mod danger {
        use super::*;

        pub fn bg(cx: &App) -> Hsla {
            red(9, cx)
        }

        pub fn bg_hover(cx: &App) -> Hsla {
            red(10, cx)
        }

        pub fn bg_active(cx: &App) -> Hsla {
            red(11, cx)
        }

        pub fn fg() -> Hsla {
            white()
        }
    }
}

pub mod status {
    use super::*;

    pub fn open(cx: &App) -> Hsla {
        if is_dark(cx) {
            green(10, cx)
        } else {
            green(9, cx)
        }
    }

    pub fn draft(cx: &App) -> Hsla {
        gray(9, cx)
    }

    pub fn closed(cx: &App) -> Hsla {
        red(9, cx)
    }

    pub fn merged(cx: &App) -> Hsla {
        if is_dark(cx) {
            violet(9, cx)
        } else {
            violet(10, cx)
        }
    }

    pub fn success(cx: &App) -> Hsla {
        open(cx)
    }

    pub fn failure(cx: &App) -> Hsla {
        red(9, cx)
    }

    pub fn pending(cx: &App) -> Hsla {
        if is_dark(cx) {
            amber(8, cx)
        } else {
            amber(11, cx)
        }
    }

    pub fn none(cx: &App) -> Hsla {
        gray(9, cx)
    }

    pub fn comment(cx: &App) -> Hsla {
        blue(9, cx)
    }

    pub fn approved(cx: &App) -> Hsla {
        success(cx)
    }

    pub fn changes_requested(cx: &App) -> Hsla {
        failure(cx)
    }

    pub fn reopened(cx: &App) -> Hsla {
        open(cx)
    }
}
