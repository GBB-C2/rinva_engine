//! End-to-end checks send real Win32 input messages through the native callbacks.
use super::*;
unsafe fn command(hwnd: HWND, id: u16) {
    SendMessageW(hwnd, WM_COMMAND, id as usize, 0);
    pump();
}
fn project() -> Project {
    APP.with(|slot| slot.borrow().as_ref().unwrap().document.project.clone())
}
fn handle(id: u16) -> HWND {
    APP.with(|slot| slot.borrow().as_ref().unwrap().controls.get(id))
}
fn check(ok: bool, message: &str) -> Result<(), String> {
    if ok {
        Ok(())
    } else {
        Err(message.into())
    }
}
fn packed(x: i32, y: i32) -> isize {
    ((x as u16 as u32) | ((y as u16 as u32) << 16)) as isize
}
/// A screenshot must contain rendered content, not just the controls' backgrounds.
/// Check real child client rectangles after theme changes; story text remains native.
pub unsafe fn verify_preview_capture(hwnd: HWND, path: &Path) -> Result<(), String> {
    let image = image::open(path).map_err(|e| e.to_string())?.into_rgb8();
    // Child windows must appear at their real client positions, even though
    // WM_PRINT also traverses the top-level window's nonclient frame.
    let mut button: RECT = std::mem::zeroed();
    GetWindowRect(handle(SAVE), &mut button);
    let mut point = POINT {
        x: button.left + 6,
        y: button.top + 6,
    };
    ScreenToClient(hwnd, &mut point);
    let palette = APP.with(|s| s.borrow().as_ref().unwrap().palette);
    let rgb = |c: u32| [c as u8, (c >> 8) as u8, (c >> 16) as u8];
    let pixel = image.get_pixel(point.x as u32, point.y as u32).0;
    check(
        pixel == rgb(palette.field) || pixel == rgb(palette.selected),
        "Screenshot shifted child controls away from their client positions",
    )?;
    for id in [CANVAS, BODY] {
        let child = handle(id);
        let mut rect: RECT = std::mem::zeroed();
        GetClientRect(child, &mut rect);
        let mut origin = POINT { x: 0, y: 0 };
        ClientToScreen(child, &mut origin);
        ScreenToClient(hwnd, &mut origin);
        let mut colors = std::collections::BTreeSet::new();
        for y in 10..rect.bottom.saturating_sub(10) {
            for x in 10..rect.right.saturating_sub(10) {
                let (x, y) = ((origin.x + x) as u32, (origin.y + y) as u32);
                if x < image.width() && y < image.height() {
                    colors.insert(image.get_pixel(x, y).0);
                }
            }
        }
        check(
            colors.len() > 12,
            &format!("Screenshot omitted content of control {id}"),
        )?;
    }
    check(
        ui::window_text(handle(BODY)) == project().scenes[0].lines[0].text,
        "Theme change altered native dialogue text",
    )
}
unsafe fn pointer(hwnd: HWND, msg: u32, point: Point) {
    let scale = APP.with(|s| s.borrow().as_ref().unwrap().scale);
    SendMessageW(
        hwnd,
        msg,
        0,
        packed(
            (point.x * scale).round() as i32,
            (point.y * scale).round() as i32,
        ),
    );
    pump();
}
fn node_point(i: usize, port: Option<Port>) -> Point {
    APP.with(|s| {
        let s = s.borrow();
        let a = s.as_ref().unwrap();
        let p = graph::positions(&a.document.project)[i];
        a.graph.screen(if let Some(port) = port {
            graph::output(p, port)
        } else {
            Point::new(p.x + 80., p.y + 24.)
        })
    })
}
unsafe fn language(hwnd: HWND, language: Language) {
    SendMessageW(handle(LANGUAGE), CB_SETCURSEL, language.index(), 0);
    SendMessageW(
        hwnd,
        WM_COMMAND,
        LANGUAGE as usize | ((CBN_SELENDOK as usize) << 16),
        0,
    );
    pump();
}

