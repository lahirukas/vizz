use crate::beatclock::BeatClock;
use crate::presets::Flux;
use crate::sketch::Sketch;
use crate::theme::Theme;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use vizz_dsp::{AnalysisSnapshot, SnapshotBus};
use vizz_render::{Gpu, Params, Preview, Quality, Style, Surface, WidgetKind};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::DwmFlush;
use windows::Win32::UI::Shell::{
    SHQueryUserNotificationState, QUNS_BUSY, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN,
};

#[derive(Clone, Copy)]
pub struct WidgetSpec {
    pub hwnd: isize,
    pub kind: WidgetKind,
    pub style: Style,
    pub quality: Quality,
    pub width: u32,
    pub height: u32,
    pub palette: Option<[[f32; 3]; 3]>,
    pub opacity: f32,
}

#[derive(Clone, Copy, PartialEq)]
pub struct PreviewSpec {
    pub kind: WidgetKind,
    pub style: Style,
    pub quality: Quality,
    pub width: u32,
    pub height: u32,
    pub palette: Option<[[f32; 3]; 3]>,
}

#[derive(Default)]
pub struct PreviewFrame {
    pub width: u32,
    pub height: u32,
    pub serial: u64,
    pub rgb: Vec<u8>,
}

pub type PreviewSlot = Arc<Mutex<PreviewFrame>>;

pub enum Cmd {
    Add(WidgetSpec),
    Remove(isize),
    Resize(isize, u32, u32),
    Look(isize, WidgetKind, Style, Quality),
    NextPreset,
    Edit(bool),
    Theme(Theme),
    WhistleBus(Option<Arc<SnapshotBus>>),
    Preview(Option<PreviewSpec>),
    Opacity(isize, f32),
    Palette(isize, Option<[[f32; 3]; 3]>),
    Timing { av_offset_ms: f32, idle_after_secs: f32 },
    Quit,
}

#[derive(Clone, Copy)]
pub struct RenderOpts {
    pub fps: u32,
    pub av_offset_ms: f32,
    pub idle_after_secs: f32,
    pub warp: bool,
}

struct Live {
    spec: WidgetSpec,
    surface: Surface,
    base: Params,
}

pub struct Renderer {
    pub preview: PreviewSlot,
    tx: Sender<Cmd>,
    thread: Option<JoinHandle<()>>,
}

pub fn base_params(spec: &WidgetSpec, theme: &Theme, edit: bool) -> Params {
    let pal = theme.palette_for(spec.style);
    let p = spec.palette.unwrap_or([pal.c0, pal.c1, pal.c2]);
    let rgba = |c: [f32; 3], a: f32| [c[0], c[1], c[2], a];
    Params {
        res: [spec.width as f32, spec.height as f32],
        edit: edit as u32 as f32,
        style: spec.style.index(),
        quality: spec.quality.index(),
        dark: theme.dark as u32 as f32,
        c0: rgba(p[0], spec.opacity),
        // c1.a > 1.5 tells shaders the widget uses a user palette preset.
        c1: rgba(p[1], if spec.palette.is_some() { 2.0 } else { 1.0 }),
        c2: rgba(p[2], 1.0),
        ..Default::default()
    }
}

fn with_base(shared: &Params, base: &Params) -> Params {
    Params {
        res: base.res,
        edit: base.edit,
        style: base.style,
        dark: base.dark,
        quality: base.quality,
        c0: base.c0,
        c1: base.c1,
        c2: base.c2,
        ..*shared
    }
}

fn fullscreen_app_running() -> bool {
    matches!(
        unsafe { SHQueryUserNotificationState() },
        Ok(s) if s == QUNS_BUSY || s == QUNS_RUNNING_D3D_FULL_SCREEN || s == QUNS_PRESENTATION_MODE
    )
}

