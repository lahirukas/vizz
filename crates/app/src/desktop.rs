use windows::core::{w, BOOL};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::ScreenToClient;
use windows::Win32::UI::WindowsAndMessaging::*;

const SPAWN_WORKERW: u32 = 0x052C;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Layer {
    // Windows 11 24H2+: icons and wallpaper are both children of Progman.
    Raised { progman: HWND, icons: HWND },
    // Earlier builds: a top-level WorkerW behind the one hosting the icons.
    Classic(HWND),
}

impl Layer {
    pub fn parent(self) -> HWND {
        match self {
            Layer::Raised { progman, .. } => progman,
            Layer::Classic(w) => w,
        }
    }

    pub fn to_client(self, x: i32, y: i32) -> (i32, i32) {
        let mut pt = POINT { x, y };
        unsafe {
            let _ = ScreenToClient(self.parent(), &mut pt);
        }
        (pt.x, pt.y)
    }

    pub fn place(self, hwnd: HWND) {
        if let Layer::Raised { icons, .. } = self {
            unsafe {
                let _ = SetWindowPos(hwnd, Some(icons), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            }
        }
    }
}

fn valid(h: windows::core::Result<HWND>) -> Option<HWND> {
    h.ok().filter(|h| !h.is_invalid())
}

extern "system" fn find_worker(top: HWND, out: LPARAM) -> BOOL {
    unsafe {
        if valid(FindWindowExW(Some(top), None, w!("SHELLDLL_DefView"), None)).is_some() {
            if let Some(worker) = valid(FindWindowExW(None, Some(top), w!("WorkerW"), None)) {
                *(out.0 as *mut HWND) = worker;
                return false.into();
            }
        }
    }
    true.into()
}

pub fn find() -> Option<Layer> {
    unsafe {
        let progman = valid(FindWindowW(w!("Progman"), None))?;
        let mut res = 0usize;
        let _ = SendMessageTimeoutW(progman, SPAWN_WORKERW, WPARAM(0xD), LPARAM(1), SMTO_NORMAL, 1000, Some(&mut res));
        if let Some(icons) = valid(FindWindowExW(Some(progman), None, w!("SHELLDLL_DefView"), None)) {
            return Some(Layer::Raised { progman, icons });
        }
        let mut worker = HWND::default();
        let _ = EnumWindows(Some(find_worker), LPARAM(&mut worker as *mut HWND as isize));
        (!worker.is_invalid()).then_some(Layer::Classic(worker))
    }
}
