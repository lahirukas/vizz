use crate::config::{Config, WidgetCfg};
use crate::desktop;
use crate::engine::Engine;
use crate::renderer::{Cmd, PreviewSpec, RenderOpts, Renderer, WidgetSpec};
use crate::theme::{Theme, PRESETS};
use crate::ui::{Flyout, PaletteSwatch, SettingsWindow, Vizz, WidgetRow};
use slint::{
    Color, ComponentHandle, Image, ModelRc, Rgb8Pixel, SharedPixelBuffer, SharedString, Timer, TimerMode, VecModel,
};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant};
use vizz_dsp::AnalysisSnapshot;
use vizz_render::{Quality, Style, WidgetKind};
use windows::core::{w, Result, HSTRING, PCWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromPoint, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows::Win32::UI::Shell::{
    ShellExecuteW, Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::*;

static EDIT: AtomicBool = AtomicBool::new(false);
static PINNED: AtomicBool = AtomicBool::new(true);
static TOPMOST: AtomicBool = AtomicBool::new(false);
static EMBEDDED: AtomicBool = AtomicBool::new(false);
static MOVED: AtomicBool = AtomicBool::new(false);
static THEME_CHANGED: AtomicBool = AtomicBool::new(false);
static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);

const WIDGET_CLASS: PCWSTR = w!("Vizz.Widget");
const TRAY_CLASS: PCWSTR = w!("Vizz.Tray");
const FLYOUT_TITLE: PCWSTR = w!("Vizz quick settings");
const WM_APP_TRAY: u32 = WM_APP + 1;
const WM_APP_MENU: u32 = WM_APP + 2;
const WM_APP_DONE: u32 = WM_APP + 3;
const WM_APP_READD: u32 = WM_APP + 4;
const WM_APP_FLYOUT: u32 = WM_APP + 5;
const WM_APP_SHOW: u32 = WM_APP + 6;
const PREVIEW_SIZE: (u32, u32) = (960, 540);
const SWATCHES_PER_ROW: usize = 6;
const RESIZE_BORDER: i32 = 10;
const MIN_W: i32 = 120;
const MIN_H: i32 = 80;
const TICK: Duration = Duration::from_millis(33);
const SAVE_DELAY: Duration = Duration::from_millis(600);

const ID_ADD: usize = 1000;
const ID_EDIT: usize = 1100;
const ID_PIN: usize = 1101;
const ID_TOP: usize = 1102;
const ID_QUIT: usize = 1103;
const ID_NEXT_PRESET: usize = 1104;
const ID_SETTINGS: usize = 1105;
const ID_STYLE_ALL: usize = 1200;
const ID_QUALITY_ALL: usize = 1300;
const ID_QUALITY: usize = 2400;
const ID_KIND: usize = 2000;
const ID_STYLE: usize = 2100;
const ID_SIZE: usize = 2200;
const ID_REMOVE: usize = 2300;
const ID_DONE: usize = 2301;
const SIZES: [(&str, i32, i32); 4] =
    [("Small", 220, 220), ("Medium", 320, 320), ("Large", 480, 480), ("Wallpaper", 0, 0)];
const INPUTS: [&str; 2] = ["loopback", "mic"];
const WHISTLE_INPUTS: [&str; 2] = ["mic", "same"];

pub struct GuiOpts {
    pub exit_after: Option<f64>,
    pub edit: bool,
    pub warp: bool,
    pub background: bool,
    pub config_path: PathBuf,
}

thread_local! {
    static APP: RefCell<Option<Rc<RefCell<App>>>> = const { RefCell::new(None) };
}

// Runs f on the app unless it is already borrowed (e.g. inside a modal menu loop).
fn with_app(f: impl FnOnce(&mut App)) {
    let app = APP.with(|a| a.borrow().clone());
    if let Some(app) = app {
        if let Ok(mut g) = app.try_borrow_mut() {
            f(&mut g);
        }
    }
}

fn ex_style(pinned: bool, edit: bool, topmost: bool) -> WINDOW_EX_STYLE {
    let mut s = WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOOLWINDOW | WS_EX_LAYERED;
    if !edit {
        s |= WS_EX_NOACTIVATE;
        if pinned || topmost {
            s |= WS_EX_TRANSPARENT;
        }
    }
    s
}

fn is_theme_change(lp: LPARAM) -> bool {
    if lp.0 == 0 {
        return false;
    }
    let s = unsafe { PCWSTR(lp.0 as *const u16).to_string() }.unwrap_or_default();
    s == "ImmersiveColorSet"
}

fn hit_test(hwnd: HWND, lp: LPARAM) -> u32 {
    let (x, y) = ((lp.0 & 0xffff) as i16 as i32, ((lp.0 >> 16) & 0xffff) as i16 as i32);
    let mut r = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut r) }.is_err() {
        return HTCAPTION;
    }
    let (l, t) = (x - r.left < RESIZE_BORDER, y - r.top < RESIZE_BORDER);
    let (rt, b) = (r.right - x <= RESIZE_BORDER, r.bottom - y <= RESIZE_BORDER);
    match (l, t, rt, b) {
        (true, true, _, _) => HTTOPLEFT,
        (_, true, true, _) => HTTOPRIGHT,
        (true, _, _, true) => HTBOTTOMLEFT,
        (_, _, true, true) => HTBOTTOMRIGHT,
        (true, _, _, _) => HTLEFT,
        (_, _, true, _) => HTRIGHT,
        (_, true, _, _) => HTTOP,
        (_, _, _, true) => HTBOTTOM,
        _ => HTCAPTION,
    }
}

fn note_settings(msg: u32, lp: LPARAM) {
    if msg == WM_DWMCOLORIZATIONCOLORCHANGED || (msg == WM_SETTINGCHANGE && is_theme_change(lp)) {
        THEME_CHANGED.store(true, Ordering::Relaxed);
    }
}

