use super::*;
#[derive(Clone, Copy)]
enum Axis {
    Left,
    Right,
    Preview,
}
pub struct Drag {
    axis: Axis,
    left: i32,
    right: i32,
    preview: i32,
}
pub unsafe fn event(app: &mut App, msg: u32, w: WPARAM, l: LPARAM) -> bool {
    if app.player.is_some() {
        return false;
    }
    let x = l as i16 as f32 / app.scale;
    let y = (l >> 16) as i16 as f32 / app.scale;
    let mut rect: RECT = std::mem::zeroed();
    GetClientRect(app.hwnd, &mut rect);
    let width = (rect.right as f32 / app.scale) as i32;
    let height = (rect.bottom as f32 / app.scale) as i32;
    let (center, right, cw, _) = app.panel_geometry(width);
    let divider = 120 + app.preview_height(height);
    let hit = if x >= (center - 20) as f32 && x <= (center - 4) as f32 && y >= 84. {
        Some(Axis::Left)
    } else if x >= (right - 12) as f32 && x < right as f32 && y >= 84. {
        Some(Axis::Right)
    } else if app.graph_mode
        && app.navigation.mode.is_none()
        && x >= center as f32
        && x <= (center + cw) as f32
        && y >= divider as f32
        && y < (divider + 12) as f32
    {
        Some(Axis::Preview)
    } else {
        None
    };
    let cursor = |axis| {
        LoadCursorW(
            null_mut(),
            if matches!(axis, Axis::Preview) {
                IDC_SIZENS
            } else {
                IDC_SIZEWE
            },
        )
    };
    match msg {
        WM_LBUTTONDOWN if hit.is_some() => {
            app.splitter = Some(Drag {
                axis: hit.unwrap(),
                left: app.left_width,
                right: app.right_width,
                preview: app.preview_share,
            });
            SetCapture(app.hwnd);
            SetFocus(app.hwnd);
            SetCursor(cursor(hit.unwrap()));
            true
        }
        WM_MOUSEMOVE => {
            if let Some(drag) = &app.splitter {
                let axis = drag.axis;
                match axis {
                    Axis::Left => {
                        app.left_width = (x as i32 - 12)
                            .clamp(180, 300)
                            .min((width - app.right_width - 572).max(180))
                    }
                    Axis::Right => {
                        app.right_width = (width - x as i32 - 16)
                            .clamp(280, 420)
                            .min((width - app.left_width - 572).max(280))
                    }
                    Axis::Preview => {
                        app.preview_share =
                            ((y as i32 - 120) * 100 / (height - 280).max(1)).clamp(25, 65)
                    }
                }
                app.layout();
                SetCursor(cursor(axis));
                return true;
            }
            if let Some(axis) = hit {
                SetCursor(cursor(axis));
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
    if let Some(drag) = app.splitter.take() {
        app.left_width = drag.left;
        app.right_width = drag.right;
        app.preview_share = drag.preview;
        if GetCapture() == app.hwnd {
            ReleaseCapture();
        }
        app.layout();
    }
}
