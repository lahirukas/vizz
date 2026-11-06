use windows::core::{w, PCWSTR};
use windows::Win32::System::Registry::*;

const RUN_KEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const VALUE: PCWSTR = w!("Vizz");

fn command() -> Result<Vec<u16>, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let cmd = format!("\"{}\" --background", exe.display());
    Ok(cmd.encode_utf16().chain(Some(0)).collect())
}

pub fn enabled() -> bool {
    let mut size = 0u32;
    unsafe { RegGetValueW(HKEY_CURRENT_USER, RUN_KEY, VALUE, RRF_RT_REG_SZ, None, None, Some(&mut size)) }.is_ok()
}

pub fn set(on: bool) -> Result<(), String> {
    unsafe {
        if !on {
            let r = RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN_KEY, VALUE);
            return if r.is_ok() || r == windows::Win32::Foundation::ERROR_FILE_NOT_FOUND {
                Ok(())
            } else {
                Err(format!("removing autostart: {r:?}"))
            };
        }
        let cmd = command()?;
        let bytes = std::slice::from_raw_parts(cmd.as_ptr() as *const u8, cmd.len() * 2);
        let mut key = HKEY::default();
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            RUN_KEY,
            None,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            None,
            &mut key,
            None,
        )
        .ok()
        .map_err(|e| e.to_string())?;
        let r = RegSetValueExW(key, VALUE, None, REG_SZ, Some(bytes)).ok().map_err(|e| e.to_string());
        let _ = RegCloseKey(key);
        r
    }
}

// Keeps the Run entry pointing at this exe if the portable folder was moved.
pub fn refresh() {
    if enabled() {
        let _ = set(true);
    }
}
