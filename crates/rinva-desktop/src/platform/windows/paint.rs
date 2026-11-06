use super::ui::{self, wide};
use std::{path::Path, ptr::null_mut};
use windows_sys::Win32::{Foundation::*, Graphics::Gdi::*, UI::WindowsAndMessaging::WNDPROC};

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
        Self::from_rgba(img)
    }
    pub unsafe fn from_png(bytes: &[u8]) -> Result<Self, String> {
        if bytes.is_empty() || bytes.len() > rinva_core::saves::MAX_THUMBNAIL {
            return Err("Invalid thumbnail".into());
        }
        let mut reader =
            image::ImageReader::with_format(std::io::Cursor::new(bytes), image::ImageFormat::Png);
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(320);
        limits.max_image_height = Some(180);
        limits.max_alloc = Some(4 * 1024 * 1024);
        reader.limits(limits);
        Self::from_rgba(reader.decode().map_err(|e| e.to_string())?.into_rgba8())
    }
    unsafe fn from_rgba(img: image::RgbaImage) -> Result<Self, String> {
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
    pub sprites: &'a [SpriteImage],
    pub ended: bool,
    pub dialog_style: u8,
    pub show_dialog: bool,
    pub end_text: &'a str,
}
pub struct SpriteImage {
    pub index: usize,
    pub image: Bitmap,
    pub name: String,
    pub placement: rinva_core::Sprite,
}
impl SpriteImage {
    unsafe fn draw(&self, dc: HDC, rect: RECT, speaker: &str) {
        let r = crate::stage::bounds(
            crate::stage::Rect {
                x: rect.left as f32,
                y: rect.top as f32,
                w: (rect.right - rect.left) as f32,
                h: (rect.bottom - rect.top) as f32,
            },
            &self.placement,
            self.image.width as f32 / self.image.height as f32,
        );
        let (x, y, w, h) = (r.x, r.y, r.w, r.h);
        let source = CreateCompatibleDC(dc);
        if source.is_null() {
            return;
        }
        let old = SelectObject(source, self.image.handle);
        AlphaBlend(
            dc,
            x as i32,
            y as i32,
            w as i32,
            h as i32,
            source,
            0,
            0,
            self.image.width,
            self.image.height,
            BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: if speaker.is_empty() || speaker == self.name {
                    255
                } else {
                    160
                },
                AlphaFormat: AC_SRC_ALPHA as u8,
            },
        );
        SelectObject(source, old);
        DeleteDC(source);
    }
}

pub unsafe fn scene(
    dc: HDC,
    bounds: RECT,
    view: SceneView<'_>,
    scale: f32,
    font: HFONT,
    small: HFONT,
) {
    ui::fill(dc, bounds, ui::rgb(0x101010));
    let frame = crate::stage::Rect::frame(
        (bounds.right - bounds.left) as f32,
        (bounds.bottom - bounds.top) as f32,
    );
    let rect = RECT {
        left: bounds.left + frame.x as i32,
        top: bounds.top + frame.y as i32,
        right: bounds.left + (frame.x + frame.w) as i32,
        bottom: bounds.top + (frame.y + frame.h) as i32,
    };
    let h = rect.bottom - rect.top;
    if rect.right <= rect.left || h <= 0 {
        return;
    }
    ui::fill(dc, rect, ui::rgb(0x282828));
    if let Some(image) = view.background {
        image.draw(dc, rect, false);
    } else if view.show_dialog {
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
            ui::rgb(0xbdbdbd),
            small,
            DT_LEFT | DT_END_ELLIPSIS | DT_SINGLELINE,
        );
    }
    if let Some(image) = view.portrait {
        image.draw(dc, rect, true);
    }
    let state = SaveDC(dc);
    IntersectClipRect(dc, rect.left, rect.top, rect.right, rect.bottom);
    for sprite in view.sprites {
        sprite.draw(dc, rect, view.speaker);
    }
    RestoreDC(dc, state);
    if !view.show_dialog {
        return;
    }
    let panel_color = if view.dialog_style == 1 {
        ui::rgb(0xf4f4f4)
    } else {
        ui::rgb(0x181818)
    };
    let text_color = if view.dialog_style == 1 {
        ui::rgb(0x242424)
    } else {
        ui::rgb(0xf1f1f1)
    };
    let accent = if view.dialog_style == 1 {
        ui::rgb(0x444444)
    } else {
        ui::rgb(0xd7d7d7)
    };
    let margin = (20. * scale) as i32;
    let panel_h = (h * 43 / 100).max((100. * scale) as i32).min(h);
    let panel = RECT {
        left: rect.left + margin,
        top: rect.bottom - panel_h,
        right: rect.right - margin,
        bottom: rect.bottom - margin,
    };
    ui::rounded(dc, panel, panel_color, panel_color, (4. * scale) as i32);
    ui::fill(
        dc,
        RECT {
            right: panel.left + (3. * scale) as i32,
            ..panel
        },
        ui::rgb(0xe0e0e0),
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
            accent,
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
        text_color,
        font,
        DT_LEFT | DT_WORDBREAK | DT_END_ELLIPSIS,
    );
}