extern "system" fn widget_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let edit = EDIT.load(Ordering::Relaxed);
    unsafe {
        match msg {
            WM_NCHITTEST => {
                return LRESULT(if edit { hit_test(hwnd, lp) } else { HTTRANSPARENT as u32 } as i32 as isize)
            }
            WM_NCRBUTTONUP | WM_RBUTTONUP if edit => {
                let _ = PostMessageW(Some(hwnd), WM_APP_MENU, WPARAM(0), LPARAM(0));
                return LRESULT(0);
            }
            WM_APP_MENU => {
                with_app(|a| a.widget_menu(hwnd));
                return LRESULT(0);
            }
            WM_APP_DONE => {
                with_app(|a| a.set_edit(false));
                return LRESULT(0);
            }
            WM_CONTEXTMENU => return LRESULT(0),
            WM_WINDOWPOSCHANGING
                if PINNED.load(Ordering::Relaxed)
                    && !edit
                    && !TOPMOST.load(Ordering::Relaxed)
                    && !EMBEDDED.load(Ordering::Relaxed) =>
            {
                let pos = &mut *(lp.0 as *mut WINDOWPOS);
                pos.hwndInsertAfter = HWND_BOTTOM;
                pos.flags &= !SWP_NOZORDER;
            }
            WM_GETMINMAXINFO => {
                let mm = &mut *(lp.0 as *mut MINMAXINFO);
                mm.ptMinTrackSize = POINT { x: MIN_W, y: MIN_H };
                return LRESULT(0);
            }
            WM_EXITSIZEMOVE => MOVED.store(true, Ordering::Relaxed),
            WM_MOUSEACTIVATE if !edit => return LRESULT(MA_NOACTIVATE as isize),
            WM_SETTINGCHANGE | WM_DWMCOLORIZATIONCOLORCHANGED => note_settings(msg, lp),
            WM_KEYDOWN if wp.0 == 0x1B && edit => {
                let _ = PostMessageW(Some(hwnd), WM_APP_DONE, WPARAM(0), LPARAM(0));
                return LRESULT(0);
            }
            WM_CLOSE => return LRESULT(0),
            _ => {}
        }
        DefWindowProcW(hwnd, msg, wp, lp)
    }
}

extern "system" fn tray_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_APP_TRAY => {
                let event = (lp.0 & 0xffff) as u32;
                if event == WM_LBUTTONUP {
                    let _ = PostMessageW(Some(hwnd), WM_APP_FLYOUT, WPARAM(0), LPARAM(0));
                } else if event == WM_RBUTTONUP || event == WM_CONTEXTMENU {
                    let _ = PostMessageW(Some(hwnd), WM_APP_MENU, WPARAM(0), LPARAM(0));
                }
                return LRESULT(0);
            }
            WM_APP_MENU => {
                with_app(|a| a.tray_menu());
                return LRESULT(0);
            }
            WM_APP_FLYOUT => {
                with_app(|a| a.toggle_flyout());
                return LRESULT(0);
            }
            WM_APP_SHOW => {
                with_app(|a| a.open_settings(0));
                return LRESULT(0);
            }
            WM_ENDSESSION if wp.0 != 0 => {
                with_app(|a| a.sync_geometry());
                return LRESULT(0);
            }
            WM_APP_READD => {
                with_app(|a| {
                    tray_icon(a.tray, a.icon, NIM_ADD);
                    if a.embedded {
                        a.respawn_all();
                    }
                });
                return LRESULT(0);
            }
            _ => {}
        }
        let created = TASKBAR_CREATED.load(Ordering::Relaxed);
        if created != 0 && msg == created {
            let _ = PostMessageW(Some(hwnd), WM_APP_READD, WPARAM(0), LPARAM(0));
        }
        if msg == WM_SETTINGCHANGE || msg == WM_DWMCOLORIZATIONCOLORCHANGED {
            note_settings(msg, lp);
        }
        DefWindowProcW(hwnd, msg, wp, lp)
    }
}

fn register(class: PCWSTR, proc_: WNDPROC, cursor: PCWSTR) -> Result<()> {
    unsafe {
        let hinst = GetModuleHandleW(None)?;
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: proc_,
            hInstance: hinst.into(),
            hCursor: LoadCursorW(None, cursor)?,
            lpszClassName: class,
            ..Default::default()
        };
        if RegisterClassExW(&wc) == 0 {
            return Err(windows::core::Error::from_win32());
        }
    }
    Ok(())
}

fn append(menu: HMENU, id: usize, text: &str, checked: bool) {
    let h = HSTRING::from(text);
    let flags = MF_STRING | if checked { MF_CHECKED } else { MF_UNCHECKED };
    unsafe {
        let _ = AppendMenuW(menu, flags, id, PCWSTR(h.as_ptr()));
    }
}

fn append_sub(menu: HMENU, sub: HMENU, text: &str) {
    let h = HSTRING::from(text);
    unsafe {
        let _ = AppendMenuW(menu, MF_POPUP, sub.0 as usize, PCWSTR(h.as_ptr()));
    }
}

fn separator(menu: HMENU) {
    unsafe {
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
    }
}

fn track(menu: HMENU, owner: HWND) -> usize {
    unsafe {
        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        // Without this the menu does not close when clicking elsewhere (documented TrackPopupMenu quirk).
        let _ = SetForegroundWindow(owner);
        let id = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY, pt.x, pt.y, None, owner, None);
        let _ = PostMessageW(Some(owner), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);
        id.0 as usize
    }
}

fn strings<S: AsRef<str>>(items: impl IntoIterator<Item = S>) -> ModelRc<SharedString> {
    ModelRc::new(VecModel::from(items.into_iter().map(|s| SharedString::from(s.as_ref())).collect::<Vec<_>>()))
}

fn audio_source(name: &str) -> vizz_audio::Source {
    if name == "mic" {
        vizz_audio::Source::Mic
    } else {
        vizz_audio::Source::Loopback
    }
}

fn find_window(title: PCWSTR) -> Option<HWND> {
    unsafe { FindWindowW(None, title) }.ok().filter(|h| !h.is_invalid())
}

struct App {
    cfg: Config,
    path: PathBuf,
    renderer: Renderer,
    engine: Engine,
    whistle: Option<Engine>,
    whistle_err: String,
    hwnds: Vec<HWND>,
    embedded: bool,
    tray: HWND,
    icon: HICON,
    quit: bool,
    settings: Option<SettingsWindow>,
    flyout: Option<Flyout>,
    flyout_since: Instant,
    dirty: Option<Instant>,
    showcase_kind: usize,
    showcase_style: usize,
    preview_on: Option<PreviewSpec>,
    preview_serial: u64,
    beat_seen: u32,
    beat_glow: f32,
    snap: AnalysisSnapshot,
    wsnap: AnalysisSnapshot,
}

