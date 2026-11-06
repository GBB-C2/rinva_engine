//! Node-authored game UI. Native BUTTON controls retain keyboard/accessibility behavior.
use super::*;
use rinva_core::{NodeKind, Scene, UiElement};

pub const TEXT: u16 = 2100;
pub const X: u16 = 2101;
pub const Y: u16 = 2102;
pub const WIDTH: u16 = 2103;
pub const HEIGHT: u16 = 2104;
pub const TARGET: u16 = 2105;
pub const APPLY: u16 = 2106;
const LABEL: u16 = 2110;
pub const BUTTON: u16 = 6000;
const LIMIT: u16 = 64;

pub unsafe fn create(c: &mut Controls, hwnd: HWND, r: &Resources) -> Result<(), String> {
    for id in [TEXT, X, Y, WIDTH, HEIGHT] {
        c.edit(hwnd, id, id == TEXT, r.font)?;
    }
    c.combo(hwnd, TARGET, r.font)?;
    c.button(hwnd, APPLY, "Применить", r.font)?;
    for id in LABEL..LABEL + 7 {
        c.label(hwnd, id, "", r.small)?;
    }
    let canvas = c.get(CANVAS);
    for id in BUTTON..BUTTON + LIMIT {
        c.button(canvas, id, "", r.font)?;
    }
    Ok(())
}

