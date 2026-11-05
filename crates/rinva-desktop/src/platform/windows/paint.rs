use super::ui::{self, wide};
use std::{path::Path, ptr::null_mut};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    Storage::Xps::{PrintWindow, PW_CLIENTONLY},
    UI::WindowsAndMessaging::WNDPROC,
};

pub struct Bitmap {
    pub handle: HBITMAP,
    pub width: i32,
    pub height: i32,
}
impl Drop for Bitmap {
    fn drop(&mut self) {
        unsafe {
            DeleteObject(self.handle);
        }
    }
}
impl Bitmap {
    pub unsafe fn load(path: &Path) -> Result<Self, String> {
        if std::fs::metadata(path).map_err(|e| e.to_string())?.len()
            > rinva_core::storage::MAX_ASSET_BYTES
        {
            return Err("Изображение слишком большое (максимум 32 МБ).".into());
        }
        let mut reader = image::ImageReader::open(path).map_err(|e| e.to_string())?;
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(8192);
        limits.max_image_height = Some(8192);
        limits.max_alloc = Some(96 * 1024 * 1024);
        reader.limits(limits);
        let img = reader
            .decode()
            .map_err(|e| format!("Не удалось открыть изображение: {e}"))?
            .into_rgba8();
        let (width, height) = (img.width() as i32, img.height() as i32);
        let mut bytes = img.into_raw();
        for pixel in bytes.as_chunks_mut::<4>().0 {
            let a = pixel[3] as u16;
            let r = (pixel[0] as u16 * a / 255) as u8;
            pixel[0] = (pixel[2] as u16 * a / 255) as u8;
            pixel[1] = (pixel[1] as u16 * a / 255) as u8;
            pixel[2] = r;
        }
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = width;
        info.bmiHeader.biHeight = -height;
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB;
        let mut bits = null_mut();
        let handle = CreateDIBSection(null_mut(), &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
        if handle.is_null() {
            return Err("Не удалось выделить память для изображения.".into());
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), bits as *mut u8, bytes.len());
        Ok(Self {
            handle,
            width,
            height,
        })
    }
    pub unsafe fn draw(&self, dc: HDC, rect: RECT, portrait: bool) {
        let source = CreateCompatibleDC(dc);
        if source.is_null() {
            return;
        }
        let old = SelectObject(source, self.handle);
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        if portrait {
            let scale = (height as f32 / self.height as f32).min(width as f32 / self.width as f32);
            let w = (self.width as f32 * scale) as i32;
            let h = (self.height as f32 * scale) as i32;
            AlphaBlend(
                dc,
                rect.left + (width - w) / 2,
                rect.bottom - h,
                w,
                h,
                source,
                0,
                0,
                self.width,
                self.height,
                BLENDFUNCTION {
                    BlendOp: AC_SRC_OVER as u8,
                    BlendFlags: 0,
                    SourceConstantAlpha: 255,
                    AlphaFormat: AC_SRC_ALPHA as u8,
                },
            );
        } else {
            let scale = (width as f32 / self.width as f32).max(height as f32 / self.height as f32);
            let sw = (width as f32 / scale) as i32;
            let sh = (height as f32 / scale) as i32;
            SetStretchBltMode(dc, HALFTONE);
            SetBrushOrgEx(dc, 0, 0, null_mut());
            StretchBlt(
                dc,
                rect.left,
                rect.top,
                width,
                height,
                source,
                (self.width - sw) / 2,
                (self.height - sh) / 2,
                sw,
                sh,
                SRCCOPY,
            );
        }
        SelectObject(source, old);
        DeleteDC(source);
    }
}

pub struct SceneView<'a> {
    pub name: &'a str,
    pub speaker: &'a str,
    pub text: &'a str,
    pub background: Option<&'a Bitmap>,
    pub portrait: Option<&'a Bitmap>,
    pub ended: bool,
    pub end_text: &'a str,
}