impl App {
    fn kind_of(&self, i: usize) -> WidgetKind {
        WidgetKind::from_name(&self.cfg.widgets[i].kind).unwrap_or(WidgetKind::Warp)
    }

    fn style_of(&self, i: usize) -> Style {
        Style::from_name(&self.cfg.widgets[i].style).unwrap_or(Style::Neon)
    }

    fn global_quality(&self) -> Quality {
        Quality::from_name(&self.cfg.quality).unwrap_or(Quality::Medium)
    }

    fn quality_of(&self, i: usize) -> Quality {
        self.cfg.widgets[i].quality.as_deref().and_then(Quality::from_name).unwrap_or(self.global_quality())
    }

    fn push_look(&self, i: usize) {
        self.renderer.send(Cmd::Look(self.hwnds[i].0 as isize, self.kind_of(i), self.style_of(i), self.quality_of(i)));
    }

    fn index(&self, hwnd: HWND) -> Option<usize> {
        self.hwnds.iter().position(|&h| h == hwnd)
    }

    fn z_order(&self) -> HWND {
        if EDIT.load(Ordering::Relaxed) || self.cfg.topmost {
            HWND_TOPMOST
        } else if self.cfg.pinned {
            HWND_BOTTOM
        } else {
            HWND_NOTOPMOST
        }
    }

