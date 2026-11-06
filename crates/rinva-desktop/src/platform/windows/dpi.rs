//! DPI entry points added after Windows 10 RTM must not be loader dependencies.
use std::{ptr::null_mut, sync::OnceLock};
use windows_sys::Win32::{
    Foundation::HWND,
    Graphics::Gdi::{
        GetDC, GetDeviceCaps, MonitorFromWindow, ReleaseDC, LOGPIXELSX, MONITOR_DEFAULTTONEAREST,
    },
    System::LibraryLoader::{GetModuleHandleW, GetProcAddress},
    UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
};

type GetWindowDpi = unsafe extern "system" fn(HWND) -> u32;
static WINDOW_DPI: OnceLock<Option<GetWindowDpi>> = OnceLock::new();

pub fn initialize(force_fallback: bool) {
    WINDOW_DPI.get_or_init(|| {
        if force_fallback {
            return None;
        }
        // user32 is already loaded by the application's baseline Win32 imports.
        unsafe {
            let module = GetModuleHandleW(super::wide("user32.dll").as_ptr());
            GetProcAddress(module, c"GetDpiForWindow".as_ptr().cast()).map(|address| {
                std::mem::transmute::<unsafe extern "system" fn() -> isize, GetWindowDpi>(address)
            })
        }
    });
}

pub fn uses_fallback() -> bool {
    WINDOW_DPI.get().is_none_or(Option::is_none)
}

pub unsafe fn for_window(hwnd: HWND) -> u32 {
    if let Some(get) = WINDOW_DPI.get().copied().flatten() {
        let dpi = get(hwnd);
        if dpi != 0 {
            return dpi;
        }
    }
    // Available since Windows 8.1; preserves per-monitor scaling on early Windows 10.
    let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
    let (mut x, mut y) = (0, 0);
    if GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut x, &mut y) >= 0 && x != 0 {
        return x;
    }
    let dc = GetDC(null_mut());
    if dc.is_null() {
        return 96;
    }
    let dpi = GetDeviceCaps(dc, LOGPIXELSX as i32);
    ReleaseDC(null_mut(), dc);
    if dpi > 0 {
        dpi as u32
    } else {
        96
    }
}