impl Renderer {
    pub fn start(bus: Arc<SnapshotBus>, theme: Theme, opts: RenderOpts) -> Result<Self, String> {
        let (tx, rx) = channel();
        let (ready_tx, ready_rx) = channel();
        let preview = PreviewSlot::default();
        let slot = preview.clone();
        let thread = std::thread::Builder::new()
            .name("render".into())
            .stack_size(512 * 1024)
            .spawn(move || match Gpu::new(opts.warp) {
                Ok(gpu) => {
                    let _ = ready_tx.send(Ok(gpu.describe()));
                    run(gpu, rx, &bus, theme, opts, &slot);
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e.to_string()));
                }
            })
            .map_err(|e| e.to_string())?;
        match ready_rx.recv() {
            Ok(Ok(desc)) => {
                eprintln!("renderer: {desc}");
                Ok(Self { preview, tx, thread: Some(thread) })
            }
            Ok(Err(e)) => Err(format!("gpu: {e}")),
            Err(_) => Err("render thread died".into()),
        }
    }

    pub fn send(&self, cmd: Cmd) {
        let _ = self.tx.send(cmd);
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        let _ = self.tx.send(Cmd::Quit);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn preview_base(spec: &PreviewSpec, theme: &Theme) -> Params {
    let ws = WidgetSpec {
        hwnd: 0,
        kind: spec.kind,
        style: spec.style,
        quality: spec.quality,
        width: spec.width,
        height: spec.height,
        palette: spec.palette,
        opacity: 1.0,
    };
    base_params(&ws, theme, false)
}

fn run(gpu: Gpu, rx: Receiver<Cmd>, bus: &SnapshotBus, mut theme: Theme, mut opts: RenderOpts, slot: &PreviewSlot) {
    let mut widgets: Vec<Live> = Vec::new();
    let mut edit = false;
    let mut clock = BeatClock::new(opts.av_offset_ms);
    let mut flux = Flux::new();
    let mut sketch = Sketch::new();
    let mut last_frame = Instant::now();
    let mut snap = AnalysisSnapshot::EMPTY;
    let mut whistle_bus: Option<Arc<SnapshotBus>> = None;
    let mut wsnap = AnalysisSnapshot::EMPTY;
    let mut preview: Option<(PreviewSpec, Preview, Params)> = None;
    let mut rgb = Vec::new();
    let mut fs_check = Instant::now() - Duration::from_secs(5);
    let mut fs_busy = false;
    let mut idle_cleared = false;
    let start = Instant::now();
    let mut frames = 0u64;
    loop {
        while let Ok(cmd) = rx.try_recv() {
            match cmd {
                Cmd::Add(spec) => match Surface::new(&gpu, HWND(spec.hwnd as *mut _), spec.width, spec.height) {
                    Ok(surface) => widgets.push(Live { spec, surface, base: base_params(&spec, &theme, edit) }),
                    Err(e) => eprintln!("surface: {e}"),
                },
                Cmd::Remove(hwnd) => widgets.retain(|w| w.spec.hwnd != hwnd),
                Cmd::Resize(hwnd, width, height) => {
                    if let Some(w) = widgets.iter_mut().find(|w| w.spec.hwnd == hwnd) {
                        if let Err(e) = w.surface.resize(&gpu, width.max(1), height.max(1)) {
                            eprintln!("resize: {e}");
                        }
                        w.spec.width = width;
                        w.spec.height = height;
                        w.base = base_params(&w.spec, &theme, edit);
                    }
                }
                Cmd::Look(hwnd, kind, style, quality) => {
                    if let Some(w) = widgets.iter_mut().find(|w| w.spec.hwnd == hwnd) {
                        w.spec.kind = kind;
                        w.spec.style = style;
                        w.spec.quality = quality;
                        w.base = base_params(&w.spec, &theme, edit);
                    }
                }
                Cmd::NextPreset => {
                    flux.pinned = None;
                    flux.next();
                    eprintln!("flux preset: {}", flux.current_name());
                }
                Cmd::Edit(on) => {
                    edit = on;
                    widgets.iter_mut().for_each(|w| w.base = base_params(&w.spec, &theme, edit));
                }
                Cmd::Theme(t) => {
                    theme = t;
                    widgets.iter_mut().for_each(|w| w.base = base_params(&w.spec, &theme, edit));
                    if let Some((s, _, b)) = &mut preview {
                        *b = preview_base(s, &theme);
                    }
                }
                Cmd::Palette(hwnd, pal) => {
                    if let Some(w) = widgets.iter_mut().find(|w| w.spec.hwnd == hwnd) {
                        w.spec.palette = pal;
                        w.base = base_params(&w.spec, &theme, edit);
                    }
                }
                Cmd::Opacity(hwnd, o) => {
                    if let Some(w) = widgets.iter_mut().find(|w| w.spec.hwnd == hwnd) {
                        w.spec.opacity = o;
                        w.base = base_params(&w.spec, &theme, edit);
                    }
                }
                Cmd::Timing { av_offset_ms, idle_after_secs } => {
                    opts.av_offset_ms = av_offset_ms;
                    opts.idle_after_secs = idle_after_secs;
                    clock.set_av_offset(av_offset_ms);
                }
                Cmd::Preview(spec) => {
                    preview = spec.and_then(|s| match Preview::new(&gpu, s.width.max(16), s.height.max(16)) {
                        Ok(pv) => Some((s, pv, preview_base(&s, &theme))),
                        Err(e) => {
                            eprintln!("preview: {e}");
                            None
                        }
                    });
                }
                Cmd::WhistleBus(b) => {
                    whistle_bus = b;
                    wsnap = AnalysisSnapshot::EMPTY;
                }
                Cmd::Quit => {
                    eprintln!("rendered {frames} frames in {:.1}s", start.elapsed().as_secs_f64());
                    return;
                }
            }
        }
        if fs_check.elapsed() >= Duration::from_secs(1) {
            fs_check = Instant::now();
            fs_busy = fullscreen_app_running();
        }
        bus.read(&mut snap);
        let mut silent = snap.silent_secs;
        if let Some(w) = &whistle_bus {
            w.read(&mut wsnap);
            snap.whistle = wsnap.whistle;
            snap.pitch_hz = wsnap.pitch_hz;
            silent = silent.min(wsnap.silent_secs);
        }
        let idle = !edit && preview.is_none() && (widgets.is_empty() || fs_busy || silent > opts.idle_after_secs);
        if idle {
            if !idle_cleared {
                for w in &mut widgets {
                    let mut p = w.base;
                    p.c0[3] = 0.0;
                    w.surface.render(&gpu, w.spec.kind, w.spec.quality, &p);
                    let _ = w.surface.present();
                }
                idle_cleared = true;
            }
            std::thread::sleep(Duration::from_millis(100));
            continue;
        }
        idle_cleared = false;
        let now = Instant::now();
        let dt = now.duration_since(last_frame).as_secs_f32().clamp(1.0 / 240.0, 0.1);
        last_frame = now;
        let mut shared = clock.params(&snap, now, Params::default());
        flux.step(&mut shared, dt);
        // flux[6].w in HLSL: continuous beat position
        shared.flux[27] = clock.beat_pos();
        let aspect = widgets
            .iter()
            .find(|w| w.spec.kind == WidgetKind::Sketch)
            .map_or(1.0, |w| w.spec.width as f32 / w.spec.height.max(1) as f32);
        let sk = sketch.step(&shared, dt, aspect);
        for w in &mut widgets {
            let mut p = with_base(&shared, &w.base);
            if w.spec.kind == WidgetKind::Sketch {
                sk.apply(&mut p);
            }
            w.surface.render(&gpu, w.spec.kind, w.spec.quality, &p);
            if let Err(e) = w.surface.present() {
                eprintln!("present: {e}");
            }
        }
        if let Some((spec, pv, base)) = &mut preview {
            let mut p = with_base(&shared, base);
            if spec.kind == WidgetKind::Sketch {
                sk.apply(&mut p);
            }
            pv.render(&gpu, spec.kind, spec.quality, &p);
            let bg = if theme.dark { [24, 24, 28] } else { [236, 236, 240] };
            if pv.read_rgb(&gpu, bg, &mut rgb).is_ok() {
                if let Ok(mut f) = slot.lock() {
                    std::mem::swap(&mut f.rgb, &mut rgb);
                    (f.width, f.height) = (pv.width, pv.height);
                    f.serial += 1;
                }
            }
        }
        frames += 1;
        for _ in 0..if opts.fps <= 30 { 2 } else { 1 } {
            if unsafe { DwmFlush() }.is_err() {
                std::thread::sleep(Duration::from_millis(16));
            }
        }
    }
}