    fn spawn(&mut self, i: usize, layer: Option<desktop::Layer>) -> Result<()> {
        let wc = &self.cfg.widgets[i];
        let edit = EDIT.load(Ordering::Relaxed);
        unsafe {
            let hinst = GetModuleHandleW(None)?;
            let (ex, style, parent, (x, y)) = match layer {
                Some(l) => (
                    WS_EX_NOREDIRECTIONBITMAP | WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE,
                    WS_CHILD | WS_CLIPSIBLINGS,
                    Some(l.parent()),
                    l.to_client(wc.x, wc.y),
                ),
                None => (ex_style(self.cfg.pinned, edit, self.cfg.topmost), WS_POPUP, None, (wc.x, wc.y)),
            };
            let hwnd = CreateWindowExW(
                ex,
                WIDGET_CLASS,
                w!("Vizz widget"),
                style,
                x,
                y,
                wc.w,
                wc.h,
                parent,
                None,
                Some(hinst.into()),
                None,
            )?;
            // Layered windows stay invisible until their attributes are set once.
            SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA)?;
            match layer {
                Some(l) => l.place(hwnd),
                None => SetWindowPos(hwnd, Some(self.z_order()), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE)?,
            }
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            self.renderer.send(Cmd::Add(WidgetSpec {
                hwnd: hwnd.0 as isize,
                kind: self.kind_of(i),
                style: self.style_of(i),
                quality: self.quality_of(i),
                width: wc.w as u32,
                height: wc.h as u32,
                palette: wc.palette,
                opacity: wc.opacity,
            }));
            self.hwnds.push(hwnd);
        }
        Ok(())
    }

    fn wants_embed(&self) -> bool {
        self.cfg.pinned && !self.cfg.topmost && !EDIT.load(Ordering::Relaxed)
    }

    fn layer(&self) -> Option<desktop::Layer> {
        if self.wants_embed() {
            desktop::find()
        } else {
            None
        }
    }

    fn spawn_all(&mut self) {
        let layer = self.layer();
        self.embedded = layer.is_some();
        EMBEDDED.store(self.embedded, Ordering::Relaxed);
        for i in 0..self.cfg.widgets.len() {
            if let Err(e) = self.spawn(i, layer) {
                eprintln!("widget {i}: {e}");
            }
        }
    }

    fn respawn_all(&mut self) {
        for h in std::mem::take(&mut self.hwnds) {
            self.renderer.send(Cmd::Remove(h.0 as isize));
            unsafe {
                let _ = DestroyWindow(h);
            }
        }
        self.spawn_all();
    }

    fn save(&mut self) {
        self.dirty = None;
        if let Err(e) = self.cfg.save(&self.path) {
            eprintln!("saving config: {e}");
        }
    }

    fn save_later(&mut self) {
        self.dirty.get_or_insert_with(Instant::now);
    }

    fn apply_window_modes(&mut self) {
        let edit = EDIT.load(Ordering::Relaxed);
        PINNED.store(self.cfg.pinned, Ordering::Relaxed);
        TOPMOST.store(self.cfg.topmost, Ordering::Relaxed);
        if self.embedded || (self.wants_embed() && desktop::find().is_some()) {
            self.sync_geometry();
            self.respawn_all();
            return;
        }
        let style = ex_style(self.cfg.pinned, edit, self.cfg.topmost);
        for &h in &self.hwnds {
            unsafe {
                SetWindowLongPtrW(h, GWL_EXSTYLE, style.0 as isize);
                if self.z_order() == HWND_BOTTOM {
                    let _ = SetWindowPos(h, Some(HWND_NOTOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
                }
                let _ = SetWindowPos(
                    h,
                    Some(self.z_order()),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
                );
            }
        }
    }

    fn set_edit(&mut self, on: bool) {
        EDIT.store(on, Ordering::Relaxed);
        self.renderer.send(Cmd::Edit(on));
        self.apply_window_modes();
        if !on {
            self.sync_geometry();
        }
        self.sync_ui();
    }

    fn set_pinned(&mut self, on: bool) {
        self.cfg.pinned = on;
        if on {
            self.cfg.topmost = false;
        }
        self.apply_window_modes();
        self.save();
        self.sync_ui();
    }

    fn set_topmost(&mut self, on: bool) {
        self.cfg.topmost = on;
        if on {
            self.cfg.pinned = false;
        }
        self.apply_window_modes();
        self.save();
        self.sync_ui();
    }

    fn sync_geometry(&mut self) {
        for (i, &h) in self.hwnds.iter().enumerate() {
            let mut r = RECT::default();
            if unsafe { GetWindowRect(h, &mut r) }.is_err() {
                continue;
            }
            let wc = &mut self.cfg.widgets[i];
            let (w, hgt) = (r.right - r.left, r.bottom - r.top);
            if w != wc.w || hgt != wc.h {
                self.renderer.send(Cmd::Resize(h.0 as isize, w as u32, hgt as u32));
            }
            (wc.x, wc.y, wc.w, wc.h) = (r.left, r.top, w, hgt);
        }
        self.save();
    }

    fn add_widget(&mut self, kind: WidgetKind) {
        let n = self.cfg.widgets.len() as i32;
        let style = self.cfg.widgets.last().map_or("neon".to_string(), |w| w.style.clone());
        let (w, h) = kind.default_size();
        self.cfg.widgets.push(WidgetCfg {
            kind: kind.name().into(),
            style,
            quality: None,
            x: 120 + 40 * (n % 8),
            y: 120 + 40 * (n % 8),
            w,
            h,
            palette: None,
            opacity: 1.0,
        });
        let layer = if self.embedded { self.layer() } else { None };
        if let Err(e) = self.spawn(self.cfg.widgets.len() - 1, layer) {
            eprintln!("creating widget: {e}");
            self.cfg.widgets.pop();
            return;
        }
        if !EDIT.load(Ordering::Relaxed) {
            self.set_edit(true);
        }
        self.save();
        self.apply_inputs();
        self.sync_ui();
    }

    fn remove(&mut self, i: usize) {
        if i >= self.hwnds.len() {
            return;
        }
        let h = self.hwnds.remove(i);
        self.cfg.widgets.remove(i);
        self.renderer.send(Cmd::Remove(h.0 as isize));
        unsafe {
            let _ = DestroyWindow(h);
        }
        self.save();
        self.apply_inputs();
        self.sync_ui();
    }

    fn set_look(&mut self, i: usize, kind: WidgetKind, style: Style) {
        if i >= self.hwnds.len() {
            return;
        }
        self.cfg.widgets[i].kind = kind.name().into();
        self.cfg.widgets[i].style = style.name().into();
        self.push_look(i);
        self.save();
        self.apply_inputs();
        self.sync_ui();
    }

    fn set_widget_quality(&mut self, i: usize, q: Option<Quality>) {
        if i >= self.hwnds.len() {
            return;
        }
        self.cfg.widgets[i].quality = q.map(|q| q.name().into());
        self.push_look(i);
        self.save();
        self.sync_ui();
    }

    fn set_global_quality(&mut self, q: Quality) {
        self.cfg.quality = q.name().into();
        for i in 0..self.hwnds.len() {
            self.push_look(i);
        }
        self.save();
        self.sync_ui();
    }

    fn swatch_of(&self, i: usize) -> usize {
        match self.cfg.widgets[i].palette {
            Some(p) => PRESETS.iter().position(|(_, q)| *q == p).map_or(0, |j| Style::ALL.len() + j),
            None => Style::ALL.iter().position(|s| *s == self.style_of(i)).unwrap_or(0),
        }
    }

    fn apply_swatch(&mut self, i: usize, k: usize) {
        if i >= self.hwnds.len() {
            return;
        }
        let n = Style::ALL.len();
        let w = &mut self.cfg.widgets[i];
        if k < n {
            w.style = Style::ALL[k].name().into();
            w.palette = None;
        } else {
            w.palette = Some(PRESETS[(k - n).min(PRESETS.len() - 1)].1);
        }
        self.renderer.send(Cmd::Palette(self.hwnds[i].0 as isize, self.cfg.widgets[i].palette));
        self.push_look(i);
    }

    fn set_swatch(&mut self, i: usize, k: usize) {
        self.apply_swatch(i, k);
        self.save();
        self.sync_ui();
    }

    fn set_swatch_all(&mut self, k: usize) {
        for i in 0..self.hwnds.len() {
            self.apply_swatch(i, k);
        }
        self.save();
        self.sync_ui();
    }

    fn set_opacity(&mut self, i: usize, o: f32) {
        if i >= self.hwnds.len() {
            return;
        }
        self.cfg.widgets[i].opacity = o.clamp(0.05, 1.0);
        self.renderer.send(Cmd::Opacity(self.hwnds[i].0 as isize, self.cfg.widgets[i].opacity));
        self.save_later();
    }

    fn resize(&mut self, i: usize, preset: usize) {
        if i >= self.hwnds.len() || preset >= SIZES.len() {
            return;
        }
        let h = self.hwnds[i];
        let wide = self.kind_of(i).wide();
        let (_, mut w, mut hgt) = SIZES[preset];
        let (mut x, mut y) = (self.cfg.widgets[i].x, self.cfg.widgets[i].y);
        if w == 0 {
            let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
            unsafe {
                let _ = GetMonitorInfoW(MonitorFromWindow(h, MONITOR_DEFAULTTONEAREST), &mut mi);
            }
            let r = mi.rcWork;
            (x, y, w, hgt) = (r.left, r.top, r.right - r.left, r.bottom - r.top);
        } else if wide {
            (w, hgt) = (w * 3 / 2, hgt * 3 / 4);
        }
        unsafe {
            let _ = SetWindowPos(h, None, x, y, w, hgt, SWP_NOZORDER | SWP_NOACTIVATE);
        }
        self.sync_geometry();
    }

    fn set_input(&mut self, i: usize) {
        let name = INPUTS[i.min(INPUTS.len() - 1)];
        if self.cfg.source != name {
            self.cfg.source = name.into();
            self.engine.set_source(audio_source(name));
        }
        self.save();
        self.apply_inputs();
        self.sync_ui();
    }

    fn set_whistle_input(&mut self, i: usize) {
        self.cfg.whistle_source = WHISTLE_INPUTS[i.min(WHISTLE_INPUTS.len() - 1)].into();
        self.save();
        self.apply_inputs();
        self.sync_ui();
    }

    fn apply_timing(&mut self) {
        self.renderer
            .send(Cmd::Timing { av_offset_ms: self.cfg.av_offset_ms, idle_after_secs: self.cfg.idle_after_secs });
        self.save();
    }

    fn apply_inputs(&mut self) {
        let has_sketch = (0..self.cfg.widgets.len()).any(|i| self.kind_of(i) == WidgetKind::Sketch);
        let needed = has_sketch && self.cfg.whistle_source == "mic" && self.cfg.source != "mic";
        match (needed, self.whistle.is_some()) {
            (true, false) => match Engine::start(vizz_audio::Source::Mic) {
                Ok(e) => {
                    self.renderer.send(Cmd::WhistleBus(Some(e.bus.clone())));
                    self.whistle = Some(e);
                    self.whistle_err.clear();
                }
                Err(e) => self.whistle_err = format!("Microphone: {e}"),
            },
            (false, true) => {
                self.renderer.send(Cmd::WhistleBus(None));
                self.whistle = None;
            }
            _ => {}
        }
    }

    fn each_ui(&self, f: impl Fn(&Vizz)) {
        if let Some(s) = &self.settings {
            f(&s.global::<Vizz>());
        }
        if let Some(fl) = &self.flyout {
            f(&fl.global::<Vizz>());
        }
    }

    fn sync_ui(&self) {
        let rows: Vec<WidgetRow> = (0..self.cfg.widgets.len())
            .map(|i| WidgetRow {
                kind: WidgetKind::ALL.iter().position(|k| *k == self.kind_of(i)).unwrap_or(0) as i32,
                style: self.swatch_of(i) as i32,
                quality: self.cfg.widgets[i]
                    .quality
                    .as_deref()
                    .and_then(Quality::from_name)
                    .and_then(|q| Quality::ALL.iter().position(|x| *x == q))
                    .map_or(0, |p| p as i32 + 1),
                opacity: self.cfg.widgets[i].opacity,
            })
            .collect();
        let quality = Quality::ALL.iter().position(|q| *q == self.global_quality()).unwrap_or(1) as i32;
        let style_all = if self.hwnds.is_empty() { 0 } else { self.swatch_of(0) as i32 };
        let input = INPUTS.iter().position(|s| *s == self.cfg.source).unwrap_or(0) as i32;
        let whistle_input = WHISTLE_INPUTS.iter().position(|s| *s == self.cfg.whistle_source).unwrap_or(0) as i32;
        let theme = Theme::current();
        let color = |c: [f32; 3]| Color::from_rgb_f32(c[0], c[1], c[2]);
        let swatch = |name: &str, p: [[f32; 3]; 3]| PaletteSwatch {
            name: name.into(),
            a: color(p[0]),
            b: color(p[1]),
            c: color(p[2]),
        };
        let all: Vec<PaletteSwatch> = Style::ALL
            .iter()
            .map(|s| {
                let p = theme.palette_for(*s);
                swatch(s.label(), [p.c0, p.c1, p.c2])
            })
            .chain(PRESETS.iter().map(|(n, p)| swatch(n, *p)))
            .collect();
        let palette_rows: Vec<ModelRc<PaletteSwatch>> =
            all.chunks(SWATCHES_PER_ROW).map(|c| ModelRc::new(VecModel::from(c.to_vec()))).collect();
        let autostart = crate::autostart::enabled();
        let (sk, ss) = (self.showcase_kind as i32, self.showcase_style as i32);
        self.each_ui(|g| {
            g.set_palette_rows(ModelRc::new(VecModel::from(palette_rows.clone())));
            g.set_autostart(autostart);
            g.set_showcase_kind(sk);
            g.set_showcase_style(ss);
            g.set_widgets(ModelRc::new(VecModel::from(rows.clone())));
            g.set_quality(quality);
            g.set_style_all(style_all);
            g.set_input(input);
            g.set_whistle_input(whistle_input);
            g.set_editing(EDIT.load(Ordering::Relaxed));
            g.set_pinned(self.cfg.pinned);
            g.set_topmost(self.cfg.topmost);
            g.set_fps60(self.cfg.fps > 30);
            g.set_av_offset(self.cfg.av_offset_ms);
            g.set_idle_secs(self.cfg.idle_after_secs);
        });
    }

    fn sync_live(&mut self) {
        if self.settings.is_none() && self.flyout.is_none() {
            return;
        }
        self.engine.bus.read(&mut self.snap);
        let mut whistle = self.snap.whistle;
        if let Some(w) = &self.whistle {
            w.bus.read(&mut self.wsnap);
            whistle = self.wsnap.whistle;
        }
        if self.snap.beat_count != self.beat_seen {
            self.beat_seen = self.snap.beat_count;
            self.beat_glow = 1.0;
        } else {
            self.beat_glow *= 0.82;
        }
        let mut status = self.engine.status();
        if !self.whistle_err.is_empty() {
            status = self.whistle_err.clone();
        }
        let s = &self.snap;
        let locked = s.confidence > 0.15 && s.silent_secs < 1.0;
        let (level, bpm, conf, glow) = (s.level, if locked { s.bpm } else { 0.0 }, s.confidence, self.beat_glow);
        self.each_ui(|g| {
            g.set_level(level.clamp(0.0, 1.0));
            g.set_bpm(bpm);
            g.set_confidence(conf);
            g.set_beat(glow);
            g.set_whistle(whistle.clamp(0.0, 1.0));
            g.set_status(SharedString::from(status.as_str()));
        });
    }

    fn sync_preview(&mut self) {
        let want =
            self.settings.as_ref().filter(|s| s.window().is_visible() && s.get_page() == 0).map(|_| PreviewSpec {
                kind: WidgetKind::ALL[self.showcase_kind.min(WidgetKind::ALL.len() - 1)],
                style: Style::ALL.get(self.showcase_style).copied().unwrap_or(Style::Neon),
                quality: Quality::High,
                width: PREVIEW_SIZE.0,
                height: PREVIEW_SIZE.1,
                palette: self.showcase_style.checked_sub(Style::ALL.len()).and_then(|j| PRESETS.get(j)).map(|p| p.1),
            });
        if want != self.preview_on {
            self.preview_on = want;
            self.renderer.send(Cmd::Preview(want));
        }
        let Some(win) = &self.settings else { return };
        if want.is_none() {
            return;
        }
        let Ok(frame) = self.renderer.preview.lock() else { return };
        if frame.serial == self.preview_serial || frame.rgb.len() != (frame.width * frame.height * 3) as usize {
            return;
        }
        self.preview_serial = frame.serial;
        let buf = SharedPixelBuffer::<Rgb8Pixel>::clone_from_slice(&frame.rgb, frame.width, frame.height);
        win.global::<Vizz>().set_preview(Image::from_rgb8(buf));
    }

    fn add_showcase(&mut self) {
        let kind = WidgetKind::ALL[self.showcase_kind.min(WidgetKind::ALL.len() - 1)];
        self.add_widget(kind);
        let i = self.cfg.widgets.len().saturating_sub(1);
        if i < self.hwnds.len() {
            self.set_swatch(i, self.showcase_style);
        }
    }

    fn open_settings(&mut self, page: i32) {
        if self.settings.is_none() {
            let Ok(win) = SettingsWindow::new() else { return };
            wire(&win.global::<Vizz>());
            self.settings = Some(win);
        }
        self.sync_ui();
        self.sync_live();
        if let Some(win) = &self.settings {
            if page >= 0 {
                win.set_page(page);
            }
            let _ = win.show();
            if let Some(h) = find_window(w!("Vizz")) {
                unsafe {
                    let _ = ShowWindow(h, SW_RESTORE);
                    let _ = SetForegroundWindow(h);
                }
            }
        }
    }

    fn hide_flyout(&mut self) {
        if let Some(f) = self.flyout.take() {
            let _ = f.hide();
        }
    }

    fn toggle_flyout(&mut self) {
        if self.flyout.is_some() {
            self.hide_flyout();
            return;
        }
        let Ok(fl) = Flyout::new() else { return };
        wire(&fl.global::<Vizz>());
        self.flyout = Some(fl);
        self.sync_ui();
        self.sync_live();
        let Some(fl) = &self.flyout else { return };
        let _ = fl.show();
        let size = fl.window().size();
        let mut pt = POINT::default();
        let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        unsafe {
            let _ = GetCursorPos(&mut pt);
            let _ = GetMonitorInfoW(MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST), &mut mi);
        }
        let work = mi.rcWork;
        let margin = 12;
        let x = (pt.x - size.width as i32 / 2).clamp(work.left + margin, work.right - size.width as i32 - margin);
        let y = if pt.y > (work.top + work.bottom) / 2 {
            work.bottom - size.height as i32 - margin
        } else {
            work.top + margin
        };
        fl.window().set_position(slint::PhysicalPosition::new(x, y));
        if let Some(h) = find_window(FLYOUT_TITLE) {
            unsafe {
                let ex = GetWindowLongPtrW(h, GWL_EXSTYLE);
                SetWindowLongPtrW(h, GWL_EXSTYLE, (ex | WS_EX_TOOLWINDOW.0 as isize) & !(WS_EX_APPWINDOW.0 as isize));
                let pref = DWMWCP_ROUND;
                let _ = DwmSetWindowAttribute(
                    h,
                    DWMWA_WINDOW_CORNER_PREFERENCE,
                    &pref as *const _ as *const _,
                    std::mem::size_of_val(&pref) as u32,
                );
                let _ = SetForegroundWindow(h);
            }
        }
        self.flyout_since = Instant::now();
    }

    fn tick(&mut self, start: Instant, exit_after: Option<f64>) {
        if MOVED.swap(false, Ordering::Relaxed) {
            self.sync_geometry();
        }
        if THEME_CHANGED.swap(false, Ordering::Relaxed) {
            self.renderer.send(Cmd::Theme(Theme::current()));
        }
        if self.dirty.is_some_and(|t| t.elapsed() > SAVE_DELAY) {
            self.save();
        }
        if self.settings.as_ref().is_some_and(|s| !s.window().is_visible()) {
            self.settings = None;
        }
        if self.flyout.is_some() && self.flyout_since.elapsed() > Duration::from_millis(400) {
            let fg = unsafe { GetForegroundWindow() };
            let mine = find_window(FLYOUT_TITLE);
            let popup_open = mine.is_some_and(|m| unsafe { GetWindow(fg, GW_OWNER) }.ok() == Some(m));
            if mine != Some(fg) && !popup_open {
                self.hide_flyout();
            }
        }
        self.sync_preview();
        self.sync_live();
        if exit_after.is_some_and(|s| start.elapsed().as_secs_f64() >= s) {
            self.quit = true;
        }
        if self.quit {
            let _ = slint::quit_event_loop();
        }
    }

    fn tray_menu(&mut self) {
        self.hide_flyout();
        unsafe {
            let Ok(menu) = CreatePopupMenu() else { return };
            append(menu, ID_SETTINGS, "Settings…", false);
            separator(menu);
            let Ok(add) = CreatePopupMenu() else { return };
            for (k, kind) in WidgetKind::ALL.iter().enumerate() {
                append(add, ID_ADD + k, kind.label(), false);
            }
            append_sub(menu, add, "Add widget");
            let Ok(styles) = CreatePopupMenu() else { return };
            for (k, name) in swatch_names().enumerate() {
                append(styles, ID_STYLE_ALL + k, name, false);
            }
            append_sub(menu, styles, "Colors for all widgets");
            let Ok(quality) = CreatePopupMenu() else { return };
            for (k, q) in Quality::ALL.iter().enumerate() {
                append(quality, ID_QUALITY_ALL + k, q.label(), *q == self.global_quality());
            }
            append_sub(menu, quality, "Quality (all widgets)");
            append(menu, ID_NEXT_PRESET, "Next Flux preset", false);
            separator(menu);
            append(menu, ID_EDIT, "Edit layout (drag, resize, right-click)", EDIT.load(Ordering::Relaxed));
            append(menu, ID_PIN, "Pin to desktop (behind windows)", self.cfg.pinned);
            append(menu, ID_TOP, "Always on top", self.cfg.topmost);
            separator(menu);
            append(menu, ID_QUIT, "Quit Vizz", false);
            let id = track(menu, self.tray);
            self.command(None, id);
        }
    }

    fn widget_menu(&mut self, hwnd: HWND) {
        let Some(i) = self.index(hwnd) else { return };
        let kind = self.kind_of(i);
        unsafe {
            let Ok(menu) = CreatePopupMenu() else { return };
            let Ok(kinds) = CreatePopupMenu() else { return };
            for (k, kd) in WidgetKind::ALL.iter().enumerate() {
                append(kinds, ID_KIND + k, kd.label(), *kd == kind);
            }
            append_sub(menu, kinds, "Design");
            let Ok(styles) = CreatePopupMenu() else { return };
            let current = self.swatch_of(i);
            for (k, name) in swatch_names().enumerate() {
                append(styles, ID_STYLE + k, name, k == current);
            }
            append_sub(menu, styles, "Colors");
            let Ok(sizes) = CreatePopupMenu() else { return };
            for (k, (name, _, _)) in SIZES.iter().enumerate() {
                append(sizes, ID_SIZE + k, name, false);
            }
            append_sub(menu, sizes, "Size");
            let Ok(quality) = CreatePopupMenu() else { return };
            let own = self.cfg.widgets[i].quality.as_deref().and_then(Quality::from_name);
            append(quality, ID_QUALITY, "Same as all widgets", own.is_none());
            for (k, q) in Quality::ALL.iter().enumerate() {
                append(quality, ID_QUALITY + 1 + k, q.label(), own == Some(*q));
            }
            append_sub(menu, quality, "Quality");
            separator(menu);
            append(menu, ID_SETTINGS, "Settings…", false);
            append(menu, ID_REMOVE, "Remove widget", false);
            append(menu, ID_DONE, "Done editing", false);
            let id = track(menu, hwnd);
            self.command(Some(i), id);
        }
    }

    fn command(&mut self, target: Option<usize>, id: usize) {
        let n_kinds = WidgetKind::ALL.len();
        let n_styles = Style::ALL.len() + PRESETS.len();
        match id {
            0 => {}
            ID_SETTINGS => self.open_settings(0),
            _ if (ID_ADD..ID_ADD + n_kinds).contains(&id) => self.add_widget(WidgetKind::ALL[id - ID_ADD]),
            _ if (ID_STYLE_ALL..ID_STYLE_ALL + n_styles).contains(&id) => self.set_swatch_all(id - ID_STYLE_ALL),
            _ if (ID_QUALITY_ALL..ID_QUALITY_ALL + Quality::ALL.len()).contains(&id) => {
                self.set_global_quality(Quality::ALL[id - ID_QUALITY_ALL])
            }
            ID_NEXT_PRESET => self.renderer.send(Cmd::NextPreset),
            ID_EDIT => self.set_edit(!EDIT.load(Ordering::Relaxed)),
            ID_PIN => self.set_pinned(!self.cfg.pinned),
            ID_TOP => self.set_topmost(!self.cfg.topmost),
            ID_QUIT => self.quit = true,
            ID_DONE => self.set_edit(false),
            _ => {
                let Some(i) = target else { return };
                if (ID_KIND..ID_KIND + n_kinds).contains(&id) {
                    self.set_look(i, WidgetKind::ALL[id - ID_KIND], self.style_of(i));
                } else if (ID_STYLE..ID_STYLE + n_styles).contains(&id) {
                    self.set_swatch(i, id - ID_STYLE);
                } else if (ID_SIZE..ID_SIZE + SIZES.len()).contains(&id) {
                    self.resize(i, id - ID_SIZE);
                } else if (ID_QUALITY..=ID_QUALITY + Quality::ALL.len()).contains(&id) {
                    self.set_widget_quality(i, (id > ID_QUALITY).then(|| Quality::ALL[id - ID_QUALITY - 1]));
                } else if id == ID_REMOVE {
                    self.remove(i);
                }
            }
        }
    }
}

