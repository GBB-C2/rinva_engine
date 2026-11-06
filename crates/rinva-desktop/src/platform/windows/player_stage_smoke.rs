//! Native regression checks for scene transforms and player menus, including interrupted gestures.
use super::*;
use rinva_core::saves::{self, Slot};
use std::time::{Duration, Instant};
fn check(value: bool, message: &str) -> Result<(), String> {
    if value {
        Ok(())
    } else {
        Err(message.into())
    }
}
fn handle(id: u16) -> HWND {
    APP.with(|s| s.borrow().as_ref().unwrap().controls.get(id))
}
fn project() -> Project {
    APP.with(|s| s.borrow().as_ref().unwrap().document.project.clone())
}
unsafe fn command(hwnd: HWND, id: u16) {
    SendMessageW(hwnd, WM_COMMAND, id as usize, 0);
    pump();
}
unsafe fn combo(hwnd: HWND, id: u16, index: usize) {
    SendMessageW(handle(id), CB_SETCURSEL, index, 0);
    SendMessageW(
        hwnd,
        WM_COMMAND,
        id as usize | ((CBN_SELENDOK as usize) << 16),
        0,
    );
    pump();
}
unsafe fn pointer(msg: u32, x: f32, y: f32) {
    let packed = (x.round() as i16 as u16 as u32) | ((y.round() as i16 as u16 as u32) << 16);
    SendMessageW(handle(CANVAS), msg, 0, packed as isize);
    pump();
}
unsafe fn settle() -> Result<(), String> {
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        APP.with(|s| {
            let mut s = s.borrow_mut();
            game_ui::tick(s.as_mut().unwrap());
        });
        pump();
        if !APP.with(|s| s.borrow().as_ref().unwrap().game.busy()) {
            return Ok(());
        }
        if Instant::now() > end {
            return Err("Save/scan worker did not finish".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn line() -> String {
    APP.with(|s| {
        s.borrow()
            .as_ref()
            .unwrap()
            .player
            .as_ref()
            .unwrap()
            .line_key()
    })
}
fn sprite_bounds() -> crate::stage::Rect {
    APP.with(|s| {
        let s = s.borrow();
        let a = s.as_ref().unwrap();
        let mut r: RECT = unsafe { std::mem::zeroed() };
        unsafe {
            GetClientRect(a.controls.get(CANVAS), &mut r);
        }
        let f = crate::stage::Rect::frame(r.right as f32, r.bottom as f32);
        let image = a
            .sprites
            .iter()
            .find(|s| s.index == a.studio.sprite)
            .unwrap();
        crate::stage::bounds(
            f,
            &image.placement,
            image.image.width as f32 / image.image.height as f32,
        )
    })
}
pub unsafe fn run(hwnd: HWND, output: &Path) -> Result<(), String> {
    smoke_trace("player/stage: begin");
    settle()?;
    let original = project();
    let old_root = APP.with(|s| s.borrow().as_ref().unwrap().asset_root.clone());
    let root = output.join("player-stage-fixture");
    studio_smoke::assets(&root)?;
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let mut p = studio_smoke::fixture();
    p.id = format!("player-stage-{}", std::process::id());
    p.title = "На развилке · Rinva".into();
    p.scenes[0].audio = None;
    let id = p.id.clone();
    APP.with(|s| {
        let mut s = s.borrow_mut();
        let a = s.as_mut().unwrap();
        a.game = game_ui::Game::default();
        a.player = None;
        a.document = Document::new(p);
        a.asset_root = root;
        a.selected_scene = 0;
        a.selected_item = 0;
        a.choices_tab = false;
        a.graph_mode = false;
        a.navigation.mode = None;
        a.studio.pane = studio::Pane::Characters;
        a.studio.sprite = 0;
        a.reader.settings.speed = 0;
        a.language = Language::Russian;
        a.dark = true;
        a.palette = Palette::new(true);
        a.rebuild_resources();
        a.image_key.clear();
        a.refresh();
    });
    pump();
    let before = project();
    let r = sprite_bounds();
    let start = (r.x + r.w / 2., r.y + r.h / 2.);
    pointer(WM_LBUTTONDOWN, start.0, start.1);
    pointer(WM_MOUSEMOVE, start.0 + 45., start.1 - 20.);
    check(
        project() == before,
        "Stage preview mutated the document before commit",
    )?;
    SendMessageW(handle(CANVAS), WM_KEYDOWN, VK_ESCAPE as usize, 0);
    pump();
    check(project() == before, "Escape committed a stage preview")?;
    check(
        GetCapture().is_null(),
        "Stage capture remained after Escape",
    )?;
    pointer(WM_LBUTTONDOWN, start.0, start.1);
    pointer(WM_MOUSEMOVE, start.0 + 45., start.1 - 20.);
    pointer(WM_LBUTTONUP, start.0 + 45., start.1 - 20.);
    let moved = project();
    check(moved != before, "Stage drag did not commit")?;
    command(hwnd, UNDO);
    check(
        project() == before,
        "Stage drag was not one undo transaction",
    )?;
    command(hwnd, REDO);
    check(project() == moved, "Stage redo lost transform")?;
    let r = sprite_bounds();
    let start = (r.x + r.w / 2., r.y + r.h / 2.);
    pointer(WM_LBUTTONDOWN, start.0, start.1);
    pointer(WM_MOUSEMOVE, start.0 - 30., start.1 + 15.);
    ReleaseCapture();
    pump();
    check(project() == moved, "Lost capture committed a stage preview")?;
    let r = sprite_bounds();
    pointer(WM_LBUTTONDOWN, r.x + r.w, r.y);
    pointer(WM_MOUSEMOVE, r.x + r.w + 10., r.y - 20.);
    pointer(WM_LBUTTONUP, r.x + r.w + 10., r.y - 20.);
    check(
        project().scenes[0].sprites[0].scale != moved.scenes[0].sprites[0].scale,
        "Scale handle did not resize sprite",
    )?;
    command(hwnd, UNDO);
    check(project() == moved, "Scale undo changed unrelated data")?;
    command(hwnd, stage_ui::GUIDES);
    command(hwnd, stage_ui::FRONT);
    paint::capture(hwnd, &output.join("stage-drag-ru.png"))?;
    command(hwnd, THEME);
    paint::capture(hwnd, &output.join("stage-light.png"))?;
    command(hwnd, THEME);
    // Keyboard editing uses the same model as numeric inspector fields.
    SetFocus(handle(CANVAS));
    SendMessageW(handle(CANVAS), WM_KEYDOWN, VK_RIGHT as usize, 0);
    pump();
    let x = project().scenes[0].sprites[0].x;
    check(
        ui::window_text(handle(studio::SPRITE_X)) == x.to_string(),
        "Stage keyboard and inspector disagree",
    )?;
    command(hwnd, PLAY);
    let first = line();
    command(hwnd, game_ui::MENU);
    settle()?;
    let at_menu = line();
    let probe = APP.with(|s| s.borrow().as_ref().unwrap().audio.probe())?;
    check(
        probe.contains("paused=true"),
        "Audio worker ignored menu pause",
    )?;
    APP.with(|s| {
        let mut s = s.borrow_mut();
        let a = s.as_mut().unwrap();
        a.reader.auto = true;
        a.reader.reveal("all");
        a.reader.completed = Some(Instant::now() - Duration::from_secs(20));
        reader_ui::tick(a);
    });
    pump();
    check(line() == at_menu, "Auto advanced through an open game menu")?;
    APP.with(|s| s.borrow_mut().as_mut().unwrap().reader.auto = false);
    paint::capture(hwnd, &output.join("player-menu-ru.png"))?;
    command(hwnd, game_ui::SAVE_PAGE);
    settle()?;
    command(hwnd, game_ui::APPLY);
    settle()?;
    let record = saves::read(&game_ui::root(), &id, Slot::Manual(1))?
        .ok_or("Manual slot was not written")?;
    check(!record.thumbnail.is_empty(), "Slot has no thumbnail")?;
    let image = image::load_from_memory(&record.thumbnail).map_err(|e| e.to_string())?;
    check(
        image.width() == 320 && image.height() == 180,
        "Slot thumbnail dimensions are wrong",
    )?;
    paint::capture(hwnd, &output.join("player-slots-ru.png"))?;
    command(hwnd, game_ui::RESUME);
    command(hwnd, GAME_SAVE);
    settle()?;
    command(hwnd, ADVANCE);
    check(line() != first, "Dialogue did not advance after menu")?;
    command(hwnd, GAME_LOAD);
    check(line() == first, "Quick load did not restore progress")?;
    // A completed background save must not move the user's highlighted choice.
    for _ in 0..8 {
        let choice = APP.with(|s| {
            matches!(
                s.borrow()
                    .as_ref()
                    .unwrap()
                    .player
                    .as_ref()
                    .unwrap()
                    .frame(),
                Frame::Choices { .. }
            )
        });
        if choice {
            break;
        }
        command(hwnd, ADVANCE);
    }
    check(
        APP.with(|s| {
            matches!(
                s.borrow()
                    .as_ref()
                    .unwrap()
                    .player
                    .as_ref()
                    .unwrap()
                    .frame(),
                Frame::Choices { .. }
            )
        }),
        "Choice fixture was not reached",
    )?;
    SendMessageW(handle(GAME_CHOICES), LB_SETCURSEL, 1, 0);
    APP.with(|s| game_ui::save_slot(s.borrow_mut().as_mut().unwrap(), Slot::Auto))?;
    settle()?;
    check(
        SendMessageW(handle(GAME_CHOICES), LB_GETCURSEL, 0, 0) == 1,
        "Background save changed highlighted choice",
    )?;
    command(hwnd, GAME_LOAD);
    // Auto uses its own atomic record and does not touch a manual slot.
    APP.with(|s| game_ui::save_slot(s.borrow_mut().as_mut().unwrap(), Slot::Auto))?;
    settle()?;
    check(
        saves::read(&game_ui::root(), &id, Slot::Auto)?.is_some(),
        "Auto slot was not written",
    )?;
    check(
        saves::read(&game_ui::root(), &id, Slot::Manual(1))?
            .unwrap()
            .saved_at
            == record.saved_at,
        "Auto overwrote a manual save",
    )?;
    command(hwnd, game_ui::MENU);
    command(hwnd, game_ui::OPTIONS);
    combo(hwnd, game_ui::STYLE, 1);
    combo(hwnd, game_ui::TEXT_SIZE, 2);
    let saved: Settings = storage::read_json(&settings_dir().join("settings.json"))?;
    check(
        saved.reading.dialog_style == 1 && saved.reading.text_size == 2,
        "Reader appearance was not persisted",
    )?;
    paint::capture(hwnd, &output.join("player-settings-ru.png"))?;
    combo(hwnd, LANGUAGE, Language::Japanese.index());
    command(hwnd, THEME);
    paint::capture(hwnd, &output.join("player-settings-ja-light.png"))?;
    command(hwnd, THEME);
    combo(hwnd, LANGUAGE, Language::Russian.index());
    command(hwnd, game_ui::RESUME);
    paint::capture(hwnd, &output.join("player-dialogue-light.png"))?;
    let mut old_rect: RECT = std::mem::zeroed();
    GetWindowRect(hwnd, &mut old_rect);
    command(hwnd, game_ui::FULLSCREEN);
    check(
        GetWindowLongPtrW(hwnd, GWL_STYLE) as u32 & WS_CAPTION == 0,
        "Fullscreen retained its caption",
    )?;
    let mut monitor: MONITORINFO = std::mem::zeroed();
    monitor.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
    GetMonitorInfoW(
        MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST),
        &mut monitor,
    );
    let mut full: RECT = std::mem::zeroed();
    GetWindowRect(hwnd, &mut full);
    check(
        (full.left, full.top, full.right, full.bottom)
            == (
                monitor.rcMonitor.left,
                monitor.rcMonitor.top,
                monitor.rcMonitor.right,
                monitor.rcMonitor.bottom,
            ),
        "Fullscreen does not fit its monitor",
    )?;
    command(hwnd, game_ui::FULLSCREEN);
    let mut restored: RECT = std::mem::zeroed();
    GetWindowRect(hwnd, &mut restored);
    check(
        (restored.left, restored.top, restored.right, restored.bottom)
            == (old_rect.left, old_rect.top, old_rect.right, old_rect.bottom),
        "Fullscreen did not restore window geometry",
    )?;
    // An incompatible save is shown but never applied to the live player.
    let mut invalid = record.clone();
    invalid.signature = "changed".into();
    saves::write(&game_ui::root(), Slot::Manual(2), &invalid)?;
    command(hwnd, game_ui::MENU);
    command(hwnd, game_ui::LOAD_PAGE);
    settle()?;
    SendMessageW(handle(game_ui::SLOTS), LB_SETCURSEL, 3, 0);
    SendMessageW(
        hwnd,
        WM_COMMAND,
        game_ui::SLOTS as usize | ((LBN_SELCHANGE as usize) << 16),
        0,
    );
    pump();
    check(
        IsWindowEnabled(handle(game_ui::APPLY)) == 0,
        "Incompatible save remained loadable",
    )?;
    // Errors are local to one slot; the successful quick/manual slots remain readable.
    let broken = saves::path(&game_ui::root(), &id, Slot::Manual(6))?;
    std::fs::create_dir(&broken).map_err(|e| e.to_string())?;
    APP.with(|s| game_ui::save_slot(s.borrow_mut().as_mut().unwrap(), Slot::Manual(6)))?;
    settle()?;
    check(
        APP.with(|s| {
            s.borrow()
                .as_ref()
                .unwrap()
                .status
                .contains("Не удалось сохранить")
        }),
        "Failed write was reported as success",
    )?;
    check(
        saves::read(&game_ui::root(), &id, Slot::Quick)?.is_some(),
        "Write failure destroyed another slot",
    )?;
    command(hwnd, game_ui::RESUME);
    command(hwnd, STOP);
    settle()?;
    storage::export_bundle(
        &project(),
        &APP.with(|s| s.borrow().as_ref().unwrap().root()),
        &output.join("standalone"),
        &std::env::current_exe().map_err(|e| e.to_string())?,
    )?;
    APP.with(|s| {
        let mut s = s.borrow_mut();
        let a = s.as_mut().unwrap();
        a.game = game_ui::Game::default();
        a.audio.stop();
        a.reader.reset();
        a.reader.settings.dialog_style = 0;
        a.reader.settings.text_size = 0;
        a.reader.settings.speed = 0;
        a.document = Document::new(original);
        a.asset_root = old_root;
        a.studio = studio::Studio::default();
        a.stage = stage_ui::Stage::default();
        a.selected_scene = 0;
        a.selected_item = 0;
        a.graph_mode = false;
        a.language = Language::Russian;
        a.image_key.clear();
        a.rebuild_resources();
        a.refresh();
    });
    pump();
    std::fs::write(output.join("player-stage.txt"),"PASS: stage preview/cancel/capture loss, atomic transform undo/redo, scaling, keyboard inspector sync, layers/guides, paused menu, manual/quick/auto slots and PNG thumbnails, appearance persistence, fullscreen restore, incompatible slot isolation, failed save isolation.\n").map_err(|e|e.to_string())?;
    smoke_trace("player/stage: passed");
    Ok(())
}

/// Executed in the exported RinvaPlayer.exe, with its own isolated settings/save directory.
pub unsafe fn standalone(hwnd: HWND, output: &Path) -> Result<(), String> {
    settle()?;
    check(
        APP.with(|s| {
            let s = s.borrow();
            let a = s.as_ref().unwrap();
            a.player_only && !a.game.session && a.game.page == Some(game_ui::Page::Home)
        }),
        "Standalone did not open at its title menu",
    )?;
    paint::capture(hwnd, &output.join("standalone-title.png"))?;
    command(hwnd, game_ui::NEW_GAME);
    check(
        APP.with(|s| {
            let s = s.borrow();
            let a = s.as_ref().unwrap();
            a.game.session && a.game.page.is_none()
        }),
        "Standalone new game failed",
    )?;
    command(hwnd, ADVANCE);
    command(hwnd, ADVANCE);
    let position = line();
    command(hwnd, GAME_SAVE);
    settle()?;
    command(hwnd, ADVANCE);
    command(hwnd, GAME_LOAD);
    check(line() == position, "Standalone quick-save roundtrip failed")?;
    APP.with(|s| game_ui::save_slot(s.borrow_mut().as_mut().unwrap(), Slot::Auto))?;
    settle()?;
    command(hwnd, game_ui::MENU);
    paint::capture(hwnd, &output.join("standalone-menu.png"))?;
    std::fs::write(output.join("native-player.txt"),"PASS: exported RinvaPlayer.exe starts at its title, starts a story, quick-saves/loads and autosaves, and opens the pause menu.\n").map_err(|e|e.to_string())?;
    Ok(())
}