/// Preview the selected construction chain, stopping at a clear or ambiguous merge.
/// Runtime state always comes from Player, never from this authoring approximation.
pub fn elements(app: &App) -> Vec<&Scene> {
    if let Some(player) = &app.player {
        return player.interface();
    }
    let mut current = app.scene();
    let mut seen = std::collections::BTreeSet::new();
    let mut result = Vec::new();
    while seen.insert(&current.id) && seen.len() <= 64 {
        if current.kind == NodeKind::UiClear {
            break;
        }
        if current.kind.ui_element() {
            result.push(current);
        }
        let mut previous = app.document.project.scenes.iter().filter(|s| {
            s.kind.interface()
                && s.kind != NodeKind::UiScreen
                && s.next.as_ref() == Some(&current.id)
        });
        let Some(one) = previous.next() else { break };
        if previous.next().is_some() {
            break;
        }
        current = one;
    }
    result.reverse();
    result
}
fn bounds(frame: crate::stage::Rect, ui: &UiElement) -> RECT {
    RECT {
        left: (frame.x + frame.w * ui.x as f32 / 100.) as i32,
        top: (frame.y + frame.h * ui.y as f32 / 100.) as i32,
        right: (frame.x + frame.w * (u16::from(ui.x) + u16::from(ui.width)) as f32 / 100.) as i32,
        bottom: (frame.y + frame.h * (u16::from(ui.y) + u16::from(ui.height)) as f32 / 100.) as i32,
    }
}
fn caption(app: &App, ui: &UiElement) -> String {
    let variables = app
        .player
        .as_ref()
        .map(|p| p.variables())
        .unwrap_or(&app.document.project.variables);
    ui.render_text(variables)
}
pub unsafe fn layout(app: &App) {
    for id in TEXT..=APPLY {
        app.controls.show(id, false);
    }
    for id in LABEL..LABEL + 7 {
        app.controls.show(id, false);
    }
    for id in BUTTON..BUTTON + LIMIT {
        app.controls.show(id, false);
    }
    if IsWindowVisible(app.controls.get(CANVAS)) != 0 && app.game.page.is_none() && !app.reader.log
    {
        let mut rect: RECT = std::mem::zeroed();
        GetClientRect(app.controls.get(CANVAS), &mut rect);
        let f = crate::stage::Rect::frame(rect.right as f32, rect.bottom as f32);
        for (i, scene) in elements(app)
            .into_iter()
            .filter(|s| s.kind == NodeKind::UiButton)
            .take(LIMIT as usize)
            .enumerate()
        {
            if let Some(ui) = &scene.ui {
                let id = BUTTON + i as u16;
                let r = bounds(f, ui);
                app.controls.text(id, &caption(app, ui));
                app.controls
                    .place(id, r.left, r.top, r.right - r.left, r.bottom - r.top, 1.);
                app.controls.show(id, true);
            }
        }
    }
    if app.player.is_some() || app.studio.pane != studio::Pane::Interface {
        return;
    }
    let mut rect: RECT = std::mem::zeroed();
    GetClientRect(app.hwnd, &mut rect);
    let (_, right, _, iw) = app.panel_geometry((rect.right as f32 / app.scale) as i32);
    let x = right + 16;
    let place = |id, x, y, w, h| {
        app.controls.show(id, true);
        app.controls.place(id, x, y, w, h, app.scale);
    };
    let label = |n, y, text: &str| {
        app.controls.text(LABEL + n, app.t(text));
        place(LABEL + n, x, y, iw, 20);
    };
    label(0, 184, "Тип узла");
    place(studio::KIND, x, 206, iw, 330);
    if app.scene().kind.ui_element() {
        label(1, 250, "Текст интерфейса");
        place(TEXT, x, 274, iw, 74);
        label(2, 360, "Позиция X / Y (%)");
        place(X, x, 384, (iw - 8) / 2, 32);
        place(Y, x + (iw + 8) / 2, 384, (iw - 8) / 2, 32);
        label(3, 428, "Ширина / высота (%)");
        place(WIDTH, x, 452, (iw - 8) / 2, 32);
        place(HEIGHT, x + (iw + 8) / 2, 452, (iw - 8) / 2, 32);
        if app.scene().kind == NodeKind::UiButton {
            label(4, 496, "Нажатие");
            place(TARGET, x, 520, iw, 220);
        }
        place(APPLY, x, 568, iw, 32);
    } else {
        label(
            1,
            258,
            if app.scene().kind == NodeKind::UiClear {
                "Убирает все элементы экрана"
            } else {
                "Ожидает нажатия кнопки"
            },
        );
    }
    let h = (rect.bottom as f32 / app.scale) as i32;
    label(5, h - 100, "После сцены");
    place(NEXT_SCENE, x, h - 76, iw, 220);
}
pub unsafe fn refresh(app: &App) {
    let ui = app.scene().ui.clone().unwrap_or_default();
    app.controls.text(TEXT, &ui.text);
    app.controls.text(APPLY, app.t("Применить"));
    for (id, value) in [(X, ui.x), (Y, ui.y), (WIDTH, ui.width), (HEIGHT, ui.height)] {
        app.controls.text(id, &value.to_string());
    }
    let mut targets = vec![app.t("Без перехода").to_owned()];
    targets.extend(app.document.project.scenes.iter().map(|s| s.name.clone()));
    let index = ui
        .target
        .as_ref()
        .and_then(|id| app.document.project.scenes.iter().position(|s| &s.id == id))
        .map(|i| i + 1)
        .unwrap_or(0);
    app.controls.fill_list(TARGET, &targets, index, true);
}
pub unsafe fn command(app: &mut App, id: u16, notification: u16) -> Result<bool, String> {
    if (BUTTON..BUTTON + LIMIT).contains(&id) {
        if notification != BN_CLICKED as u16 || app.game.page.is_some() || app.reader.log {
            return Ok(true);
        }
        let target = elements(app)
            .into_iter()
            .filter(|s| s.kind == NodeKind::UiButton)
            .nth((id - BUTTON) as usize)
            .map(|s| s.id.clone());
        if let Some(target) = target {
            if let Some(player) = &mut app.player {
                player.click(&target)?;
            } else {
                app.selected_scene = app
                    .document
                    .project
                    .scenes
                    .iter()
                    .position(|s| s.id == target)
                    .unwrap();
                app.studio.pane = studio::Pane::Interface;
            }
            app.refresh();
            if app.player.is_some() && IsWindowVisible(app.controls.get(id)) == 0 {
                SetFocus(app.controls.get(ADVANCE));
            }
        }
        return Ok(true);
    }
    if !(TEXT..=APPLY).contains(&id) {
        return Ok(false);
    }
    if id != APPLY || notification != BN_CLICKED as u16 {
        return Ok(true);
    }
    let number = |id| {
        app.controls
            .read(id)
            .trim()
            .parse::<u8>()
            .map_err(|_| "Введите целое число от 0 до 100.".to_owned())
    };
    let (x, y, width, height) = (number(X)?, number(Y)?, number(WIDTH)?, number(HEIGHT)?);
    if width == 0
        || height == 0
        || u16::from(x) + u16::from(width) > 100
        || u16::from(y) + u16::from(height) > 100
    {
        return Err("Элемент интерфейса должен находиться внутри экрана (0…100%).".into());
    }
    let text = app.controls.read(TEXT);
    if text.chars().count() > 4000
        || (app.scene().kind != NodeKind::UiPanel && text.trim().is_empty())
    {
        return Err("Введите текст элемента интерфейса (до 4000 символов).".into());
    }
    let target = app
        .controls
        .selection(TARGET, true)
        .and_then(|n| n.checked_sub(1))
        .and_then(|n| app.document.project.scenes.get(n))
        .map(|s| s.id.clone());
    let i = app.selected_scene;
    app.document.change(|p| {
        p.scenes[i].ui = Some(UiElement {
            x,
            y,
            width,
            height,
            text,
            target,
        })
    });
    app.navigation.changed();
    app.refresh();
    Ok(true)
}
pub unsafe fn paint(app: &App, dc: HDC, rect: RECT) {
    let frame = crate::stage::Rect::frame(rect.right as f32, rect.bottom as f32);
    for scene in elements(app) {
        let Some(ui) = &scene.ui else { continue };
        let mut r = bounds(frame, ui);
        if scene.kind == NodeKind::UiPanel {
            ui::rounded(
                dc,
                r,
                app.palette.surface,
                app.palette.border,
                (4. * app.scale) as i32,
            );
        } else if scene.kind == NodeKind::UiText {
            InflateRect(&mut r, -(6. * app.scale) as i32, -(4. * app.scale) as i32);
            ui::text(
                dc,
                &caption(app, ui),
                r,
                app.palette.text,
                app.resources.dialogue,
                DT_WORDBREAK | DT_NOPREFIX,
            );
        }
    }
}
