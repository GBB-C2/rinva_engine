use super::*;
pub unsafe fn event(app: &mut App, msg: u32, w: WPARAM, l: LPARAM) -> bool {
    if app.player.is_some() {
        return false;
    }
    let x = l as i16 as i32;
    let y = (l >> 16) as i16 as i32;
    let mut rect: RECT = std::mem::zeroed();
    GetClientRect(app.hwnd, &mut rect);
    let width = (rect.right as f32 / app.scale) as i32;
    let (center, right, _, _) = app.panel_geometry(width);
    let logical = x as f32 / app.scale;
    let side = if logical >= (center - 20) as f32 && logical <= (center - 4) as f32 {
        Some(true)
    } else if logical >= (right - 12) as f32 && logical < right as f32 {
        Some(false)
    } else {
        None
    };
    match msg {
        WM_LBUTTONDOWN if y as f32 / app.scale >= 84. && side.is_some() => {
            app.splitter = Some((side.unwrap(), app.left_width, app.right_width));
            SetCapture(app.hwnd);
            SetFocus(app.hwnd);
            SetCursor(LoadCursorW(null_mut(), IDC_SIZEWE));
            true
        }
        WM_MOUSEMOVE => {
            if let Some((left, _, _)) = app.splitter {
                if left {
                    app.left_width = (logical as i32 - 12)
                        .clamp(180, 300)
                        .min((width - app.right_width - 572).max(180));
                } else {
                    app.right_width = (width - logical as i32 - 16)
                        .clamp(280, 420)
                        .min((width - app.left_width - 572).max(280));
                }
                app.layout();
                SetCursor(LoadCursorW(null_mut(), IDC_SIZEWE));
                return true;
            }
            if side.is_some() && y as f32 / app.scale >= 84. {
                SetCursor(LoadCursorW(null_mut(), IDC_SIZEWE));
                return true;
            }
            false
        }
        WM_LBUTTONUP if app.splitter.is_some() => {
            app.splitter = None;
            ReleaseCapture();
            if let Err(error) = app.persist_settings() {
                app.status = error;
                app.update_title();
            }
            true
        }
        WM_CAPTURECHANGED | WM_CANCELMODE | WM_KILLFOCUS => {
            cancel(app);
            false
        }
        WM_KEYDOWN if w as u16 == VK_ESCAPE && app.splitter.is_some() => {
            cancel(app);
            true
        }
        _ => false,
    }
}
unsafe fn cancel(app: &mut App) {
    if let Some((_, left, right)) = app.splitter.take() {
        app.left_width = left;
        app.right_width = right;
        if GetCapture() == app.hwnd {
            ReleaseCapture();
        }
        app.layout();
    }
}