pub unsafe fn run(hwnd: HWND, output: &Path) -> Result<(), String> {
    let original = project();
    let old_language = APP.with(|s| s.borrow().as_ref().unwrap().language);
    SetWindowPos(
        hwnd,
        null_mut(),
        0,
        0,
        1440,
        960,
        SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
    );
    pump();
    // Rounded hit regions are real native regions, not screenshot-only decoration.
    let region = CreateRectRgn(0, 0, 0, 0);
    GetWindowRgn(handle(SAVE), region);
    let rounded = PtInRegion(region, 0, 0) == 0 && PtInRegion(region, 24, 16) != 0;
    DeleteObject(region);
    check(rounded, "4-DIP control rounding failed")?;
    smoke_trace("language tests");
    for lang in LANGUAGES {
        smoke_trace(lang.name());
        language(hwnd, lang);
        APP.with(|s| fonts::verify_glyphs(s.borrow().as_ref().unwrap().resources.font))?;
        check(
            ui::window_text(handle(SAVE)) == lang.text("Сохранить"),
            "Language label did not update",
        )?;
        check(
            project() == original,
            "Language change modified author text",
        )?;
        check(
            storage::read_json::<Settings>(&settings_dir().join("settings.json"))?.language == lang,
            "Language preference not saved",
        )?;
        command(hwnd, VIEW_NODES);
        UpdateWindow(hwnd);
        pump();
        let file = match lang {
            Language::English => "nodes-en.png",
            Language::Russian => "nodes-ru.png",
            Language::Korean => "nodes-ko.png",
            Language::Japanese => "nodes-ja.png",
            Language::Chinese => "nodes-zh.png",
        };
        paint::capture(hwnd, &output.join(file))?;
    }
    language(hwnd, Language::Russian);
    // Escape in an opened language selector must not commit its highlighted candidate.
    SendMessageW(handle(LANGUAGE), CB_SHOWDROPDOWN, 1, 0);
    pump();
    SendMessageW(handle(LANGUAGE), WM_KEYDOWN, VK_DOWN as usize, 0);
    pump();
    SendMessageW(handle(LANGUAGE), WM_KEYDOWN, VK_ESCAPE as usize, 0);
    pump();
    check(
        APP.with(|s| s.borrow().as_ref().unwrap().language) == Language::Russian,
        "Cancelling language popup committed a language",
    )?;
    smoke_trace("node gestures");
    let canvas = handle(CANVAS);
    let start = node_point(0, None);
    pointer(canvas, WM_LBUTTONDOWN, start);
    pointer(
        canvas,
        WM_MOUSEMOVE,
        Point::new(start.x + 60., start.y + 50.),
    );
    check(project() == original, "Drag preview mutated the document")?;
    SendMessageW(canvas, WM_KEYDOWN, VK_ESCAPE as usize, 0);
    pump();
    check(
        project() == original,
        "Cancelling node drag changed project",
    )?;
    pointer(canvas, WM_LBUTTONDOWN, start);
    pointer(
        canvas,
        WM_MOUSEMOVE,
        Point::new(start.x + 60., start.y + 50.),
    );
    SetCapture(hwnd);
    pump();
    ReleaseCapture();
    check(
        project() == original && APP.with(|s| s.borrow().as_ref().unwrap().graph.drag.is_none()),
        "Capture loss did not cancel node drag",
    )?;
    pointer(canvas, WM_LBUTTONDOWN, start);
    pointer(
        canvas,
        WM_MOUSEMOVE,
        Point::new(start.x + 24., start.y + 24.),
    );
    SetFocus(handle(SAVE));
    pump();
    check(
        project() == original && GetCapture() != canvas,
        "Focus loss left graph capture active",
    )?;
    let input = APP.with(|s| {
        let s = s.borrow();
        let a = s.as_ref().unwrap();
        a.graph
            .screen(graph::input(graph::positions(&a.document.project)[0]))
    });
    pointer(canvas, WM_LBUTTONDOWN, input);
    pointer(canvas, WM_LBUTTONUP, input);
    check(
        GetCapture() != canvas,
        "Clicking an input port retained capture",
    )?;
    pointer(canvas, WM_LBUTTONDOWN, start);
    for n in 1..=12 {
        pointer(
            canvas,
            WM_MOUSEMOVE,
            Point::new(start.x + n as f32 * 5., start.y + n as f32 * 4.),
        );
    }
    pointer(
        canvas,
        WM_LBUTTONUP,
        Point::new(start.x + 60., start.y + 48.),
    );
    check(
        project().scenes[0].node_position.is_some(),
        "Native node drag did not commit",
    )?;
    command(hwnd, UNDO);
    check(
        project() == original,
        "Node drag was not a single undo step",
    )?;
    command(hwnd, REDO);
    check(project() != original, "Node redo failed")?;
    command(hwnd, UNDO);
    let from = node_point(0, Some(Port::Choice(0)));
    let to = node_point(2, None);
    pointer(canvas, WM_LBUTTONDOWN, from);
    pointer(canvas, WM_MOUSEMOVE, to);
    pointer(canvas, WM_LBUTTONUP, to);
    check(
        project().scenes[0].choices[0].target == original.scenes[2].id,
        "Native graph wire did not change target",
    )?;
    check(
        APP.with(|s| {
            s.borrow()
                .as_ref()
                .unwrap()
                .controls
                .selection(TARGET, true)
        }) == Some(2),
        "Inspector is not synchronized with node connection",
    )?;
    let mut player = Player::new(&project())?;
    for _ in 0..3 {
        player.advance()?;
    }
    player.choose(0)?;
    check(
        matches!(player.frame(),Frame::Dialogue{scene,..}if scene.id==original.scenes[2].id),
        "Graph connection did not affect playback",
    )?;
    command(hwnd, GRAPH_UNLINK);
    check(
        project().scenes[0].choices[0].target.is_empty(),
        "Disconnect failed",
    )?;
    command(hwnd, UNDO);
    command(hwnd, UNDO);
    check(project() == original, "Connection undo failed")?;
    command(hwnd, GRAPH_ARRANGE);
    check(
        project().scenes.iter().all(|s| s.node_position.is_some()),
        "Arrange did not persist positions",
    )?;
    command(hwnd, UNDO);
    check(project() == original, "Arrange undo failed")?;
    // Structural edits must not leave a port selected on a different scene or choice.
    let from = node_point(0, Some(Port::Choice(1)));
    pointer(canvas, WM_LBUTTONDOWN, from);
    pointer(canvas, WM_LBUTTONUP, from);
    command(hwnd, UP);
    check(
        APP.with(|s| s.borrow().as_ref().unwrap().graph.selected_port)
            == Some((0, Port::Choice(0))),
        "Choice reorder did not move its selected port",
    )?;
    command(hwnd, UNDO);
    command(hwnd, ADD_ITEM);
    check(
        APP.with(|s| s.borrow().as_ref().unwrap().graph.selected_port)
            == Some((0, Port::Choice(2))),
        "New choice did not select its port",
    )?;
    command(hwnd, DELETE_ITEM);
    check(
        APP.with(|s| s.borrow().as_ref().unwrap().graph.selected_port.is_none()),
        "Deleted choice left a stale selected port",
    )?;
    command(hwnd, UNDO);
    command(hwnd, UNDO);
    let from = node_point(0, Some(Port::Choice(0)));
    pointer(canvas, WM_LBUTTONDOWN, from);
    pointer(canvas, WM_LBUTTONUP, from);
    command(hwnd, DUP_SCENE);
    check(
        APP.with(|s| {
            let s = s.borrow();
            let a = s.as_ref().unwrap();
            a.graph.selected_port.is_none() && a.selected_scene == original.scenes.len()
        }),
        "Duplicating a scene retained another scene's selected port",
    )?;
    command(hwnd, UNDO);
    APP.with(|s| {
        let mut s = s.borrow_mut();
        let a = s.as_mut().unwrap();
        a.selected_scene = 0;
        a.selected_item = 0;
        a.refresh();
    });
    pump();
    check(
        project() == original,
        "Structural graph edits did not undo cleanly",
    )?;
    command(hwnd, ZOOM_IN);
    command(hwnd, ZOOM_OUT);
    command(hwnd, GRAPH_FIT);
    command(hwnd, THEME);
    UpdateWindow(hwnd);
    pump();
    paint::capture(hwnd, &output.join("nodes-light.png"))?;
    command(hwnd, THEME);
    // Exercise long native text and lists, including thumb tracking and keyboard scrolling.
    command(hwnd, VIEW_PREVIEW);
    command(hwnd, TAB_LINES);
    APP.with(|slot| {
        let mut s = slot.borrow_mut();
        let a = s.as_mut().unwrap();
        a.selected_scene = 0;
        a.selected_item = 0;
        a.document.change(|p| {
            p.scenes[0].lines = (0..45)
                .map(|n| Line {
                    speaker: format!("Speaker {n}"),
                    text: if n == 0 {
                        (1..=70)
                            .map(|n| {
                                format!("{n:02} · Русский · English · 한국어 · 日本語 · 中文\r\n")
                            })
                            .collect()
                    } else {
                        format!("Line {n}")
                    },
                })
                .collect();
        });
        a.refresh();
    });
    pump();
    smoke_trace("scrollbar tests");
    let before_scroll = project();
    for id in [BODY, ITEMS] {
        let bar = APP.with(|s| {
            *s.borrow()
                .as_ref()
                .unwrap()
                .controls
                .scrolls
                .get(&id)
                .unwrap()
        });
        check(
            IsWindowVisible(bar.hwnd) != 0,
            "Vertical scrollbar missing for long content",
        )?;
        let (r, arrow, thumb_top, thumb_bottom) = scroll::geometry(bar.hwnd);
        smoke_trace(&format!(
            "scrollbar {id} height={} thumb={thumb_top}..{thumb_bottom}",
            r.bottom
        ));
        SendMessageW(
            bar.hwnd,
            WM_LBUTTONDOWN,
            0,
            packed(r.right / 2, (thumb_top + thumb_bottom) / 2),
        );
        SendMessageW(
            bar.hwnd,
            WM_MOUSEMOVE,
            0,
            packed(r.right / 2, r.bottom - arrow - 4),
        );
        SendMessageW(
            bar.hwnd,
            WM_LBUTTONUP,
            0,
            packed(r.right / 2, r.bottom - arrow - 4),
        );
        pump();
        let first = SendMessageW(
            bar.target,
            if id == BODY {
                EM_GETFIRSTVISIBLELINE
            } else {
                LB_GETTOPINDEX
            },
            0,
            0,
        );
        if first <= 0 {
            paint::capture(hwnd, &output.join("scroll-failure.png"))?;
            return Err(format!("Dragging scrollbar {id} did not scroll content"));
        }
        SendMessageW(bar.hwnd, WM_KEYDOWN, VK_HOME as usize, 0);
        pump();
        check(
            SendMessageW(
                bar.target,
                if id == BODY {
                    EM_GETFIRSTVISIBLELINE
                } else {
                    LB_GETTOPINDEX
                },
                0,
                0,
            ) == 0,
            "Scrollbar Home failed",
        )?;
        SendMessageW(
            bar.target,
            WM_MOUSEWHEEL,
            ((-120i16 as u16 as usize) << 16) as WPARAM,
            0,
        );
        pump();
        check(
            SendMessageW(
                bar.target,
                if id == BODY {
                    EM_GETFIRSTVISIBLELINE
                } else {
                    LB_GETTOPINDEX
                },
                0,
                0,
            ) > 0,
            "Mouse wheel did not scroll content",
        )?;
    }
    check(
        project() == before_scroll,
        "Scrolling modified document text",
    )?;
    UpdateWindow(hwnd);
    pump();
    paint::capture(hwnd, &output.join("scrollbars.png"))?;
    APP.with(|slot| {
        let mut s = slot.borrow_mut();
        let a = s.as_mut().unwrap();
        a.document = Document::new(original.clone());
        a.selected_scene = 0;
        a.selected_item = 0;
        a.graph = Graph::default();
        a.choices_tab = false;
        a.graph_mode = false;
        a.refresh();
    });
    pump();
    language(hwnd, old_language);
    check(project() == original, "UI smoke did not restore demo")?;
    // Minimum supported window keeps every control reachable after localization.
    SetWindowPos(
        hwnd,
        null_mut(),
        0,
        0,
        1120,
        760,
        SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
    );
    pump();
    language(hwnd, Language::Japanese);
    paint::capture(hwnd, &output.join("editor-small-ja.png"))?;
    language(hwnd, old_language);
    SetWindowPos(
        hwnd,
        null_mut(),
        0,
        0,
        1440,
        960,
        SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
    );
    pump();
    Ok(())
}
