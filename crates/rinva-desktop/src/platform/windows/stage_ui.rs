use super::*;
use crate::stage::{self, Rect};

pub const GUIDES: u16 = 4100;
pub const FRONT: u16 = 4101;
pub const BEHIND: u16 = 4102;
#[derive(Default)]
pub struct Stage {
    pub drag: Option<Drag>,
    pub guides: bool,
}
pub struct Drag {
    index: usize,
    scene: String,
    origin: rinva_core::Sprite,
    pub preview: rinva_core::Sprite,
    start: (f32, f32),
    frame: Rect,
    resize: bool,
}
pub fn active(app: &App) -> bool {
    app.player.is_none()
        && app.navigation.mode.is_none()
        && app.studio.pane == studio::Pane::Characters
}
pub unsafe fn create(c: &mut Controls, hwnd: HWND, r: &Resources) -> Result<(), String> {
    for (id, text) in [
        (GUIDES, "Сетка 5%"),
        (FRONT, "На передний план"),
        (BEHIND, "На задний план"),
    ] {
        c.button(hwnd, id, text, r.small)?;
    }
    Ok(())
}
pub unsafe fn layout(app: &App, w: i32, _h: i32) {
    for id in [GUIDES, FRONT, BEHIND] {
        app.controls.show(id, active(app));
    }
    if !active(app) {
        return;
    }
    let (_, right, _, iw) = app.panel_geometry(w);
    app.controls.place(
        studio::CHARACTER_APPLY,
        right + 16,
        608,
        (iw - 8) / 2,
        30,
        app.scale,
    );
    app.controls.place(
        GUIDES,
        right + 20 + iw / 2,
        608,
        (iw - 8) / 2,
        30,
        app.scale,
    );
    app.controls
        .place(FRONT, right + 16, 646, (iw - 8) / 2, 30, app.scale);
    app.controls.place(
        BEHIND,
        right + 20 + iw / 2,
        646,
        (iw - 8) / 2,
        30,
        app.scale,
    );
    for (id, key) in [
        (GUIDES, "Сетка 5%"),
        (FRONT, "На передний план"),
        (BEHIND, "На задний план"),
    ] {
        app.controls.text(id, app.t(key));
    }
    for id in [FRONT, BEHIND] {
        app.controls.enable(id, !app.scene().sprites.is_empty());
    }
}
pub unsafe fn cancel(app: &mut App) {
    if app.stage.drag.take().is_some() {
        if GetCapture() == app.controls.get(CANVAS) {
            ReleaseCapture();
        }
        sync_preview(app);
        let _guard = SyncGuard::new();
        studio::refresh(app);
        InvalidateRect(app.controls.get(CANVAS), null(), 0);
    }
}
fn sync_preview(app: &mut App) {
    let sprites = app.scene().sprites.clone();
    for image in &mut app.sprites {
        if let Some(placement) = sprites.get(image.index) {
            image.placement = app
                .stage
                .drag
                .as_ref()
                .filter(|d| d.index == image.index)
                .map(|d| d.preview.clone())
                .unwrap_or_else(|| placement.clone());
        }
    }
}
pub unsafe fn command(app: &mut App, id: u16) -> bool {
    if ![GUIDES, FRONT, BEHIND].contains(&id) {
        return false;
    }
    cancel(app);
    if id == GUIDES {
        app.stage.guides = !app.stage.guides;
    } else {
        let i = app.selected_scene;
        let selected = app.studio.sprite;
        app.document.change(|p| {
            let sprites = &mut p.scenes[i].sprites;
            if selected >= sprites.len() {
                return;
            }
            let mut order: Vec<_> = (0..sprites.len()).collect();
            order.sort_by_key(|n| sprites[*n].layer);
            order.retain(|n| *n != selected);
            if id == FRONT {
                order.push(selected);
            } else {
                order.insert(0, selected);
            }
            for (layer, index) in order.into_iter().enumerate() {
                sprites[index].layer = layer as i32;
            }
        });
        app.navigation.changed();
    }
    app.refresh();
    true
}
fn image_bounds(image: &paint::SpriteImage, frame: Rect) -> Rect {
    stage::bounds(
        frame,
        &image.placement,
        image.image.width as f32 / image.image.height as f32,
    )
}
fn corners(r: Rect) -> [(f32, f32); 4] {
    [
        (r.x, r.y),
        (r.x + r.w, r.y),
        (r.x, r.y + r.h),
        (r.x + r.w, r.y + r.h),
    ]
}
fn hit_handle(r: Rect, x: f32, y: f32, size: f32) -> bool {
    corners(r)
        .iter()
        .any(|p| (p.0 - x).abs() <= size && (p.1 - y).abs() <= size)
}
pub unsafe fn paint(app: &App, dc: HDC, bounds: RECT) {
    if !active(app) {
        return;
    }
    let f = Rect::frame(bounds.right as f32, bounds.bottom as f32);
    let state = SaveDC(dc);
    SetBkMode(dc, TRANSPARENT as i32);
    IntersectClipRect(
        dc,
        f.x as i32,
        f.y as i32,
        (f.x + f.w) as i32,
        (f.y + f.h) as i32,
    );
    let pen = CreatePen(PS_DOT, 1, app.palette.muted);
    let old = SelectObject(dc, pen);
    if app.stage.guides {
        for n in 1..20 {
            let x = (f.x + f.w * n as f32 / 20.) as i32;
            let y = (f.y + f.h * n as f32 / 20.) as i32;
            MoveToEx(dc, x, f.y as i32, null_mut());
            LineTo(dc, x, (f.y + f.h) as i32);
            MoveToEx(dc, f.x as i32, y, null_mut());
            LineTo(dc, (f.x + f.w) as i32, y);
        }
    }
    SelectObject(dc, old);
    DeleteObject(pen);
    if let Some(image) = app.sprites.iter().find(|s| s.index == app.studio.sprite) {
        let r = image_bounds(image, f);
        let pen = CreatePen(
            PS_SOLID,
            (2. * app.scale).round() as i32,
            app.palette.accent,
        );
        let old = SelectObject(dc, pen);
        let brush = SelectObject(dc, GetStockObject(NULL_BRUSH));
        Rectangle(
            dc,
            r.x as i32,
            r.y as i32,
            (r.x + r.w) as i32,
            (r.y + r.h) as i32,
        );
        SelectObject(dc, brush);
        SelectObject(dc, old);
        DeleteObject(pen);
        let size = (5. * app.scale).round() as i32;
        for (x, y) in corners(r) {
            ui::rounded(
                dc,
                RECT {
                    left: x as i32 - size,
                    top: y as i32 - size,
                    right: x as i32 + size,
                    bottom: y as i32 + size,
                },
                app.palette.accent,
                app.palette.on_accent,
                2,
            );
        }
    }
    RestoreDC(dc, state);
    // Keep the hint readable over arbitrary artwork in either editor theme.
    let px = |n: f32| (n * app.scale).round() as i32;
    let hint = RECT {
        left: px(8.),
        top: px(8.),
        right: bounds.right - px(8.),
        bottom: px(32.),
    };
    ui::rounded(dc, hint, app.palette.surface, app.palette.border, px(4.));
    ui::text(
        dc,
        app.t("Тяните персонажа или угол · стрелки — сдвиг · Esc — отмена"),
        RECT {
            left: hint.left + px(8.),
            right: hint.right - px(8.),
            ..hint
        },
        app.palette.text,
        app.resources.small,
        DT_SINGLELINE | DT_END_ELLIPSIS | DT_VCENTER,
    );
}
pub unsafe fn event(app: &mut App, hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> bool {
    if matches!(
        msg,
        WM_CAPTURECHANGED | WM_CANCELMODE | WM_KILLFOCUS | WM_SIZE
    ) {
        cancel(app);
        return false;
    }
    if !active(app) {
        return false;
    }
    let x = (l as i16) as f32;
    let y = ((l >> 16) as i16) as f32;
    let mut bounds: RECT = std::mem::zeroed();
    GetClientRect(hwnd, &mut bounds);
    let f = Rect::frame(bounds.right as f32, bounds.bottom as f32);
    match msg {
        WM_LBUTTONDOWN => {
            SetFocus(hwnd);
            if !f.contains(x, y) {
                return true;
            }
            let current = app.sprites.iter().find(|s| s.index == app.studio.sprite);
            let resize =
                current.is_some_and(|s| hit_handle(image_bounds(s, f), x, y, 8. * app.scale));
            let index = if resize {
                Some(app.studio.sprite)
            } else {
                app.sprites
                    .iter()
                    .rev()
                    .find(|s| image_bounds(s, f).contains(x, y))
                    .map(|s| s.index)
            };
            if let Some(index) = index {
                app.studio.sprite = index;
                let selected = app.scene().sprites[index].clone();
                if let Some(c) = app
                    .document
                    .project
                    .characters
                    .iter()
                    .position(|c| c.id == selected.character)
                {
                    app.studio.character = c;
                    app.studio.expression = app.document.project.characters[c]
                        .expressions
                        .iter()
                        .position(|e| e.id == selected.expression)
                        .unwrap_or(0);
                }
                app.document.finish_edit();
                let origin = app.scene().sprites[index].clone();
                app.stage.drag = Some(Drag {
                    index,
                    scene: app.scene().id.clone(),
                    preview: origin.clone(),
                    origin,
                    start: (x, y),
                    frame: f,
                    resize,
                });
                {
                    let _guard = SyncGuard::new();
                    studio::refresh(app);
                }
                SetCapture(hwnd);
                InvalidateRect(hwnd, null(), 0);
            }
            true
        }
        WM_MOUSEMOVE => {
            if let Some(d) = &mut app.stage.drag {
                d.preview = if d.resize {
                    let anchor = (
                        d.frame.x + d.frame.w * d.origin.x as f32 / 100.,
                        d.frame.y + d.frame.h * d.origin.y as f32 / 100.,
                    );
                    stage::resize(&d.origin, anchor, d.start, (x, y))
                } else {
                    stage::translate(
                        &d.origin,
                        d.frame,
                        x - d.start.0,
                        y - d.start.1,
                        app.stage.guides,
                    )
                };
                let _guard = SyncGuard::new();
                for (id, value) in [
                    (studio::SPRITE_X, d.preview.x),
                    (studio::SPRITE_Y, d.preview.y),
                    (studio::SPRITE_SCALE, d.preview.scale as i32),
                ] {
                    app.controls.text(id, &value.to_string());
                }
                sync_preview(app);
                InvalidateRect(hwnd, null(), 0);
                SetCursor(LoadCursorW(null_mut(), IDC_SIZEALL));
                true
            } else {
                false
            }
        }
        WM_LBUTTONUP => {
            if let Some(d) = app.stage.drag.take() {
                if d.scene == app.scene().id {
                    let i = app.selected_scene;
                    app.document.change(|p| {
                        if let Some(s) = p.scenes[i].sprites.get_mut(d.index) {
                            *s = d.preview;
                        }
                    });
                    app.navigation.changed();
                }
                ReleaseCapture();
                app.refresh();
            }
            true
        }
        WM_KEYDOWN if w as u16 == VK_ESCAPE => {
            cancel(app);
            true
        }
        WM_KEYDOWN if [VK_LEFT, VK_RIGHT, VK_UP, VK_DOWN].contains(&(w as u16)) => {
            cancel(app);
            let step = if GetKeyState(VK_SHIFT as i32) < 0 {
                10
            } else {
                1
            };
            let (dx, dy) = match w as u16 {
                VK_LEFT => (-step, 0),
                VK_RIGHT => (step, 0),
                VK_UP => (0, -step),
                _ => (0, step),
            };
            let i = app.selected_scene;
            let selected = app.studio.sprite;
            app.document.change(|p| {
                if let Some(s) = p.scenes[i].sprites.get_mut(selected) {
                    s.x = (s.x + dx).clamp(-100, 200);
                    s.y = (s.y + dy).clamp(-100, 200);
                }
            });
            app.navigation.changed();
            app.refresh();
            true
        }
        _ => false,
    }
}