/// Render only the live scene, never menus or desktop pixels, into a bounded thumbnail.
pub unsafe fn thumbnail(app: &super::App) -> Result<Vec<u8>, String> {
    let dc = CreateCompatibleDC(null_mut());
    if dc.is_null() {
        return Err("Thumbnail allocation failed".into());
    }
    let mut info: BITMAPINFO = std::mem::zeroed();
    info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    info.bmiHeader.biWidth = 320;
    info.bmiHeader.biHeight = -180;
    info.bmiHeader.biPlanes = 1;
    info.bmiHeader.biBitCount = 32;
    info.bmiHeader.biCompression = BI_RGB;
    let mut bits = null_mut();
    let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
    if bitmap.is_null() {
        DeleteDC(dc);
        return Err("Thumbnail allocation failed".into());
    }
    let old = SelectObject(dc, bitmap);
    app.paint_canvas(
        dc,
        RECT {
            left: 0,
            top: 0,
            right: 320,
            bottom: 180,
        },
    );
    GdiFlush();
    let mut bytes = std::slice::from_raw_parts(bits as *const u8, 320 * 180 * 4).to_vec();
    SelectObject(dc, old);
    DeleteObject(bitmap);
    DeleteDC(dc);
    for pixel in bytes.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
        pixel[3] = 255;
    }
    let image = image::RgbaImage::from_raw(320, 180, bytes).ok_or("Invalid thumbnail size")?;
    let mut out = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut out, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(out.into_inner())
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
    // Complete pending theme/layout paints, then ask the live HWND hierarchy to
    // render synchronously. PrintWindow's redirected surface can be stale on a
    // virtual desktop immediately after theme or size changes.
    RedrawWindow(
        hwnd,
        std::ptr::null(),
        null_mut(),
        RDW_INVALIDATE | RDW_ERASE | RDW_FRAME | RDW_ALLCHILDREN | RDW_UPDATENOW,
    );
    // WM_PRINT positions children in window coordinates. Render its nonclient
    // frame too, offset outside this client-sized bitmap, so all controls retain
    // their actual client coordinates instead of shifting by the title bar.
    let mut window: RECT = std::mem::zeroed();
    GetWindowRect(hwnd, &mut window);
    let mut client = POINT { x: 0, y: 0 };
    ClientToScreen(hwnd, &mut client);
    SetViewportOrgEx(
        memory,
        window.left - client.x,
        window.top - client.y,
        null_mut(),
    );
    SendMessageW(
        hwnd,
        WM_PRINT,
        memory as usize,
        (PRF_NONCLIENT | PRF_CLIENT | PRF_CHILDREN | PRF_ERASEBKGND) as isize,
    );
    GdiFlush();
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
