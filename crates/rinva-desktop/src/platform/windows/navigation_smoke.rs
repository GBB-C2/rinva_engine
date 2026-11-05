//! Native regression scenarios use the same callbacks and keyboard routing as the editor.
use super::*;
use navigation::*;

fn handle(id: u16) -> HWND {
    APP.with(|s| s.borrow().as_ref().unwrap().controls.get(id))
}
fn project() -> Project {
    APP.with(|s| s.borrow().as_ref().unwrap().document.project.clone())
}
fn check(value: bool, message: &str) -> Result<(), String> {
    if value {
        Ok(())
    } else {
        Err(message.into())
    }
}
unsafe fn command(hwnd: HWND, id: u16) {
    SendMessageW(hwnd, WM_COMMAND, id as usize, 0);
    pump();
}
unsafe fn key(hwnd: HWND, key: u16) -> Result<(), String> {
    let msg = MSG {
        hwnd,
        message: WM_KEYDOWN,
        wParam: key as usize,
        ..std::mem::zeroed()
    };
    check(
        shortcut(&msg),
        "Navigation keyboard shortcut was not routed",
    )?;
    pump();
    Ok(())
}
unsafe fn query(hwnd: HWND, value: &str) {
    SetWindowTextW(handle(QUERY), wide(value).as_ptr());
    pump();
    command(hwnd, RECHECK);
}
unsafe fn filter(hwnd: HWND, index: usize) {
    SendMessageW(handle(FILTER), CB_SETCURSEL, index, 0);
    SendMessageW(
        hwnd,
        WM_COMMAND,
        FILTER as usize | ((CBN_SELENDOK as usize) << 16),
        0,
    );
    pump();
}
unsafe fn fixture(p: Project) {
    APP.with(|s| {
        let mut s = s.borrow_mut();
        let a = s.as_mut().unwrap();
        a.document = Document::new(p);
        a.navigation = Navigation::default();
        a.graph = Graph::default();
        a.graph_mode = false;
        a.selected_scene = 0;
        a.selected_item = 0;
        a.choices_tab = false;
        a.image_key.clear();
        a.refresh();
    });
    pump();
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
    smoke_trace("navigation: Unicode selection");
    let original = project();
    let old_language = APP.with(|s| s.borrow().as_ref().unwrap().language);
    let mut unicode = original.clone();
    unicode.scenes[0].lines[1].text = "🌙\r\nТихий ПРИЛИВ · 日本語 · 한국어 · 中文".into();
    fixture(unicode.clone());
    command(hwnd, VIEW_NODES);
    command(hwnd, SEARCH);
    query(hwnd, "прилив");
    SetFocus(handle(QUERY));
    pump();
    SendMessageW(handle(QUERY), WM_IME_STARTCOMPOSITION, 0, 0);
    for code in [VK_RETURN, VK_ESCAPE] {
        let message = MSG {
            hwnd: handle(QUERY),
            message: WM_KEYDOWN,
            wParam: code as usize,
            ..std::mem::zeroed()
        };
        check(
            !shortcut(&message),
            "Navigation shortcut intercepted IME composition",
        )?;
    }
    SendMessageW(handle(QUERY), WM_IME_ENDCOMPOSITION, 0, 0);
    check(
        SendMessageW(handle(RESULTS), LB_GETCOUNT, 0, 0) == 1,
        "Unicode search returned wrong native count",
    )?;
    // Enter from the native query goes to the author field and selects UTF-16 units.
    SetFocus(handle(QUERY));
    pump();
    key(handle(QUERY), VK_RETURN)?;
    let (scene, item, closed) = APP.with(|s| {
        let s = s.borrow();
        let a = s.as_ref().unwrap();
        (
            a.selected_scene,
            a.selected_item,
            a.navigation.mode.is_none(),
        )
    });
    check(
        scene == 0 && item == 1 && closed && GetFocus() == handle(BODY),
        "Search did not navigate to exact dialogue",
    )?;
    let mut from: u32 = 0;
    let mut to: u32 = 0;
    SendMessageW(
        handle(BODY),
        EM_GETSEL,
        &mut from as *mut _ as usize,
        &mut to as *mut _ as isize,
    );
    let text = ui::window_text(handle(BODY))
        .encode_utf16()
        .collect::<Vec<_>>();
    check(
        String::from_utf16(&text[from as usize..to as usize]).unwrap() == "ПРИЛИВ",
        "Native search selection split Unicode or highlighted wrong text",
    )?;
    check(
        project() == unicode && !APP.with(|s| s.borrow().as_ref().unwrap().document.can_undo()),
        "Search altered the story or undo history",
    )?;
    // A result retained across an external structural edit must not target another line.
    command(hwnd, SEARCH);
    query(hwnd, "прилив");
    APP.with(|s| {
        s.borrow_mut().as_mut().unwrap().document.project.scenes[0]
            .lines
            .remove(1);
    });
    command(hwnd, GO);
    check(
        APP.with(|s| s.borrow().as_ref().unwrap().navigation.mode.is_some()),
        "Stale search result navigated to a different line",
    )?;

    fixture(original.clone());
    language(hwnd, Language::Russian);
    command(hwnd, SEARCH);
    query(hwnd, "свет");
    check(
        SendMessageW(handle(RESULTS), LB_GETCOUNT, 0, 0) >= 2,
        "Demo search did not find scene and title",
    )?;
    SetFocus(handle(QUERY));
    pump();
    key(handle(QUERY), VK_F3)?;
    check(
        GetFocus() == handle(QUERY),
        "Find-next stole focus from query",
    )?;
    paint::capture(hwnd, &output.join("search-ru.png"))?;
    command(hwnd, THEME);
    paint::capture(hwnd, &output.join("search-light.png"))?;
    command(hwnd, THEME);
    for (lang, file) in [
        (Language::English, "search-en.png"),
        (Language::Korean, "search-ko.png"),
        (Language::Japanese, "search-ja.png"),
        (Language::Chinese, "search-zh.png"),
    ] {
        language(hwnd, lang);
        check(
            ui::window_text(handle(SEARCH)) == lang.text("Поиск"),
            "Search label was not localized",
        )?;
        paint::capture(hwnd, &output.join(file))?;
    }
    language(hwnd, Language::Japanese);
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
    paint::capture(hwnd, &output.join("search-small-ja.png"))?;
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
    language(hwnd, Language::Russian);
    filter(hwnd, 2);
    check(
        project() == original,
        "Changing search scope changed the story",
    )?;
    query(hwnd, "no-rinva-match-12345");
    check(
        SendMessageW(handle(RESULTS), LB_GETCOUNT, 0, 0) == 0 && IsWindowEnabled(handle(GO)) == 0,
        "Empty search retained an actionable result",
    )?;
    command(hwnd, CLEAR);
    check(
        ui::window_text(handle(QUERY)).is_empty(),
        "Search clear did not clear the native field",
    )?;
    SetFocus(handle(QUERY));
    pump();
    key(handle(QUERY), VK_ESCAPE)?;
    check(
        APP.with(|s| s.borrow().as_ref().unwrap().navigation.mode.is_none()),
        "Escape did not close navigation",
    )?;

    smoke_trace("navigation: diagnostics and repairs");
    let mut broken = original.clone();
    broken.scenes[0].lines[1].text.clear();
    broken.scenes[0].choices[1].target = "missing".into();
    broken.scenes[1].background = Some("assets/missing-background.png".into());
    broken
        .scenes
        .push(rinva_core::Scene::new("orphan", "Неиспользованная сцена"));
    fixture(broken);
    command(hwnd, PLAY);
    check(
        APP.with(|s| {
            let s = s.borrow();
            let a = s.as_ref().unwrap();
            a.player.is_none() && a.navigation.mode == Some(Mode::Problems)
        }),
        "Invalid play did not open diagnostics",
    )?;
    check(
        SendMessageW(handle(RESULTS), LB_GETCOUNT, 0, 0) >= 4,
        "Diagnostics omitted errors, assets or warnings",
    )?;
    paint::capture(hwnd, &output.join("problems-ru.png"))?;
    command(hwnd, THEME);
    paint::capture(hwnd, &output.join("problems-light.png"))?;
    command(hwnd, THEME);
    language(hwnd, Language::Japanese);
    paint::capture(hwnd, &output.join("problems-ja.png"))?;
    language(hwnd, Language::Russian);
    // The first issue is the second dialogue, not the first line in the scene.
    command(hwnd, GO);
    check(
        APP.with(|s| s.borrow().as_ref().unwrap().selected_item) == 1 && GetFocus() == handle(BODY),
        "Diagnostic did not focus the empty dialogue",
    )?;
    command(hwnd, PROBLEMS);
    SendMessageW(handle(RESULTS), LB_SETCURSEL, 0, 0);
    SendMessageW(
        hwnd,
        WM_COMMAND,
        RESULTS as usize | ((LBN_SELCHANGE as usize) << 16),
        0,
    );
    pump();
    SetFocus(handle(BODY));
    pump();
    // WM_SETTEXT deliberately sends no EN_CHANGE for a multiline EDIT. Exercise
    // the actual native insertion path used by typing/paste, including undo.
    SendMessageW(handle(BODY), EM_SETSEL, 0, -1);
    SendMessageW(
        handle(BODY),
        EM_REPLACESEL,
        1,
        wide("Исправленная реплика.").as_ptr() as isize,
    );
    pump();
    check(
        project().scenes[0].lines[1].text == "Исправленная реплика.",
        "Native multiline edit did not reach the story",
    )?;
    std::thread::sleep(std::time::Duration::from_millis(220));
    SendMessageW(hwnd, WM_TIMER, 1, 0);
    pump();
    check(
        GetFocus() == handle(BODY) && SendMessageW(handle(RESULTS), LB_GETCOUNT, 0, 0) == 4,
        "Live diagnostics did not update or stole edit focus",
    )?;
    key(handle(BODY), VK_F8)?;
    filter(hwnd, 1);
    command(hwnd, GO);
    check(
        (GetFocus() == handle(TARGET) || IsChild(handle(TARGET), GetFocus()) != 0)
            && APP.with(|s| s.borrow().as_ref().unwrap().selected_item) == 1,
        "Missing choice target did not focus the destination selector",
    )?;
    SendMessageW(handle(TARGET), CB_SETCURSEL, 2, 0);
    SendMessageW(
        hwnd,
        WM_COMMAND,
        TARGET as usize | ((CBN_SELENDOK as usize) << 16),
        0,
    );
    pump();
    command(hwnd, PROBLEMS);
    filter(hwnd, 1);
    command(hwnd, GO);
    check(
        GetFocus() == handle(BACKGROUND),
        "Missing image did not focus the background picker",
    )?;
    command(hwnd, CLEAR_BG);
    command(hwnd, PROBLEMS);
    filter(hwnd, 2);
    check(
        SendMessageW(handle(RESULTS), LB_GETCOUNT, 0, 0) == 1,
        "Unreachable-scene warning was lost or duplicated",
    )?;
    command(hwnd, GO);
    check(
        APP.with(|s| s.borrow().as_ref().unwrap().scene().id.clone()) == "orphan",
        "Warning did not navigate to the unreachable scene",
    )?;
    let repaired = project();
    command(hwnd, PLAY);
    check(
        APP.with(|s| s.borrow().as_ref().unwrap().player.is_some()),
        "Warnings incorrectly blocked playback",
    )?;
    command(hwnd, STOP);
    check(
        project() == repaired,
        "Navigation or playback changed repaired story",
    )?;
    fixture(original.clone());
    language(hwnd, old_language);
    check(
        project() == original,
        "Navigation smoke did not restore the demo",
    )?;
    smoke_trace("navigation tests passed");
    Ok(())
}