fn swatch_names() -> impl Iterator<Item = &'static str> {
    Style::ALL.iter().map(|s| s.label()).chain(PRESETS.iter().map(|p| p.0))
}

fn wire(g: &Vizz) {
    g.set_kinds(strings(WidgetKind::ALL.iter().map(|k| k.label())));
    g.set_short_kinds(strings(WidgetKind::ALL.iter().map(|k| k.label().split(" (").next().unwrap_or(k.label()))));
    g.set_styles(strings(Style::ALL.iter().map(|s| s.label())));
    g.set_qualities(strings(Quality::ALL.iter().map(|q| q.label())));
    g.set_widget_qualities(strings(
        std::iter::once("Same as all widgets").chain(Quality::ALL.iter().map(|q| q.label())),
    ));
    g.set_version(SharedString::from(env!("CARGO_PKG_VERSION")));
    let idx = |v: i32| v.max(0) as usize;
    g.on_add_widget(move |k| with_app(|a| a.add_widget(WidgetKind::ALL[idx(k).min(WidgetKind::ALL.len() - 1)])));
    g.on_remove_widget(move |i| with_app(|a| a.remove(idx(i))));
    g.on_set_kind(move |i, k| {
        with_app(|a| {
            let i = idx(i);
            if i < a.hwnds.len() {
                a.set_look(i, WidgetKind::ALL[idx(k).min(WidgetKind::ALL.len() - 1)], a.style_of(i));
            }
        })
    });
    g.on_set_style(move |i, s| with_app(|a| a.set_swatch(idx(i), idx(s))));
    g.on_set_widget_quality(move |i, q| {
        with_app(|a| {
            a.set_widget_quality(idx(i), (q > 0).then(|| Quality::ALL[(idx(q) - 1).min(Quality::ALL.len() - 1)]))
        })
    });
    g.on_set_opacity(move |i, v| with_app(|a| a.set_opacity(idx(i), v)));
    g.on_set_size(move |i, s| with_app(|a| a.resize(idx(i), idx(s))));
    g.on_set_quality(move |q| with_app(|a| a.set_global_quality(Quality::ALL[idx(q).min(Quality::ALL.len() - 1)])));
    g.on_set_style_all(move |s| with_app(|a| a.set_swatch_all(idx(s))));
    g.on_set_input(move |i| with_app(|a| a.set_input(idx(i))));
    g.on_set_whistle_input(move |i| with_app(|a| a.set_whistle_input(idx(i))));
    g.on_set_editing(move |on| with_app(|a| a.set_edit(on)));
    g.on_set_pinned(move |on| with_app(|a| a.set_pinned(on)));
    g.on_set_topmost(move |on| with_app(|a| a.set_topmost(on)));
    g.on_set_fps60(move |on| {
        with_app(|a| {
            a.cfg.fps = if on { 60 } else { 30 };
            a.save();
        })
    });
    g.on_set_av_offset(move |v| {
        with_app(|a| {
            a.cfg.av_offset_ms = v.round();
            a.apply_timing();
        })
    });
    g.on_set_idle(move |v| {
        with_app(|a| {
            a.cfg.idle_after_secs = v.round().max(1.0);
            a.apply_timing();
        })
    });
    g.on_next_preset(move || with_app(|a| a.renderer.send(Cmd::NextPreset)));
    g.on_open_settings(move |page| {
        with_app(|a| {
            a.hide_flyout();
            a.open_settings(page);
        })
    });
    g.on_quit(move || with_app(|a| a.quit = true));
    g.on_set_showcase(move |k| {
        with_app(|a| {
            a.showcase_kind = idx(k).min(WidgetKind::ALL.len() - 1);
            a.sync_ui();
        })
    });
    g.on_set_showcase_style(move |s| {
        with_app(|a| {
            a.showcase_style = idx(s).min(Style::ALL.len() + PRESETS.len() - 1);
            a.sync_ui();
        })
    });
    g.on_add_showcase(move || with_app(|a| a.add_showcase()));
    g.on_set_autostart(move |on| {
        with_app(|a| {
            if let Err(e) = crate::autostart::set(on) {
                a.whistle_err = e;
            }
            a.sync_ui();
        })
    });
    g.on_open_link(|url| {
        let url = HSTRING::from(url.as_str());
        unsafe {
            ShellExecuteW(None, w!("open"), PCWSTR(url.as_ptr()), None, None, SW_SHOWNORMAL);
        }
    });
}

