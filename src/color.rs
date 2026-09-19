use gpui::{Hsla, rgb};

fn h(c: u32) -> Hsla {
    rgb(c).into()
}

pub fn bg() -> Hsla {
    h(0x1e1e1e)
}

pub fn text() -> Hsla {
    h(0xffffff)
}

pub fn text_muted() -> Hsla {
    h(0x8b949e)
}

pub fn text_secondary() -> Hsla {
    h(0xaaaaaa)
}

pub fn text_error() -> Hsla {
    h(0xff6666)
}

pub fn surface() -> Hsla {
    h(0x2a2a2a)
}

pub fn surface_hover() -> Hsla {
    h(0x3d3d3d)
}

pub fn surface_raised() -> Hsla {
    h(0x2d2d2d)
}

pub fn surface_subtle() -> Hsla {
    h(0x333333)
}

pub fn surface_subtle_active() -> Hsla {
    h(0x3a3a3a)
}

pub fn surface_active() -> Hsla {
    h(0x444444)
}

pub fn border() -> Hsla {
    h(0x444444)
}

pub fn border_strong() -> Hsla {
    h(0x555555)
}

pub fn accent() -> Hsla {
    h(0x6ea8fe)
}

pub fn accent_fill() -> Hsla {
    h(0x3b6ea8)
}

pub fn success() -> Hsla {
    h(0x3fb950)
}

pub fn danger() -> Hsla {
    h(0xf85149)
}

pub fn warning() -> Hsla {
    h(0xd29922)
}

pub fn merged() -> Hsla {
    h(0xa371f7)
}

pub fn draft() -> Hsla {
    h(0x6e7681)
}

pub fn comment() -> Hsla {
    h(0x58a6ff)
}
