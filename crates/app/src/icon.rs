use windows::core::Result;
use windows::Win32::Graphics::Gdi::{
    CreateBitmap, CreateDIBSection, DeleteObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
};
use windows::Win32::UI::WindowsAndMessaging::{CreateIconIndirect, HICON, ICONINFO};

const PULSE: [[f32; 2]; 7] =
    [[-0.95, 0.0], [-0.38, 0.0], [-0.2, 0.42], [0.02, -0.52], [0.2, 0.24], [0.34, 0.0], [0.95, 0.0]];

fn seg_dist(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let ab = [b[0] - a[0], b[1] - a[1]];
    let ap = [p[0] - a[0], p[1] - a[1]];
    let h = ((ap[0] * ab[0] + ap[1] * ab[1]) / (ab[0] * ab[0] + ab[1] * ab[1])).clamp(0.0, 1.0);
    ((ap[0] - ab[0] * h).powi(2) + (ap[1] - ab[1] * h).powi(2)).sqrt()
}

fn shade(x: f32, y: f32) -> [f32; 4] {
    let r = (x * x + y * y).sqrt();
    let ring = (r - 0.74).abs() < 0.14;
    let line = PULSE.windows(2).any(|w| seg_dist([x, y], w[0], w[1]) < 0.09);
    if line {
        return [1.0, 1.0, 1.0, 1.0];
    }
    if ring {
        let t = (y.atan2(x) / std::f32::consts::TAU + 0.5).fract();
        let k = (t * 2.0 - 1.0).abs();
        let cyan = [0.0, 0.85, 1.0];
        let magenta = [1.0, 0.25, 0.75];
        return [
            cyan[0] + (magenta[0] - cyan[0]) * k,
            cyan[1] + (magenta[1] - cyan[1]) * k,
            cyan[2] + (magenta[2] - cyan[2]) * k,
            1.0,
        ];
    }
    [0.0; 4]
}

pub fn pixels(size: u32) -> Vec<u8> {
    let ss = 4;
    let mut out = vec![0u8; (size * size * 4) as usize];
    for py in 0..size {
        for px in 0..size {
            let mut acc = [0.0f32; 4];
            for sy in 0..ss {
                for sx in 0..ss {
                    let x = ((px * ss + sx) as f32 + 0.5) / (size * ss) as f32 * 2.0 - 1.0;
                    let y = 1.0 - ((py * ss + sy) as f32 + 0.5) / (size * ss) as f32 * 2.0;
                    let c = shade(x, y);
                    for k in 0..3 {
                        acc[k] += c[k] * c[3];
                    }
                    acc[3] += c[3];
                }
            }
            let n = (ss * ss) as f32;
            let a = acc[3] / n;
            let o = ((py * size + px) * 4) as usize;
            let un = |v: f32| if a > 0.0 { ((v / n / a) * 255.0).round().clamp(0.0, 255.0) as u8 } else { 0 };
            out[o] = un(acc[2]);
            out[o + 1] = un(acc[1]);
            out[o + 2] = un(acc[0]);
            out[o + 3] = (a * 255.0).round() as u8;
        }
    }
    out
}

pub fn create(size: u32) -> Result<HICON> {
    let bgra = pixels(size);
    unsafe {
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: size as i32,
                biHeight: -(size as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let color = CreateDIBSection(None, &bmi, DIB_RGB_COLORS, &mut bits, None, 0)?;
        std::ptr::copy_nonoverlapping(bgra.as_ptr(), bits as *mut u8, bgra.len());
        let mask = CreateBitmap(size as i32, size as i32, 1, 1, None);
        let info = ICONINFO { fIcon: true.into(), xHotspot: 0, yHotspot: 0, hbmMask: mask, hbmColor: color };
        let icon = CreateIconIndirect(&info);
        let _ = DeleteObject(color.into());
        let _ = DeleteObject(mask.into());
        icon
    }
}