fn app_icon() -> HICON {
    unsafe {
        let size = GetSystemMetrics(SM_CXSMICON).max(16);
        if let Ok(hinst) = GetModuleHandleW(None) {
            // Resource id 1 is the icon embedded from res/vizz.rc.
            if let Ok(h) = LoadImageW(
                Some(hinst.into()),
                PCWSTR(std::ptr::without_provenance(1)),
                IMAGE_ICON,
                size,
                size,
                LR_DEFAULTCOLOR,
            ) {
                return HICON(h.0);
            }
        }
    }
    crate::icon::create(32).or_else(|_| unsafe { LoadIconW(None, IDI_APPLICATION) }).unwrap_or_default()
}

fn tray_icon(hwnd: HWND, icon: HICON, op: windows::Win32::UI::Shell::NOTIFY_ICON_MESSAGE) {
    unsafe {
        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: 1,
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
            uCallbackMessage: WM_APP_TRAY,
            hIcon: icon,
            ..Default::default()
        };
        for (d, s) in nid.szTip.iter_mut().zip("Vizz".encode_utf16()) {
            *d = s;
        }
        let _ = Shell_NotifyIconW(op, &nid);
    }
}

pub fn run(engine: Engine, cfg: Config, opts: &GuiOpts) -> std::result::Result<(), String> {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        // The mutex handle is intentionally never closed; it lives as long as the process.
        CreateMutexW(None, true, w!("Local\\Vizz.SingleInstance")).map_err(|e| e.to_string())?;
        if GetLastError() == ERROR_ALREADY_EXISTS {
            if let Ok(h) = FindWindowW(TRAY_CLASS, None) {
                let _ = PostMessageW(Some(h), WM_APP_SHOW, WPARAM(0), LPARAM(0));
                return Ok(());
            }
            return Err("Vizz is already running (look for its icon in the tray)".into());
        }
        TASKBAR_CREATED.store(RegisterWindowMessageW(w!("TaskbarCreated")), Ordering::Relaxed);
    }
    EDIT.store(opts.edit, Ordering::Relaxed);
    PINNED.store(cfg.pinned, Ordering::Relaxed);
    TOPMOST.store(cfg.topmost, Ordering::Relaxed);
    register(WIDGET_CLASS, Some(widget_proc), IDC_SIZEALL).map_err(|e| e.to_string())?;
    register(TRAY_CLASS, Some(tray_proc), IDC_ARROW).map_err(|e| e.to_string())?;
    let render_opts = RenderOpts {
        fps: if cfg.quality == "low" { 30 } else { cfg.fps },
        av_offset_ms: cfg.av_offset_ms,
        idle_after_secs: cfg.idle_after_secs,
        warp: opts.warp,
    };
    let renderer = Renderer::start(engine.bus.clone(), Theme::current(), render_opts)?;
    renderer.send(Cmd::Edit(opts.edit));
    let tray = unsafe {
        let hinst = GetModuleHandleW(None).map_err(|e| e.to_string())?;
        CreateWindowExW(
            WS_EX_TOOLWINDOW,
            TRAY_CLASS,
            w!("Vizz tray"),
            WS_POPUP,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(hinst.into()),
            None,
        )
        .map_err(|e| e.to_string())?
    };
    let icon = app_icon();
    tray_icon(tray, icon, NIM_ADD);

    let app = Rc::new(RefCell::new(App {
        cfg,
        path: opts.config_path.clone(),
        renderer,
        engine,
        whistle: None,
        whistle_err: String::new(),
        hwnds: Vec::new(),
        embedded: false,
        tray,
        icon,
        quit: false,
        settings: None,
        flyout: None,
        flyout_since: Instant::now(),
        dirty: None,
        showcase_kind: 1,
        showcase_style: 0,
        preview_on: None,
        preview_serial: 0,
        beat_seen: 0,
        beat_glow: 0.0,
        snap: AnalysisSnapshot::EMPTY,
        wsnap: AnalysisSnapshot::EMPTY,
    }));
    APP.with(|a| *a.borrow_mut() = Some(app.clone()));
    {
        let mut a = app.borrow_mut();
        a.spawn_all();
        a.apply_inputs();
        crate::autostart::refresh();
        if !opts.background {
            a.open_settings(0);
        }
    }
    eprintln!("Vizz is running. Click its tray icon for quick settings; right-click for the menu.");

    let start = Instant::now();
    let exit_after = opts.exit_after;
    let timer = Timer::default();
    timer.start(TimerMode::Repeated, TICK, move || with_app(|a| a.tick(start, exit_after)));
    let result = slint::run_event_loop_until_quit().map_err(|e| e.to_string());
    timer.stop();

    APP.with(|a| *a.borrow_mut() = None);
    let Ok(cell) = Rc::try_unwrap(app) else { return Err("app still borrowed at exit".into()) };
    let mut app = cell.into_inner();
    app.sync_geometry();
    app.hide_flyout();
    if let Some(s) = app.settings.take() {
        let _ = s.hide();
    }
    tray_icon(app.tray, app.icon, NIM_DELETE);
    let App { renderer, hwnds, tray, whistle, engine, .. } = app;
    drop(renderer);
    drop(whistle);
    drop(engine);
    unsafe {
        for h in hwnds {
            let _ = DestroyWindow(h);
        }
        let _ = DestroyWindow(tray);
    }
    result
}
