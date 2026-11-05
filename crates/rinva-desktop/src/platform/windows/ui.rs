//! Small Win32 adapters. All HWNDs are borrowed from the UI thread; Windows owns
//! child controls. GDI objects created here are owned and released by `Resources`.
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
        Controls::{Dialogs::*, EM_SETLIMITTEXT, WM_MOUSELEAVE},
        Input::KeyboardAndMouse::*,
        Shell::{DefSubclassProc, SetWindowSubclass},
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
    pub unsafe fn new(scale: f32, palette: Palette) -> Self {
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
                wide("Segoe UI").as_ptr(),
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
}
impl Controls {
    pub fn new() -> Self {
        Self {
            handles: BTreeMap::new(),
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
                ES_MULTILINE as u32 | ES_AUTOVSCROLL as u32 | ES_WANTRETURN as u32 | WS_VSCROLL
            } else {
                ES_AUTOHSCROLL as u32
            };
        self.add(parent, id, "EDIT", "", style, font)?;
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
                | WS_VSCROLL
                | LBS_NOTIFY as u32
                | LBS_OWNERDRAWFIXED as u32
                | LBS_HASSTRINGS as u32
                | LBS_NOINTEGRALHEIGHT as u32,
            font,
        )
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
        MoveWindow(self.get(id), px(x), px(y), px(w.max(1)), px(h.max(1)), 1);
    }
    pub unsafe fn text(&self, id: u16, value: &str) {
        SetWindowTextW(self.get(id), wide(value).as_ptr());
    }
    pub unsafe fn read(&self, id: u16) -> String {
        window_text(self.get(id))
    }
    pub unsafe fn show(&self, id: u16, show: bool) {
        ShowWindow(self.get(id), if show { SW_SHOW } else { SW_HIDE });
    }
    pub unsafe fn enable(&self, id: u16, on: bool) {
        windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow(self.get(id), on as i32);
    }
    pub unsafe fn fill_list(&self, id: u16, items: &[String], selected: usize, combo: bool) {
        let handle = self.get(id);
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
        SendMessageW(handle, WM_SETREDRAW, 1, 0);
        InvalidateRect(handle, null(), 0);
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
    save: bool,
    image: bool,
    initial: &str,
    title: &str,
) -> Result<Option<PathBuf>, String> {
    let filter = wide(if image {
        "Изображения PNG и JPEG\0*.png;*.jpg;*.jpeg\0\0"
    } else {
        "Проект Rinva\0*.rinva\0\0"
    });
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
