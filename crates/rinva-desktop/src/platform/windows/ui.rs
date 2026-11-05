//! Small Win32 adapters. All HWNDs are borrowed from the UI thread; Windows owns
//! child controls. GDI objects created here are owned and released by `Resources`.
use super::scroll;
use crate::i18n::Language;
use std::{
    collections::BTreeMap,
    path::PathBuf,
    ptr::{null, null_mut},
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::{Dwm::*, Gdi::*},
    System::{LibraryLoader::*, SystemServices::SS_LEFT},
    UI::{
        Controls::{Dialogs::*, *},
        Input::KeyboardAndMouse::*,
        Shell::{DefSubclassProc, GetWindowSubclass, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::*,
    },
};

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
pub fn rgb(hex: u32) -> u32 {
    ((hex & 255) << 16) | (hex & 0xff00) | (hex >> 16)
}

#[derive(Clone, Copy)]
pub struct Palette {
    pub window: u32,
    pub surface: u32,
    pub field: u32,
    pub border: u32,
    pub text: u32,
    pub muted: u32,
    pub accent: u32,
    pub on_accent: u32,
    pub selected: u32,
}
impl Palette {
    pub fn new(dark: bool) -> Self {
        let c = if dark {
            [
                0x171b22, 0x202630, 0x2b323e, 0x3c4655, 0xf1f4f8, 0xaeb9c9, 0x9cbef8, 0x14233d,
                0x354765,
            ]
        } else {
            [
                0xf0f2f6, 0xffffff, 0xe9edf4, 0xcbd3df, 0x202b3b, 0x58677b, 0x315fc4, 0xffffff,
                0xdce8fc,
            ]
        };
        Self {
            window: rgb(c[0]),
            surface: rgb(c[1]),
            field: rgb(c[2]),
            border: rgb(c[3]),
            text: rgb(c[4]),
            muted: rgb(c[5]),
            accent: rgb(c[6]),
            on_accent: rgb(c[7]),
            selected: rgb(c[8]),
        }
    }
}

pub struct Resources {
    pub font: HFONT,
    pub title: HFONT,
    pub small: HFONT,
    pub field_brush: HBRUSH,
    pub surface_brush: HBRUSH,
}
impl Resources {
    pub unsafe fn new(scale: f32, palette: Palette, language: Language) -> Self {
        let font = |size: f32, weight: u32| {
            CreateFontW(
                -(size * scale).round() as i32,
                0,
                0,
                0,
                weight as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET as u32,
                OUT_DEFAULT_PRECIS as u32,
                CLIP_DEFAULT_PRECIS as u32,
                CLEARTYPE_QUALITY as u32,
                DEFAULT_PITCH as u32,
                wide(super::fonts::family(language)).as_ptr(),
            )
        };
        Self {
            font: font(14., FW_NORMAL),
            title: font(21., FW_SEMIBOLD),
            small: font(12., FW_NORMAL),
            field_brush: CreateSolidBrush(palette.field),
            surface_brush: CreateSolidBrush(palette.surface),
        }
    }
}
impl Drop for Resources {
    fn drop(&mut self) {
        unsafe {
            for object in [
                self.font,
                self.title,
                self.small,
                self.field_brush,
                self.surface_brush,
            ] {
                if !object.is_null() {
                    DeleteObject(object);
                }
            }
        }
    }
}

pub struct Controls {
    pub handles: BTreeMap<u16, HWND>,
    pub scrolls: BTreeMap<u16, scroll::Bar>,
    pub palette: Palette,
    pub scale: f32,
}
impl Controls {
    pub fn new(palette: Palette, scale: f32) -> Self {
        Self {
            handles: BTreeMap::new(),
            scrolls: BTreeMap::new(),
            palette,
            scale,
        }
    }
    pub fn get(&self, id: u16) -> HWND {
        self.handles.get(&id).copied().unwrap_or(null_mut())
    }
    pub unsafe fn add(
        &mut self,
        parent: HWND,
        id: u16,
        class: &str,
        text: &str,
        style: u32,
        font: HFONT,
    ) -> Result<(), String> {
        let handle = CreateWindowExW(
            0,
            wide(class).as_ptr(),
            wide(text).as_ptr(),
            WS_CHILD | WS_VISIBLE | style,
            0,
            0,
            1,
            1,
            parent,
            id as usize as HMENU,
            GetModuleHandleW(null()),
            null(),
        );
        if handle.is_null() {
            return Err(format!(
                "Не удалось создать элемент {id}: {}",
                std::io::Error::last_os_error()
            ));
        }
        SendMessageW(handle, WM_SETFONT, font as usize, 0);
        self.handles.insert(id, handle);
        if class == "EDIT" || class == "LISTBOX" {
            let data = Box::into_raw(Box::new(FieldStyle {
                palette: self.palette,
                scale: self.scale,
                list: class == "LISTBOX",
                multiline: style & ES_MULTILINE as u32 != 0,
                composing: false,
            }));
            if SetWindowSubclass(handle, Some(field_proc), 2, data as usize) == 0 {
                drop(Box::from_raw(data));
                return Err("Could not style native field".into());
            }
        }
        Ok(())
    }
    pub unsafe fn button(
        &mut self,
        parent: HWND,
        id: u16,
        text: &str,
        font: HFONT,
    ) -> Result<(), String> {
        self.add(
            parent,
            id,
            "BUTTON",
            text,
            WS_TABSTOP | BS_OWNERDRAW as u32,
            font,
        )?;
        SetWindowSubclass(self.get(id), Some(button_proc), 1, 0);
        Ok(())
    }
    pub unsafe fn label(
        &mut self,
        parent: HWND,
        id: u16,
        text: &str,
        font: HFONT,
    ) -> Result<(), String> {
        self.add(parent, id, "STATIC", text, SS_LEFT, font)
    }
    pub unsafe fn edit(
        &mut self,
        parent: HWND,
        id: u16,
        multiline: bool,
        font: HFONT,
    ) -> Result<(), String> {
        let style = WS_TABSTOP
            | WS_BORDER
            | if multiline {
                ES_MULTILINE as u32 | ES_AUTOVSCROLL as u32 | ES_WANTRETURN as u32
            } else {
                ES_AUTOHSCROLL as u32
            };
        self.add(parent, id, "EDIT", "", style, font)?;
        SendMessageW(
            self.get(id),
            EM_SETMARGINS,
            (EC_LEFTMARGIN | EC_RIGHTMARGIN) as usize,
            8 | (8 << 16),
        );
        if multiline {
            let bar = scroll::attach(
                parent,
                self.get(id),
                false,
                id + 1000,
                self.palette,
                self.scale,
            )?;
            self.scrolls.insert(id, bar);
        }
        SendMessageW(
            self.get(id),
            EM_SETLIMITTEXT,
            if multiline { 4000 } else { 160 },
            0,
        );
        Ok(())
    }
    pub unsafe fn list(&mut self, parent: HWND, id: u16, font: HFONT) -> Result<(), String> {
        self.add(
            parent,
            id,
            "LISTBOX",
            "",
            WS_TABSTOP
                | LBS_NOTIFY as u32
                | LBS_DISABLENOSCROLL as u32
                | LBS_OWNERDRAWFIXED as u32
                | LBS_HASSTRINGS as u32
                | LBS_NOINTEGRALHEIGHT as u32,
            font,
        )?;
        let bar = scroll::attach(
            parent,
            self.get(id),
            true,
            id + 1000,
            self.palette,
            self.scale,
        )?;
        self.scrolls.insert(id, bar);
        Ok(())
    }
    pub unsafe fn combo(&mut self, parent: HWND, id: u16, font: HFONT) -> Result<(), String> {
        self.add(
            parent,
            id,
            "COMBOBOX",
            "",
            WS_TABSTOP | WS_VSCROLL | CBS_DROPDOWNLIST as u32,
            font,
        )
    }
    pub unsafe fn place(&self, id: u16, x: i32, y: i32, w: i32, h: i32, scale: f32) {
        let px = |v: i32| (v as f32 * scale).round() as i32;
        let bar_width = if self.scrolls.contains_key(&id) {
            16
        } else {
            0
        };
        MoveWindow(
            self.get(id),
            px(x),
            px(y),
            px((w - bar_width).max(1)),
            px(h.max(1)),
            1,
        );
        if let Some(bar) = self.scrolls.get(&id) {
            MoveWindow(bar.hwnd, px(x + w - 14), px(y), px(14), px(h), 1);
            scroll::sync(*bar);
        }
        if id < 700 && id != super::STATUS {
            let mut rect: RECT = std::mem::zeroed();
            GetWindowRect(self.get(id), &mut rect);
            let region = CreateRoundRectRgn(
                0,
                0,
                rect.right - rect.left + 1,
                rect.bottom - rect.top + 1,
                px(8),
                px(8),
            );
            if SetWindowRgn(self.get(id), region, 1) == 0 {
                DeleteObject(region);
            }
        }
        let mut data = 0;
        if GetWindowSubclass(self.get(id), Some(field_proc), 2, &mut data) != 0 {
            let style = &*(data as *const FieldStyle);
            if style.multiline && !style.list {
                let mut r: RECT = std::mem::zeroed();
                GetClientRect(self.get(id), &mut r);
                InflateRect(&mut r, -px(8), -px(8));
                SendMessageW(self.get(id), EM_SETRECTNP, 0, &r as *const _ as isize);
            }
        }
    }
    pub unsafe fn text(&self, id: u16, value: &str) {
        if window_text(self.get(id)) != value {
            SetWindowTextW(self.get(id), wide(value).as_ptr());
        }
    }
    pub unsafe fn read(&self, id: u16) -> String {
        window_text(self.get(id))
    }
    pub unsafe fn show(&self, id: u16, show: bool) {
        ShowWindow(self.get(id), if show { SW_SHOW } else { SW_HIDE });
        if let Some(bar) = self.scrolls.get(&id) {
            if show {
                scroll::sync(*bar);
            } else {
                ShowWindow(bar.hwnd, SW_HIDE);
            }
        }
    }
    pub unsafe fn enable(&self, id: u16, on: bool) {
        windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow(self.get(id), on as i32);
        if let Some(bar) = self.scrolls.get(&id) {
            EnableWindow(bar.hwnd, on as i32);
        }
    }
    pub unsafe fn fill_list(&self, id: u16, items: &[String], selected: usize, combo: bool) {
        let handle = self.get(id);
        let old_selection = if combo {
            -1
        } else {
            SendMessageW(handle, LB_GETCURSEL, 0, 0)
        };
        let old_top = if combo {
            0
        } else {
            SendMessageW(handle, LB_GETTOPINDEX, 0, 0)
        };
        SendMessageW(handle, WM_SETREDRAW, 0, 0);
        SendMessageW(
            handle,
            if combo {
                CB_RESETCONTENT
            } else {
                LB_RESETCONTENT
            },
            0,
            0,
        );
        for value in items {
            SendMessageW(
                handle,
                if combo { CB_ADDSTRING } else { LB_ADDSTRING },
                0,
                wide(value).as_ptr() as isize,
            );
        }
        SendMessageW(
            handle,
            if combo { CB_SETCURSEL } else { LB_SETCURSEL },
            selected,
            0,
        );
        if !combo && old_selection == selected as isize {
            SendMessageW(handle, LB_SETTOPINDEX, old_top.max(0) as usize, 0);
        }
        SendMessageW(handle, WM_SETREDRAW, 1, 0);
        InvalidateRect(handle, null(), 0);
    }
    pub unsafe fn sync_scrolls(&self) {
        for bar in self.scrolls.values() {
            scroll::sync(*bar);
        }
    }
    pub unsafe fn style(&mut self, palette: Palette, scale: f32, language: Language) {
        self.palette = palette;
        self.scale = scale;
        for &handle in self.handles.values() {
            let mut data = 0;
            if GetWindowSubclass(handle, Some(field_proc), 2, &mut data) != 0 {
                let s = &mut *(data as *mut FieldStyle);
                s.palette = palette;
                s.scale = scale;
                RedrawWindow(handle, null(), null_mut(), RDW_INVALIDATE | RDW_FRAME);
            }
        }
        for bar in self.scrolls.values() {
            scroll::style(*bar, palette, scale);
            SetWindowTextW(
                bar.hwnd,
                wide(language.text("Вертикальная прокрутка")).as_ptr(),
            );
        }
    }
    pub unsafe fn selection(&self, id: u16, combo: bool) -> Option<usize> {
        let value = SendMessageW(
            self.get(id),
            if combo { CB_GETCURSEL } else { LB_GETCURSEL },
            0,
            0,
        );
        (value >= 0).then_some(value as usize)
    }
}

unsafe extern "system" fn button_proc(
    hwnd: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    if msg == WM_MOUSEMOVE {
        let mut track = TRACKMOUSEEVENT {
            cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
            dwFlags: TME_LEAVE,
            hwndTrack: hwnd,
            dwHoverTime: 0,
        };
        TrackMouseEvent(&mut track);
        InvalidateRect(hwnd, null(), 0);
    } else if msg == WM_MOUSELEAVE {
        InvalidateRect(hwnd, null(), 0);
    }
    DefSubclassProc(hwnd, msg, w, l)
}

pub unsafe fn window_text(hwnd: HWND) -> String {
    let size = GetWindowTextLengthW(hwnd).max(0) as usize;
    let mut text = vec![0u16; size + 1];
    let copied = GetWindowTextW(hwnd, text.as_mut_ptr(), text.len() as i32).max(0) as usize;
    String::from_utf16_lossy(&text[..copied])
}

pub unsafe fn fill(dc: HDC, rect: RECT, color: u32) {
    let brush = CreateSolidBrush(color);
    FillRect(dc, &rect, brush);
    DeleteObject(brush);
}
pub unsafe fn text(dc: HDC, value: &str, mut rect: RECT, color: u32, font: HFONT, flags: u32) {
    let old = SelectObject(dc, font);
    SetBkMode(dc, TRANSPARENT as i32);
    SetTextColor(dc, color);
    DrawTextW(dc, wide(value).as_ptr(), -1, &mut rect, flags | DT_NOPREFIX);
    SelectObject(dc, old);
}

pub unsafe fn theme_title(hwnd: HWND, dark: bool) {
    let value = dark as i32;
    // Unsupported DWM attributes simply leave the system-owned caption unchanged.
    DwmSetWindowAttribute(
        hwnd,
        DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
        &value as *const _ as _,
        std::mem::size_of::<i32>() as u32,
    );
}

pub unsafe fn message(hwnd: HWND, title: &str, value: &str, flags: u32) -> i32 {
    MessageBoxW(hwnd, wide(value).as_ptr(), wide(title).as_ptr(), flags)
}

pub unsafe fn file_dialog(
    hwnd: HWND,
    language: Language,
    save: bool,
    image: bool,
    initial: &str,
    title: &str,
) -> Result<Option<PathBuf>, String> {
    let filter = wide(&format!(
        "{}\0{}\0\0",
        language.text(if image {
            "Изображения PNG и JPEG"
        } else {
            "Проект Rinva"
        }),
        if image {
            "*.png;*.jpg;*.jpeg"
        } else {
            "*.rinva"
        }
    ));
    let mut file = vec![0u16; 32768];
    let initial = wide(initial);
    let n = initial.len().min(file.len());
    file[..n].copy_from_slice(&initial[..n]);
    let extension = wide("rinva");
    let title = wide(title);
    let mut dialog: OPENFILENAMEW = std::mem::zeroed();
    dialog.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    dialog.hwndOwner = hwnd;
    dialog.lpstrFilter = filter.as_ptr();
    dialog.lpstrFile = file.as_mut_ptr();
    dialog.nMaxFile = file.len() as u32;
    dialog.lpstrDefExt = if image { null() } else { extension.as_ptr() };
    dialog.lpstrTitle = title.as_ptr();
    dialog.Flags = OFN_EXPLORER
        | OFN_NOCHANGEDIR
        | OFN_PATHMUSTEXIST
        | if save {
            OFN_OVERWRITEPROMPT
        } else {
            OFN_FILEMUSTEXIST
        };
    let ok = if save {
        GetSaveFileNameW(&mut dialog)
    } else {
        GetOpenFileNameW(&mut dialog)
    };
    if ok == 0 {
        let code = CommDlgExtendedError();
        return if code == 0 {
            Ok(None)
        } else {
            Err(format!("Не удалось открыть окно выбора файла: {code:#x}"))
        };
    }
    let length = file.iter().position(|c| *c == 0).unwrap_or(file.len());
    use std::os::windows::ffi::OsStringExt;
    Ok(Some(PathBuf::from(std::ffi::OsString::from_wide(
        &file[..length],
    ))))
}

/// Radius is in device pixels; the shared 4-DIP token is scaled by each caller.
pub unsafe fn rounded(dc: HDC, rect: RECT, color: u32, border: u32, radius: i32) {
    let brush = CreateSolidBrush(color);
    let pen = CreatePen(PS_SOLID, 1, border);
    let old_brush = SelectObject(dc, brush);
    let old_pen = SelectObject(dc, pen);
    RoundRect(
        dc,
        rect.left,
        rect.top,
        rect.right,
        rect.bottom,
        radius * 2,
        radius * 2,
    );
    SelectObject(dc, old_brush);
    SelectObject(dc, old_pen);
    DeleteObject(brush);
    DeleteObject(pen);
}
struct FieldStyle {
    palette: Palette,
    scale: f32,
    list: bool,
    multiline: bool,
    composing: bool,
}
pub unsafe fn composing(hwnd: HWND) -> bool {
    let mut data = 0;
    GetWindowSubclass(hwnd, Some(field_proc), 2, &mut data) != 0
        && (*(data as *const FieldStyle)).composing
}
unsafe extern "system" fn field_proc(
    hwnd: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
    _id: usize,
    data: usize,
) -> LRESULT {
    if msg == WM_NCDESTROY {
        RemoveWindowSubclass(hwnd, Some(field_proc), 2);
        drop(Box::from_raw(data as *mut FieldStyle));
        return DefSubclassProc(hwnd, msg, w, l);
    }
    if msg == WM_IME_STARTCOMPOSITION {
        (*(data as *mut FieldStyle)).composing = true;
    } else if msg == WM_IME_ENDCOMPOSITION || msg == WM_KILLFOCUS {
        (*(data as *mut FieldStyle)).composing = false;
    }
    if msg == WM_MOUSEWHEEL {
        let style = &*(data as *const FieldStyle);
        let lines = -((w >> 16) as i16 as i32) / 120 * 3;
        if style.list {
            let current = SendMessageW(hwnd, LB_GETTOPINDEX, 0, 0) as i32;
            SendMessageW(hwnd, LB_SETTOPINDEX, (current + lines).max(0) as usize, 0);
        } else if style.multiline {
            SendMessageW(hwnd, EM_LINESCROLL, 0, lines as isize);
        }
        PostMessageW(GetParent(hwnd), scroll::SYNC_MESSAGE, 0, 0);
        return 0;
    }
    let result = DefSubclassProc(hwnd, msg, w, l);
    if matches!(msg, WM_NCPAINT | WM_SETFOCUS | WM_KILLFOCUS) {
        let style = &*(data as *const FieldStyle);
        if !style.list {
            let dc = GetWindowDC(hwnd);
            let mut r: RECT = std::mem::zeroed();
            GetWindowRect(hwnd, &mut r);
            OffsetRect(&mut r, -r.left, -r.top);
            let old = SelectObject(dc, GetStockObject(NULL_BRUSH));
            let pen = CreatePen(
                PS_SOLID,
                1,
                if GetFocus() == hwnd {
                    style.palette.accent
                } else {
                    style.palette.border
                },
            );
            let old_pen = SelectObject(dc, pen);
            RoundRect(
                dc,
                0,
                0,
                r.right,
                r.bottom,
                (8. * style.scale) as i32,
                (8. * style.scale) as i32,
            );
            SelectObject(dc, old);
            SelectObject(dc, old_pen);
            DeleteObject(pen);
            ReleaseDC(hwnd, dc);
        }
    }
    if matches!(
        msg,
        WM_KEYUP
            | WM_LBUTTONUP
            | WM_SETTEXT
            | WM_SIZE
            | WM_VSCROLL
            | EM_LINESCROLL
            | LB_SETTOPINDEX
    ) {
        PostMessageW(GetParent(hwnd), scroll::SYNC_MESSAGE, 0, 0);
    }
    result
}