pub unsafe fn scene(
    dc: HDC,
    bounds: RECT,
    view: SceneView<'_>,
    scale: f32,
    font: HFONT,
    small: HFONT,
) {
    ui::fill(dc, bounds, ui::rgb(0x10151e));
    let available_w = bounds.right - bounds.left;
    let available_h = bounds.bottom - bounds.top;
    let w = available_w.min(available_h * 16 / 9);
    let h = w * 9 / 16;
    if w <= 0 || h <= 0 {
        return;
    }
    let rect = RECT {
        left: bounds.left + (available_w - w) / 2,
        top: bounds.top + (available_h - h) / 2,
        right: bounds.left + (available_w + w) / 2,
        bottom: bounds.top + (available_h + h) / 2,
    };
    ui::fill(dc, rect, ui::rgb(0x223249));
    if let Some(image) = view.background {
        image.draw(dc, rect, false);
    } else {
        // A quiet scene slate remains useful before the author imports artwork.
        ui::text(
            dc,
            view.name,
            RECT {
                left: rect.left + 24,
                top: rect.top + 20,
                right: rect.right - 24,
                bottom: rect.top + 60,
            },
            ui::rgb(0xadc0d7),
            small,
            DT_LEFT | DT_END_ELLIPSIS | DT_SINGLELINE,
        );
    }
    if let Some(image) = view.portrait {
        image.draw(dc, rect, true);
    }
    let margin = (20. * scale) as i32;
    let panel_h = (h * 43 / 100).max((100. * scale) as i32).min(h);
    let panel = RECT {
        left: rect.left + margin,
        top: rect.bottom - panel_h,
        right: rect.right - margin,
        bottom: rect.bottom - margin,
    };
    ui::rounded(
        dc,
        panel,
        ui::rgb(0x151b27),
        ui::rgb(0x151b27),
        (4. * scale) as i32,
    );
    ui::fill(
        dc,
        RECT {
            right: panel.left + (3. * scale) as i32,
            ..panel
        },
        ui::rgb(0x9cbef8),
    );
    let x = panel.left + (16. * scale) as i32;
    let y = panel.top + (10. * scale) as i32;
    if !view.speaker.is_empty() {
        ui::text(
            dc,
            view.speaker,
            RECT {
                left: x,
                top: y,
                right: panel.right - 12,
                bottom: y + (22. * scale) as i32,
            },
            ui::rgb(0xb5cff8),
            font,
            DT_LEFT | DT_SINGLELINE | DT_END_ELLIPSIS,
        );
    }
    let body_y = y + if view.speaker.is_empty() {
        0
    } else {
        (27. * scale) as i32
    };
    ui::text(
        dc,
        if view.ended { view.end_text } else { view.text },
        RECT {
            left: x,
            top: body_y,
            right: panel.right - (16. * scale) as i32,
            bottom: panel.bottom - (8. * scale) as i32,
        },
        ui::rgb(0xf1f4f8),
        font,
        DT_LEFT | DT_WORDBREAK | DT_END_ELLIPSIS,
    );
}

pub unsafe fn capture(hwnd: HWND, path: &Path) -> Result<(), String> {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    let mut rect: RECT = std::mem::zeroed();
    GetClientRect(hwnd, &mut rect);
    let dc = GetDC(hwnd);
    let memory = CreateCompatibleDC(dc);
    let bitmap = CreateCompatibleBitmap(dc, rect.right, rect.bottom);
    if dc.is_null() || memory.is_null() || bitmap.is_null() {
        return Err("Screenshot allocation failed".into());
    }
    let old = SelectObject(memory, bitmap);
    PrintWindow(hwnd, memory, PW_CLIENTONLY);
    SelectObject(memory, old);
    let mut info: BITMAPINFO = std::mem::zeroed();
    info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    info.bmiHeader.biWidth = rect.right;
    info.bmiHeader.biHeight = -rect.bottom;
    info.bmiHeader.biPlanes = 1;
    info.bmiHeader.biBitCount = 32;
    let mut bytes = vec![0u8; (rect.right * rect.bottom * 4) as usize];
    let copied = GetDIBits(
        dc,
        bitmap,
        0,
        rect.bottom as u32,
        bytes.as_mut_ptr() as _,
        &mut info,
        DIB_RGB_COLORS,
    );
    DeleteObject(bitmap);
    DeleteDC(memory);
    ReleaseDC(hwnd, dc);
    if copied == 0 {
        return Err("Screenshot copy failed".into());
    }
    for pixel in bytes.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
        pixel[3] = 255;
    }
    image::save_buffer(
        path,
        &bytes,
        rect.right as u32,
        rect.bottom as u32,
        image::ColorType::Rgba8,
    )
    .map_err(|e| e.to_string())
}

pub unsafe fn register(instance: HINSTANCE, procedure: WNDPROC) -> Result<(), String> {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    let mut class: WNDCLASSW = std::mem::zeroed();
    class.lpfnWndProc = procedure;
    class.hInstance = instance;
    class.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
    // Keep the string alive until RegisterClassW has copied it.
    let name = wide("RinvaCanvas");
    class.lpszClassName = name.as_ptr();
    if RegisterClassW(&class) == 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    Ok(())
}
