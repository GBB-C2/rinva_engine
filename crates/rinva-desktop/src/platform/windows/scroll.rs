//! The scrollbar remains a Win32 SCROLLBAR for keyboard/accessibility semantics.
//! Its painting and pointer tracking share geometry and the native 32-bit range.
use super::ui::{self, Palette};
use std::ptr::{null, null_mut};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    UI::{Controls::*, Input::KeyboardAndMouse::*, Shell::*, WindowsAndMessaging::*},
};

const BAR_SUBCLASS: usize = 11;
pub const SYNC_MESSAGE: u32 = WM_APP + 10;
#[derive(Clone, Copy)]
enum Press {
    Drag { offset: i32, original: i32 },
    Step(i32),
}
struct State {
    target: HWND,
    list: bool,
    palette: Palette,
    scale: f32,
    press: Option<Press>,
    wheel: i32,
}
#[derive(Clone, Copy)]
pub struct Bar {
    pub hwnd: HWND,
    pub target: HWND,
}

pub unsafe fn attach(
    parent: HWND,
    target: HWND,
    list: bool,
    id: u16,
    palette: Palette,
    scale: f32,
) -> Result<Bar, String> {
    let hwnd = CreateWindowExW(
        0,
        ui::wide("SCROLLBAR").as_ptr(),
        ui::wide("Vertical scroll").as_ptr(),
        WS_CHILD | WS_TABSTOP | SBS_VERT as u32,
        0,
        0,
        14,
        100,
        parent,
        id as usize as HMENU,
        null_mut(),
        null(),
    );
    if hwnd.is_null() {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let data = Box::into_raw(Box::new(State {
        target,
        list,
        palette,
        scale,
        press: None,
        wheel: 0,
    }));
    if SetWindowSubclass(hwnd, Some(bar_proc), BAR_SUBCLASS, data as usize) == 0 {
        drop(Box::from_raw(data));
        DestroyWindow(hwnd);
        return Err("Could not create scrollbar".into());
    }
    Ok(Bar { hwnd, target })
}
pub unsafe fn style(bar: Bar, palette: Palette, scale: f32) {
    let mut data = 0;
    if GetWindowSubclass(bar.hwnd, Some(bar_proc), BAR_SUBCLASS, &mut data) != 0 {
        let s = &mut *(data as *mut State);
        s.palette = palette;
        s.scale = scale;
    }
    InvalidateRect(bar.hwnd, null(), 0);
}
unsafe fn range(hwnd: HWND) -> SCROLLINFO {
    let mut info = SCROLLINFO {
        cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
        fMask: SIF_ALL,
        ..std::mem::zeroed()
    };
    GetScrollInfo(hwnd, SB_CTL, &mut info);
    info
}
unsafe fn source(state: &State) -> (i32, i32, i32) {
    let mut rect: RECT = std::mem::zeroed();
    GetClientRect(state.target, &mut rect);
    if state.list {
        let row = SendMessageW(state.target, LB_GETITEMHEIGHT, 0, 0).max(1) as i32;
        (
            SendMessageW(state.target, LB_GETCOUNT, 0, 0).max(0) as i32,
            (rect.bottom / row).max(1),
            SendMessageW(state.target, LB_GETTOPINDEX, 0, 0).max(0) as i32,
        )
    } else {
        SendMessageW(state.target, EM_GETRECT, 0, &mut rect as *mut _ as isize);
        let dc = GetDC(state.target);
        let font = SendMessageW(state.target, WM_GETFONT, 0, 0) as HFONT;
        let old = SelectObject(dc, font);
        let mut metrics: TEXTMETRICW = std::mem::zeroed();
        GetTextMetricsW(dc, &mut metrics);
        SelectObject(dc, old);
        ReleaseDC(state.target, dc);
        (
            SendMessageW(state.target, EM_GETLINECOUNT, 0, 0) as i32,
            ((rect.bottom - rect.top) / metrics.tmHeight.max(1)).max(1),
            SendMessageW(state.target, EM_GETFIRSTVISIBLELINE, 0, 0) as i32,
        )
    }
}
pub unsafe fn sync(bar: Bar) {
    let mut data = 0;
    if GetWindowSubclass(bar.hwnd, Some(bar_proc), BAR_SUBCLASS, &mut data) == 0 {
        return;
    }
    let s = &*(data as *const State);
    let (count, page, pos) = source(s);
    let old = range(bar.hwnd);
    let info = SCROLLINFO {
        cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
        fMask: SIF_RANGE | SIF_PAGE | SIF_POS,
        nMin: 0,
        nMax: (count - 1).max(0),
        nPage: page as u32,
        nPos: pos,
        nTrackPos: 0,
    };
    if old.nMax != info.nMax || old.nPage != info.nPage || old.nPos != info.nPos {
        SetScrollInfo(bar.hwnd, SB_CTL, &info, 0);
        InvalidateRect(bar.hwnd, null(), 0);
    }
    ShowWindow(
        bar.hwnd,
        if count > page && IsWindowVisible(bar.target) != 0 {
            SW_SHOWNA
        } else {
            SW_HIDE
        },
    );
}
unsafe fn position(hwnd: HWND, desired: i32) {
    let mut data = 0;
    if GetWindowSubclass(hwnd, Some(bar_proc), BAR_SUBCLASS, &mut data) == 0 {
        return;
    }
    let s = &*(data as *const State);
    let (count, page, current) = source(s);
    let target = s.target;
    let list = s.list;
    let desired = desired.clamp(0, (count - page).max(0));
    if list {
        SendMessageW(target, LB_SETTOPINDEX, desired as usize, 0);
    } else {
        SendMessageW(target, EM_LINESCROLL, 0, (desired - current) as isize);
    }
    super::smoke_trace(&format!(
        "scroll {} desired={desired} previous={current} actual={} count={count} page={page}",
        GetDlgCtrlID(target),
        source(&*(data as *const State)).2
    ));
    sync(Bar { hwnd, target });
    InvalidateRect(target, null(), 0);
    InvalidateRect(hwnd, null(), 0);
}
pub unsafe fn request(bar: Bar, code: i32, value: i32) {
    let info = range(bar.hwnd);
    let n = match code {
        SB_LINEUP => info.nPos - 1,
        SB_LINEDOWN => info.nPos + 1,
        SB_PAGEUP => info.nPos - info.nPage as i32,
        SB_PAGEDOWN => info.nPos + info.nPage as i32,
        SB_TOP => 0,
        SB_BOTTOM => info.nMax,
        SB_THUMBTRACK | SB_THUMBPOSITION => value,
        _ => return,
    };
    position(bar.hwnd, n);
}
pub(super) unsafe fn geometry(hwnd: HWND) -> (RECT, i32, i32, i32) {
    let mut rect: RECT = std::mem::zeroed();
    GetClientRect(hwnd, &mut rect);
    let info = range(hwnd);
    let arrow = rect.right.min(rect.bottom / 2).max(1);
    let track = (rect.bottom - 2 * arrow).max(0);
    let extent = (info.nMax - info.nMin + 1).max(1);
    let thumb = ((track as f64 * info.nPage as f64 / extent as f64).round() as i32)
        .max(rect.right * 2)
        .min(track);
    let maximum = (extent - info.nPage as i32).max(1);
    let top = arrow
        + ((track - thumb) as f64 * (info.nPos - info.nMin) as f64 / maximum as f64).round() as i32;
    (rect, arrow, top, top + thumb)
}
unsafe fn paint(hwnd: HWND, dc: HDC, s: &State) {
    let (r, arrow, top, bottom) = geometry(hwnd);
    ui::fill(dc, r, s.palette.surface);
    let mid = r.right / 2;
    let width = (6. * s.scale).round().max(3.) as i32;
    ui::rounded(
        dc,
        RECT {
            left: mid - 1,
            top: arrow,
            right: mid + 1,
            bottom: r.bottom - arrow,
        },
        s.palette.field,
        s.palette.field,
        2,
    );
    let mut pointer: POINT = std::mem::zeroed();
    GetCursorPos(&mut pointer);
    ScreenToClient(hwnd, &mut pointer);
    let hover = pointer.x >= 0 && pointer.x < r.right && pointer.y >= top && pointer.y < bottom;
    if bottom > top {
        ui::rounded(
            dc,
            RECT {
                left: mid - width / 2,
                top,
                right: mid + (width + 1) / 2,
                bottom,
            },
            if hover || s.press.is_some() {
                s.palette.accent
            } else {
                s.palette.muted
            },
            s.palette.surface,
            (4. * s.scale) as i32,
        );
    }
    let pen = CreatePen(PS_SOLID, 1, s.palette.muted);
    let old = SelectObject(dc, pen);
    for (y, dir) in [(arrow / 2, -1), (r.bottom - arrow / 2, 1)] {
        let size = (3. * s.scale) as i32;
        MoveToEx(dc, mid - size, y - dir * size / 2, null_mut());
        LineTo(dc, mid, y + dir * size / 2);
        LineTo(dc, mid + size, y - dir * size / 2);
    }
    SelectObject(dc, old);
    DeleteObject(pen);
    if GetFocus() == hwnd {
        let mut f = r;
        InflateRect(&mut f, -1, -1);
        DrawFocusRect(dc, &f);
    }
}
unsafe extern "system" fn bar_proc(
    hwnd: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
    _id: usize,
    data: usize,
) -> LRESULT {
    // No State borrow is held across DefSubclassProc, which may re-enter this callback.
    match msg {
        WM_NCDESTROY => {
            RemoveWindowSubclass(hwnd, Some(bar_proc), BAR_SUBCLASS);
            drop(Box::from_raw(data as *mut State));
            return DefSubclassProc(hwnd, msg, w, l);
        }
        WM_ERASEBKGND => return 1,
        WM_PAINT | WM_PRINTCLIENT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let dc = if msg == WM_PAINT {
                BeginPaint(hwnd, &mut ps)
            } else {
                w as HDC
            };
            paint(hwnd, dc, &*(data as *const State));
            if msg == WM_PAINT {
                EndPaint(hwnd, &ps);
            }
            return 0;
        }
        WM_LBUTTONDOWN => {
            SetFocus(hwnd);
            let (_, arrow, top, bottom) = geometry(hwnd);
            let y = (l >> 16) as i16 as i32;
            let info = range(hwnd);
            let mut rect: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rect);
            let press = if y >= top && y < bottom {
                Press::Drag {
                    offset: y - top,
                    original: info.nPos,
                }
            } else {
                Press::Step(if y < arrow {
                    -1
                } else if y >= rect.bottom - arrow {
                    1
                } else if y < top {
                    -(info.nPage as i32)
                } else {
                    info.nPage as i32
                })
            };
            super::smoke_trace(&format!(
                "scroll pointer down y={y} thumb={top}..{bottom} pos={} page={} max={}",
                info.nPos, info.nPage, info.nMax
            ));
            (*(data as *mut State)).press = Some(press);
            SetCapture(hwnd);
            if let Press::Step(delta) = press {
                position(hwnd, info.nPos + delta);
                SetTimer(hwnd, 1, 300, None);
            }
            InvalidateRect(hwnd, null(), 0);
            return 0;
        }
        WM_MOUSEMOVE => {
            let mut track = TRACKMOUSEEVENT {
                cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: hwnd,
                dwHoverTime: 0,
            };
            TrackMouseEvent(&mut track);
            if let Some(Press::Drag { offset, .. }) = (*(data as *const State)).press {
                let (r, arrow, top, bottom) = geometry(hwnd);
                let travel = (r.bottom - 2 * arrow - (bottom - top)).max(1);
                let info = range(hwnd);
                let max = (info.nMax - info.nPage as i32 + 1).max(0);
                let y = (l >> 16) as i16 as i32;
                position(
                    hwnd,
                    ((y - offset - arrow) as f64 / travel as f64 * max as f64).round() as i32,
                );
            }
            InvalidateRect(hwnd, null(), 0);
            return 0;
        }
        WM_MOUSEWHEEL => {
            let s = &mut *(data as *mut State);
            s.wheel += (w >> 16) as i16 as i32;
            let lines = s.wheel / 120 * 3;
            s.wheel %= 120;
            position(hwnd, range(hwnd).nPos - lines);
            return 0;
        }
        WM_TIMER => {
            if let Some(Press::Step(delta)) = (*(data as *const State)).press {
                position(hwnd, range(hwnd).nPos + delta);
                SetTimer(hwnd, 1, 75, None);
            }
            return 0;
        }
        WM_LBUTTONUP => {
            (*(data as *mut State)).press = None;
            KillTimer(hwnd, 1);
            if GetCapture() == hwnd {
                ReleaseCapture();
            }
            InvalidateRect(hwnd, null(), 0);
            return 0;
        }
        WM_CAPTURECHANGED | WM_CANCELMODE | WM_KILLFOCUS => {
            (*(data as *mut State)).press = None;
            KillTimer(hwnd, 1);
            if GetCapture() == hwnd {
                ReleaseCapture();
            }
            InvalidateRect(hwnd, null(), 0);
        }
        WM_KEYDOWN if w as u16 == VK_ESCAPE => {
            if let Some(Press::Drag { original, .. }) = (*(data as *mut State)).press.take() {
                position(hwnd, original);
            }
            KillTimer(hwnd, 1);
            if GetCapture() == hwnd {
                ReleaseCapture();
            }
            return 0;
        }
        WM_MOUSELEAVE | WM_SETFOCUS => {
            InvalidateRect(hwnd, null(), 0);
        }
        _ => {}
    }
    DefSubclassProc(hwnd, msg, w, l)
}
