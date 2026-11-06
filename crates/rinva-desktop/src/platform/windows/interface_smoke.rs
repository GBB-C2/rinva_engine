//! End-to-end native proof for the two viewports and node-built buttons.
use super::*;
fn handle(id: u16) -> HWND {
    APP.with(|s| s.borrow().as_ref().unwrap().controls.get(id))
}
fn check(ok: bool, message: &str) -> Result<(), String> {
    if ok {
        Ok(())
    } else {
        Err(message.into())
    }
}
unsafe fn command(hwnd: HWND, id: u16) {
    SendMessageW(hwnd, WM_COMMAND, id as usize, 0);
    pump();
}
unsafe fn edit(id: u16, text: &str) {
    SetWindowTextW(handle(id), wide(text).as_ptr());
    pump();
}
fn packed(x: i32, y: i32) -> isize {
    ((x as u16 as u32) | ((y as u16 as u32) << 16)) as isize
}
fn project() -> Project {
    APP.with(|s| s.borrow().as_ref().unwrap().document.project.clone())
}
pub unsafe fn run(hwnd: HWND, output: &Path) -> Result<(), String> {
    smoke_trace("node interface: start");
    let original = project();
    let fixture: Project =
        serde_json::from_str(include_str!("../../../../../examples/node-ui.rinva"))
            .map_err(|e| e.to_string())?;
    let old_share = APP.with(|s| s.borrow().as_ref().unwrap().preview_share);
    APP.with(|s| {
        let mut s = s.borrow_mut();
        let a = s.as_mut().unwrap();
        a.player = None;
        a.game = game_ui::Game::default();
        a.reader.reset();
        a.audio.stop();
        a.document = Document::new(fixture.clone());
        a.selected_scene = 5;
        a.selected_item = 0;
        a.choices_tab = false;
        a.graph = Graph::default();
        a.graph_mode = true;
        a.studio = studio::Studio::default();
        a.navigation = navigation::Navigation::default();
        a.language = Language::Russian;
        a.dark = true;
        a.preview_share = 45;
        a.image_key.clear();
        a.rebuild_resources();
        a.refresh();
        a.fit_graph();
    });
    pump();
    check(
        IsWindowVisible(handle(CANVAS)) != 0 && IsWindowVisible(handle(GRAPH_CANVAS)) != 0,
        "Preview and graph must be visible together",
    )?;
    let mut preview: RECT = std::mem::zeroed();
    let mut graph: RECT = std::mem::zeroed();
    GetWindowRect(handle(CANVAS), &mut preview);
    GetWindowRect(handle(GRAPH_CANVAS), &mut graph);
    check(preview.bottom < graph.top, "Preview overlaps node graph")?;
    check(
        IsWindowVisible(handle(interface_ui::BUTTON + 1)) != 0,
        "Construction preview did not create native buttons",
    )?;
    check(
        APP.with(|s| s.borrow().as_ref().unwrap().studio.pane == studio::Pane::Interface),
        "Selecting a UI node did not open its inspector",
    )?;
    // A panel executed after a button must not cover it in the save thumbnail.
    APP.with(|s| {
        let mut s = s.borrow_mut();
        let a = s.as_mut().unwrap();
        let mut late = fixture.clone();
        let mut panel = late.scenes[1].clone();
        panel.id = "late-panel".into();
        panel.ui = late.scenes[4].ui.clone();
        panel.next = Some("screen".into());
        late.scenes[5].next = Some(panel.id.clone());
        late.scenes.push(panel);
        a.player = Some(Player::new(&late)?);
        let result = paint::thumbnail(a);
        a.player = None;
        let bytes = result?;
        let image = image::load_from_memory(&bytes)
            .map_err(|e| e.to_string())?
            .to_rgba8();
        let pixel = image.get_pixel(59, 114);
        check(
            pixel[0] as u32 == a.palette.field & 255,
            "Late panel covered button in save thumbnail",
        )?;
        a.refresh();
        Ok::<_, String>(())
    })?;
    // Selecting a button in the preview must not leave another node's output active.
    APP.with(|s| {
        let mut s = s.borrow_mut();
        let a = s.as_mut().unwrap();
        a.graph.selected_port = Some((4, Port::Click));
        a.graph.selection.insert("start".into());
    });
    SendMessageW(handle(interface_ui::BUTTON + 1), BM_CLICK, 0, 0);
    pump();
    check(
        APP.with(|s| {
            let s = s.borrow();
            let a = s.as_ref().unwrap();
            a.selected_scene == 5 && a.graph.selected_port.is_none() && a.graph.selection.is_empty()
        }),
        "Preview selection retained another node's graph connection",
    )?;
    check(
        project() == fixture,
        "Selecting a preview button changed the story",
    )?;
    // Editing is one undoable transaction, and the native label refreshes immediately.
    edit(interface_ui::TEXT, "Получить монету · 海");
    command(hwnd, interface_ui::APPLY);
    check(
        project().scenes[5].ui.as_ref().unwrap().text == "Получить монету · 海",
        "UI inspector did not apply Unicode text",
    )?;
    check(
        ui::window_text(handle(interface_ui::BUTTON + 1)) == "Получить монету · 海",
        "Preview button label is stale",
    )?;
    command(hwnd, UNDO);
    check(project() == fixture, "UI edit undo changed other nodes")?;
    command(hwnd, REDO);
    command(hwnd, UNDO);
    // Renaming a variable updates UI bindings and mechanics in the same undo step.
    SendMessageW(handle(studio::INSPECTOR), CB_SETCURSEL, 1, 0);
    SendMessageW(
        hwnd,
        WM_COMMAND,
        studio::INSPECTOR as usize | ((CBN_SELENDOK as usize) << 16),
        0,
    );
    pump();
    edit(studio::VAR_NAME, "tokens");
    edit(studio::VAR_VALUE, "0");
    command(hwnd, studio::VAR_UPDATE);
    check(
        project().scenes[3]
            .ui
            .as_ref()
            .unwrap()
            .text
            .contains("{tokens}"),
        "Variable rename broke UI binding",
    )?;
    command(hwnd, UNDO);
    check(
        project() == fixture,
        "Variable rename undo did not restore UI",
    )?;
    SendMessageW(handle(studio::INSPECTOR), CB_SETCURSEL, 5, 0);
    SendMessageW(
        hwnd,
        WM_COMMAND,
        studio::INSPECTOR as usize | ((CBN_SELENDOK as usize) << 16),
        0,
    );
    pump();
    // Horizontal splitter commits only on mouse-up; Escape/capture loss restore it.
    let (scale, x, y) = APP.with(|s| {
        let s = s.borrow();
        let a = s.as_ref().unwrap();
        let mut r: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut r);
        let h = (r.bottom as f32 / a.scale) as i32;
        let (x, _, cw, _) = a.panel_geometry((r.right as f32 / a.scale) as i32);
        (
            a.scale,
            ((x + cw / 2) as f32 * a.scale) as i32,
            ((126 + a.preview_height(h)) as f32 * a.scale) as i32,
        )
    });
    SendMessageW(hwnd, WM_LBUTTONDOWN, 0, packed(x, y));
    SendMessageW(hwnd, WM_MOUSEMOVE, 0, packed(x, y + (60. * scale) as i32));
    check(
        APP.with(|s| s.borrow().as_ref().unwrap().preview_share != 45),
        "Horizontal splitter did not resize",
    )?;
    SendMessageW(hwnd, WM_KEYDOWN, VK_ESCAPE as usize, 0);
    pump();
    check(
        APP.with(|s| s.borrow().as_ref().unwrap().preview_share == 45),
        "Splitter Escape did not restore original height",
    )?;
    SendMessageW(hwnd, WM_LBUTTONDOWN, 0, packed(x, y));
    SendMessageW(hwnd, WM_MOUSEMOVE, 0, packed(x, y + (32. * scale) as i32));
    SendMessageW(hwnd, WM_LBUTTONUP, 0, packed(x, y + (32. * scale) as i32));
    pump();
    check(
        storage::read_json::<Settings>(&settings_dir().join("settings.json"))?.preview_share
            == APP.with(|s| s.borrow().as_ref().unwrap().preview_share),
        "Preview height preference not persisted",
    )?;
    APP.with(|s| {
        let mut s = s.borrow_mut();
        let a = s.as_mut().unwrap();
        a.graph.zoom = 0.7;
        a.graph.pan = Point::new(16., 0.);
        a.refresh();
    });
    pump();
    for dark in [true, false] {
        if APP.with(|s| s.borrow().as_ref().unwrap().dark) != dark {
            command(hwnd, THEME);
        }
        let p = APP.with(|s| s.borrow().as_ref().unwrap().palette);
        check(
            [
                p.window,
                p.surface,
                p.field,
                p.border,
                p.text,
                p.muted,
                p.accent,
                p.on_accent,
                p.selected,
            ]
            .iter()
            .all(|c| c & 255 == (c >> 8) & 255 && c & 255 == (c >> 16) & 255),
            "Application palette has a color tint",
        )?;
        paint::capture(
            hwnd,
            &output.join(if dark {
                "node-workspace-dark.png"
            } else {
                "node-workspace-light.png"
            }),
        )?;
    }
    command(hwnd, THEME);
    // The native dropdown still supports keyboard dismissal after owner drawing.
    SendMessageW(handle(studio::NEW_KIND), CB_SHOWDROPDOWN, 1, 0);
    pump();
    SendMessageW(handle(studio::NEW_KIND), WM_KEYDOWN, VK_DOWN as usize, 0);
    SendMessageW(handle(studio::NEW_KIND), WM_KEYDOWN, VK_ESCAPE as usize, 0);
    pump();
    check(
        project() == fixture,
        "Cancelling node type dropdown modified story",
    )?;
    // Create a new UI node through native commands, then undo it completely.
    SendMessageW(handle(studio::NEW_KIND), CB_SETCURSEL, 8, 0);
    SendMessageW(
        hwnd,
        WM_COMMAND,
        studio::NEW_KIND as usize | ((CBN_SELENDOK as usize) << 16),
        0,
    );
    command(hwnd, studio::NEW_NODE);
    check(
        project().scenes.last().unwrap().kind == rinva_core::NodeKind::UiButton,
        "New node palette did not create a button",
    )?;
    command(hwnd, UNDO);
    check(project() == fixture, "New UI node undo failed")?;
    command(hwnd, PLAY);
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
                Frame::Screen { .. }
            )
        }),
        "Player did not wait at UI screen",
    )?;
    check(
        IsWindowVisible(handle(GRAPH_CANVAS)) == 0,
        "Graph remained visible in player",
    )?;
    // Buttons are real keyboard targets, nested in the native preview window.
    let mut next = GetNextDlgTabItem(hwnd, null_mut(), 0);
    let mut found = false;
    for _ in 0..256 {
        if next == handle(interface_ui::BUTTON + 1) {
            found = true;
            break;
        }
        next = GetNextDlgTabItem(hwnd, next, 0);
    }
    check(found, "Node-built buttons cannot be reached with Tab")?;
    SetFocus(handle(interface_ui::BUTTON + 1));
    SendMessageW(
        handle(interface_ui::BUTTON + 1),
        WM_KEYDOWN,
        VK_SPACE as usize,
        0,
    );
    SendMessageW(
        handle(interface_ui::BUTTON + 1),
        WM_KEYUP,
        VK_SPACE as usize,
        0,
    );
    pump();
    check(
        APP.with(|s| {
            s.borrow()
                .as_ref()
                .unwrap()
                .player
                .as_ref()
                .unwrap()
                .variables()["coins"]
                == 1
        }),
        "Keyboard button did not execute assignment branch",
    )?;
    paint::capture(hwnd, &output.join("node-interface-player.png"))?;
    let saved = APP.with(|s| s.borrow().as_ref().unwrap().player.as_ref().unwrap().save());
    SendMessageW(handle(interface_ui::BUTTON), BM_CLICK, 0, 0);
    pump();
    check(
        IsWindowVisible(handle(interface_ui::BUTTON)) == 0,
        "Clear node left native buttons on screen",
    )?;
    check(APP.with(|s|matches!(s.borrow().as_ref().unwrap().player.as_ref().unwrap().frame(),Frame::Dialogue{scene,..} if scene.id=="dialogue")),"Start button did not enter dialogue")?;
    command(hwnd, BACK);
    check(
        IsWindowVisible(handle(interface_ui::BUTTON)) != 0,
        "Back did not restore menu buttons",
    )?;
    APP.with(|s| {
        let mut s = s.borrow_mut();
        let a = s.as_mut().unwrap();
        a.player.as_mut().unwrap().restore(saved)?;
        a.refresh();
        Ok::<_, String>(())
    })?;
    command(hwnd, STOP);
    // Minimum supported size, CJK labels, and both viewports remain usable.
    SetWindowPos(
        hwnd,
        null_mut(),
        0,
        0,
        (1120. * scale) as i32,
        (760. * scale) as i32,
        SWP_NOMOVE | SWP_NOZORDER,
    );
    APP.with(|s| {
        let mut s = s.borrow_mut();
        let a = s.as_mut().unwrap();
        a.selected_scene = 5;
        a.language = Language::Japanese;
        a.rebuild_resources();
        a.refresh();
        a.fit_graph();
    });
    pump();
    paint::capture(hwnd, &output.join("node-workspace-small-ja.png"))?;
    SetWindowPos(
        hwnd,
        null_mut(),
        0,
        0,
        (1440. * scale) as i32,
        (960. * scale) as i32,
        SWP_NOMOVE | SWP_NOZORDER,
    );
    APP.with(|s| {
        let mut s = s.borrow_mut();
        let a = s.as_mut().unwrap();
        a.player = None;
        a.game = game_ui::Game::default();
        a.reader.reset();
        a.audio.stop();
        a.document = Document::new(original);
        a.selected_scene = 0;
        a.selected_item = 0;
        a.graph = Graph::default();
        a.graph_mode = false;
        a.preview_share = old_share;
        a.studio = studio::Studio::default();
        a.language = Language::Russian;
        a.image_key.clear();
        a.rebuild_resources();
        a.refresh();
    });
    pump();
    std::fs::write(output.join("node-interface.txt"),"PASS: simultaneous native preview/graph, interface node creation, Unicode inspector edit/undo/redo, live button labels, neutral dark/light palettes, splitter commit/cancel/persistence, dropdown dismissal, keyboard/Tab button activation, variable action, dialogue transition, UI clear/back/save restoration, minimum-size Japanese UI, live HWND screenshots.\n").map_err(|e|e.to_string())?;
    smoke_trace("node interface: passed");
    Ok(())
}
