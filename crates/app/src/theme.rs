use vizz_render::Style;
use windows::core::{w, PCWSTR};
use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub dark: bool,
    pub accent: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub c0: [f32; 3],
    pub c1: [f32; 3],
    pub c2: [f32; 3],
}

fn read_dword(key: PCWSTR, value: PCWSTR) -> Option<u32> {
    let mut data = 0u32;
    let mut size = 4u32;
    let ok = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key,
            value,
            RRF_RT_REG_DWORD,
            None,
            Some(&mut data as *mut u32 as *mut _),
            Some(&mut size),
        )
    };
    ok.is_ok().then_some(data)
}

impl Theme {
    pub fn current() -> Self {
        let dark = read_dword(
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("AppsUseLightTheme"),
        )
        .is_none_or(|v| v == 0);
        // DWM stores the accent as 0xAABBGGRR.
        let accent = read_dword(w!("Software\\Microsoft\\Windows\\DWM"), w!("AccentColor"))
            .map(|v| [(v & 0xff) as f32 / 255.0, ((v >> 8) & 0xff) as f32 / 255.0, ((v >> 16) & 0xff) as f32 / 255.0])
            .unwrap_or([0.0, 0.47, 0.84]);
        Self { dark, accent }
    }

    pub fn palette(&self) -> Palette {
        let (h, s, _) = rgb_to_hsv(self.accent);
        let s = s.max(0.78);
        Palette { c0: hsv_to_rgb(h, s, 1.0), c1: hsv_to_rgb(h + 0.12, s, 1.0), c2: hsv_to_rgb(h - 0.17, s * 0.95, 1.0) }
    }

    pub fn palette_for(&self, style: Style) -> Palette {
        let (h, s, v) = rgb_to_hsv(self.accent);
        match style {
            Style::Neon => self.palette(),
            Style::Fluent => {
                let v = if self.dark { v.max(0.85) } else { v.min(0.8) };
                Palette { c0: hsv_to_rgb(h, s, v), c1: hsv_to_rgb(h, s * 0.6, v), c2: hsv_to_rgb(h + 0.05, s * 0.3, v) }
            }
            Style::Synth => Palette { c0: [1.0, 0.16, 0.62], c1: [0.1, 0.85, 1.0], c2: [1.0, 0.62, 0.12] },
            Style::Liquid => Palette {
                c0: hsv_to_rgb(h - 0.05, 0.75, 1.0),
                c1: hsv_to_rgb(h + 0.2, 0.6, 1.0),
                c2: hsv_to_rgb(h - 0.3, 0.55, 1.0),
            },
        }
    }
}

pub fn rgb_to_hsv([r, g, b]: [f32; 3]) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d < 1e-6 {
        0.0
    } else if max == r {
        ((g - b) / d).rem_euclid(6.0) / 6.0
    } else if max == g {
        ((b - r) / d + 2.0) / 6.0
    } else {
        ((r - g) / d + 4.0) / 6.0
    };
    (h, if max > 0.0 { d / max } else { 0.0 }, max)
}

pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [f32; 3] {
    let h = h.rem_euclid(1.0) * 6.0;
    let c = v * s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    [r + m, g + m, b + m]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsv_roundtrip() {
        let c = [0.2, 0.6, 0.9];
        let (h, s, v) = rgb_to_hsv(c);
        let back = hsv_to_rgb(h, s, v);
        for i in 0..3 {
            assert!((c[i] - back[i]).abs() < 1e-4);
        }
    }
}

pub const PRESETS: [(&str, [[f32; 3]; 3]); 12] = [
    ("Sunset", [[1.0, 0.35, 0.2], [1.0, 0.75, 0.2], [0.55, 0.2, 0.75]]),
    ("Ocean", [[0.0, 0.45, 0.75], [0.0, 0.85, 0.8], [0.85, 0.95, 1.0]]),
    ("Forest", [[0.1, 0.45, 0.2], [0.55, 0.8, 0.25], [0.95, 0.85, 0.45]]),
    ("Aurora", [[0.1, 0.9, 0.6], [0.3, 0.5, 1.0], [0.85, 0.3, 0.95]]),
    ("Lava", [[0.8, 0.08, 0.05], [1.0, 0.45, 0.0], [1.0, 0.9, 0.4]]),
    ("Candy", [[1.0, 0.4, 0.7], [0.5, 0.85, 1.0], [1.0, 0.95, 0.5]]),
    ("Peacock", [[0.0, 0.35, 0.6], [0.0, 0.7, 0.55], [0.9, 0.75, 0.25]]),
    ("Coral reef", [[1.0, 0.45, 0.4], [0.0, 0.75, 0.75], [1.0, 0.85, 0.55]]),
    ("Galaxy", [[0.25, 0.1, 0.6], [0.85, 0.2, 0.75], [0.4, 0.8, 1.0]]),
    ("Citrus", [[1.0, 0.85, 0.0], [0.55, 0.9, 0.1], [1.0, 0.5, 0.0]]),
    ("Amethyst", [[0.35, 0.15, 0.55], [0.7, 0.45, 0.95], [0.95, 0.85, 1.0]]),
    ("Ice", [[0.6, 0.85, 1.0], [0.2, 0.5, 0.9], [0.95, 0.98, 1.0]]),
];
