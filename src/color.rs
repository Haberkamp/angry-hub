use gpui::{Hsla, rgb};

fn h(c: u32) -> Hsla {
    rgb(c).into()
}

#[allow(dead_code)]
pub mod gray {
    use super::h;
    use gpui::Hsla;

    pub fn s1() -> Hsla {
        h(0xFCFCFC)
    }
    pub fn s2() -> Hsla {
        h(0xF9F9F9)
    }
    pub fn s3() -> Hsla {
        h(0xF0F0F0)
    }
    pub fn s4() -> Hsla {
        h(0xE8E8E8)
    }
    pub fn s5() -> Hsla {
        h(0xE0E0E0)
    }
    pub fn s6() -> Hsla {
        h(0xD9D9D9)
    }
    pub fn s7() -> Hsla {
        h(0xCECECE)
    }
    pub fn s8() -> Hsla {
        h(0xBBBBBB)
    }
    pub fn s9() -> Hsla {
        h(0x8D8D8D)
    }
    pub fn s10() -> Hsla {
        h(0x838383)
    }
    pub fn s11() -> Hsla {
        h(0x646464)
    }
    pub fn s12() -> Hsla {
        h(0x202020)
    }
}

#[allow(dead_code)]
pub mod red {
    use super::h;
    use gpui::Hsla;

    pub fn s1() -> Hsla {
        h(0xFFFCFC)
    }
    pub fn s2() -> Hsla {
        h(0xFFF7F7)
    }
    pub fn s3() -> Hsla {
        h(0xFEEBEC)
    }
    pub fn s4() -> Hsla {
        h(0xFFDBDC)
    }
    pub fn s5() -> Hsla {
        h(0xFFCDCE)
    }
    pub fn s6() -> Hsla {
        h(0xFDBDBE)
    }
    pub fn s7() -> Hsla {
        h(0xF4A9AA)
    }
    pub fn s8() -> Hsla {
        h(0xEB8E90)
    }
    pub fn s9() -> Hsla {
        h(0xE5484D)
    }
    pub fn s10() -> Hsla {
        h(0xDC3E42)
    }
    pub fn s11() -> Hsla {
        h(0xCE2C31)
    }
    pub fn s12() -> Hsla {
        h(0x641723)
    }
}

#[allow(dead_code)]
pub mod green {
    use super::h;
    use gpui::Hsla;

    pub fn s1() -> Hsla {
        h(0xFBFEFC)
    }
    pub fn s2() -> Hsla {
        h(0xF4FBF6)
    }
    pub fn s3() -> Hsla {
        h(0xE6F6EB)
    }
    pub fn s4() -> Hsla {
        h(0xD6F1DF)
    }
    pub fn s5() -> Hsla {
        h(0xC4E8D1)
    }
    pub fn s6() -> Hsla {
        h(0xADDDC0)
    }
    pub fn s7() -> Hsla {
        h(0x8ECEAA)
    }
    pub fn s8() -> Hsla {
        h(0x5BB98B)
    }
    pub fn s9() -> Hsla {
        h(0x30A46C)
    }
    pub fn s10() -> Hsla {
        h(0x2B9A66)
    }
    pub fn s11() -> Hsla {
        h(0x218358)
    }
    pub fn s12() -> Hsla {
        h(0x193B2D)
    }
}

#[allow(dead_code)]
pub mod blue {
    use super::h;
    use gpui::Hsla;

    pub fn s1() -> Hsla {
        h(0xFBFDFF)
    }
    pub fn s2() -> Hsla {
        h(0xF4FAFF)
    }
    pub fn s3() -> Hsla {
        h(0xE6F4FE)
    }
    pub fn s4() -> Hsla {
        h(0xD5EFFF)
    }
    pub fn s5() -> Hsla {
        h(0xC2E5FF)
    }
    pub fn s6() -> Hsla {
        h(0xACD8FC)
    }
    pub fn s7() -> Hsla {
        h(0x8EC8F6)
    }
    pub fn s8() -> Hsla {
        h(0x5EB1EF)
    }
    pub fn s9() -> Hsla {
        h(0x0090FF)
    }
    pub fn s10() -> Hsla {
        h(0x0588F0)
    }
    pub fn s11() -> Hsla {
        h(0x0D74CE)
    }
    pub fn s12() -> Hsla {
        h(0x113264)
    }
}

pub fn white() -> Hsla {
    h(0xFFFFFF)
}
