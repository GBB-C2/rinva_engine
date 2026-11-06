//! Exercises the authoring forms and player through native controls, then captures live HWNDs.
use super::*;
use rinva_core::{
    Assignment, AudioCue, Character, Comparison, Condition, Expression, NodeKind, Operation, Scene,
    Sprite,
};
fn handle(id: u16) -> HWND {
    APP.with(|s| s.borrow().as_ref().unwrap().controls.get(id))
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
unsafe fn combo(hwnd: HWND, id: u16, n: usize) {
    SendMessageW(handle(id), CB_SETCURSEL, n, 0);
    SendMessageW(
        hwnd,
        WM_COMMAND,
        id as usize | ((CBN_SELENDOK as usize) << 16),
        0,
    );
    pump();
}
unsafe fn edit(id: u16, value: &str) {
    SetWindowTextW(handle(id), wide(value).as_ptr());
    pump();
}
fn project() -> Project {
    APP.with(|s| s.borrow().as_ref().unwrap().document.project.clone())
}
unsafe fn pointer(id: u16, message: u32, at: Point) {
    let scale = APP.with(|s| s.borrow().as_ref().unwrap().scale);
    let x = (at.x * scale).round() as i32;
    let y = (at.y * scale).round() as i32;
    let packed = ((x as u16 as u32) | ((y as u16 as u32) << 16)) as isize;
    SendMessageW(handle(id), message, 0, packed);
    pump();
}
fn node_point(index: usize) -> Point {
    APP.with(|s| {
        let s = s.borrow();
        let a = s.as_ref().unwrap();
        let at = graph::positions(&a.document.project)[index];
        a.graph.screen(Point::new(at.x + 44., at.y + 28.))
    })
}

fn assets(root: &Path) -> Result<(), String> {
    std::fs::create_dir_all(root.join("assets")).map_err(|e| e.to_string())?;
    // Explicit geometric fixtures, not production character artwork.
    for (name, color) in [
        ("actor-a.png", [113, 166, 236, 255]),
        ("actor-b.png", [220, 154, 120, 255]),
    ] {
        let image = image::RgbaImage::from_fn(160, 280, |x, y| {
            let head = (x as i32 - 80).pow(2) + (y as i32 - 56).pow(2) < 36 * 36;
            let body = (38..122).contains(&x) && (100..280).contains(&y);
            image::Rgba(if head || body { color } else { [0, 0, 0, 0] })
        });
        image
            .save(root.join("assets").join(name))
            .map_err(|e| e.to_string())?;
    }
    let rate = 22050u32;
    let samples = rate * 5;
    let bytes = samples * 2;
    let mut wav = vec![];
    wav.extend(b"RIFF");
    wav.extend((36 + bytes).to_le_bytes());
    wav.extend(b"WAVEfmt ");
    wav.extend(16u32.to_le_bytes());
    wav.extend(1u16.to_le_bytes());
    wav.extend(1u16.to_le_bytes());
    wav.extend(rate.to_le_bytes());
    wav.extend((rate * 2).to_le_bytes());
    wav.extend(2u16.to_le_bytes());
    wav.extend(16u16.to_le_bytes());
    wav.extend(b"data");
    wav.extend(bytes.to_le_bytes());
    for n in 0..samples {
        let value = ((n as f64 / rate as f64 * 440. * std::f64::consts::TAU).sin() * 1200.) as i16;
        wav.extend(value.to_le_bytes());
    }
    std::fs::write(root.join("assets/tone.wav"), wav).map_err(|e| e.to_string())
}
fn fixture() -> Project {
    let mut p = Project::demo();
    p.title = "На развилке · Studio".into();
    p.variables.insert("trust".into(), 0);
    p.characters = vec![
        Character {
            id: "mira".into(),
            name: "Мира".into(),
            expressions: vec![Expression {
                id: "calm".into(),
                name: "Спокойная".into(),
                image: "assets/actor-a.png".into(),
            }],
        },
        Character {
            id: "leo".into(),
            name: "Лео".into(),
            expressions: vec![Expression {
                id: "calm".into(),
                name: "Спокойный".into(),
                image: "assets/actor-b.png".into(),
            }],
        },
    ];
    let mut a = Sprite::new("mira".into(), "calm".into());
    a.x = 30;
    a.scale = 82;
    let mut b = Sprite::new("leo".into(), "calm".into());
    b.x = 70;
    b.scale = 82;
    b.layer = 1;
    for s in &mut p.scenes {
        s.sprites = vec![a.clone(), b.clone()];
        s.group = "Глава 1".into();
    }
    p.scenes[0].audio = Some(AudioCue {
        music: Some("assets/tone.wav".into()),
        effect: Some("assets/tone.wav".into()),
        ..AudioCue::new()
    });
    p.scenes[0].lines[1].voice = Some("assets/tone.wav".into());
    let mut set = Scene::new("trust-up", "Помочь Мире");
    set.kind = NodeKind::SetVariable;
    set.lines.clear();
    set.assignment = Some(Assignment {
        variable: "trust".into(),
        operation: Operation::Add,
        value: 1,
    });
    set.next = Some("trust-check".into());
    let mut condition = Scene::new("trust-check", "Доверие ≥ 1");
    condition.kind = NodeKind::Condition;
    condition.lines.clear();
    condition.condition = Some(Condition {
        variable: "trust".into(),
        comparison: Comparison::AtLeast,
        value: 1,
    });
    condition.next = Some(p.scenes[1].id.clone());
    condition.otherwise = Some(p.scenes[2].id.clone());
    p.scenes[0].choices[0].target = set.id.clone();
    p.scenes.extend([set, condition]);
    p
}
pub unsafe fn run(hwnd: HWND, output: &Path) -> Result<(), String> {
    smoke_trace("studio: fixtures");
    let original = project();
    let old_root = APP.with(|s| s.borrow().as_ref().unwrap().asset_root.clone());
    let root = output.join("studio-fixture");
    assets(&root)?;
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let p = fixture();
    p.check()?;
    storage::write_json(&root.join("story.rinva"), &p)?;
    APP.with(|s| {
        let mut s = s.borrow_mut();
        let a = s.as_mut().unwrap();
        a.player = None;
        a.reader.reset();
        a.document = Document::new(p);
        a.asset_root = root.clone();
        a.navigation = navigation::Navigation::default();
        a.graph = Graph::default();
        a.studio = studio::Studio::default();
        a.selected_scene = 0;
        a.selected_item = 0;
        a.choices_tab = false;
        a.graph_mode = false;
        a.language = Language::Russian;
        a.dark = true;
        a.palette = Palette::new(true);
        a.rebuild_resources();
        a.image_key.clear();
        a.refresh();
    });
    pump();
    combo(hwnd, studio::INSPECTOR, 1);
    edit(studio::VAR_NAME, "affection");
    edit(studio::VAR_VALUE, "3");
    let before = project();
    command(hwnd, studio::VAR_ADD);
    check(
        project().variables.get("affection") == Some(&3),
        "Native variable creation failed",
    )?;
    command(hwnd, UNDO);
    check(project() == before, "Variable undo changed other fields")?;
    command(hwnd, REDO);
    APP.with(|s| {
        let mut s = s.borrow_mut();
        let a = s.as_mut().unwrap();
        a.selected_scene = 5;
        a.studio.pane = studio::Pane::Logic;
        a.refresh();
    });
    pump();
    combo(hwnd, studio::RULE_VAR, 2);
    combo(hwnd, studio::RULE_OP, 3);
    edit(studio::RULE_VALUE, "2");
    command(hwnd, studio::APPLY_RULE);
    check(
        project().scenes[5]
            .condition
            .as_ref()
            .is_some_and(|c| c.value == 2 && c.variable == "trust"),
        "Native condition editor failed",
    )?;
    command(hwnd, UNDO);
    paint::capture(hwnd, &output.join("studio-logic-ru.png"))?;
    command(hwnd, VIEW_NODES);
    command(hwnd, GRAPH_FIT);
    APP.with(|s| {
        let mut s = s.borrow_mut();
        let a = s.as_mut().unwrap();
        a.selected_scene = 0;
        a.studio.pane = studio::Pane::Notes;
        a.graph.selection = [
            a.document.project.scenes[0].id.clone(),
            a.document.project.scenes[4].id.clone(),
        ]
        .into_iter()
        .collect();
        a.refresh();
    });
    pump();
    edit(studio::GROUP, "Решение");
    edit(studio::NOTE, "Выбор игрока меняет доверие Миры.");
    command(hwnd, studio::NOTES_APPLY);
    check(
        project().scenes[0].group == "Решение" && project().scenes[4].group == "Решение",
        "Native group assignment failed",
    )?;
    let saved_signature = project().story_signature();
    command(hwnd, UNDO);
    check(
        project().story_signature() == saved_signature,
        "Author notes invalidate game progress",
    )?;
    command(hwnd, REDO);
    paint::capture(hwnd, &output.join("studio-nodes-ru.png"))?;
    // Select with the actual Control modifier; move the pair through the canvas callback.
    APP.with(|s| s.borrow_mut().as_mut().unwrap().graph.selection.clear());
    let mut keys = [0u8; 256];
    GetKeyboardState(keys.as_mut_ptr());
    let mut modified = keys;
    modified[VK_CONTROL as usize] |= 0x80;
    SetKeyboardState(modified.as_ptr());
    for index in [0, 4] {
        pointer(CANVAS, WM_LBUTTONDOWN, node_point(index));
        pointer(CANVAS, WM_LBUTTONUP, node_point(index));
    }
    SetKeyboardState(keys.as_ptr());
    check(
        APP.with(|s| s.borrow().as_ref().unwrap().graph.selection.len()) == 2,
        "Control-click did not select two nodes",
    )?;
    let before = project();
    let start = node_point(0);
    let finish = Point::new(start.x + 28., start.y + 20.);
    pointer(CANVAS, WM_LBUTTONDOWN, start);
    pointer(CANVAS, WM_MOUSEMOVE, finish);
    check(project() == before, "Group preview mutated the document")?;
    pointer(CANVAS, WM_LBUTTONUP, finish);
    let after = project();
    let before_positions = graph::positions(&before);
    let after_positions = graph::positions(&after);
    // A first drag freezes the generated layout, including stationary nodes.
    // Compare their actual positions rather than the optional serialized field.
    check(
        before_positions
            .iter()
            .zip(&after_positions)
            .enumerate()
            .all(|(n, (old, new))| {
                if matches!(n, 0 | 4) {
                    old != new
                } else {
                    old == new
                }
            }),
        "Group drag moved the wrong nodes",
    )?;
    let delta = |n: usize| {
        Point::new(
            after_positions[n].x - before_positions[n].x,
            after_positions[n].y - before_positions[n].y,
        )
    };
    check(
        delta(0) == delta(4),
        "Group drag changed relative positions",
    )?;
    let mut content = after;
    for (scene, previous) in content.scenes.iter_mut().zip(&before.scenes) {
        scene.node_position = previous.node_position;
    }
    check(content == before, "Group drag changed story content")?;
    command(hwnd, UNDO);
    check(project() == before, "Group drag was not one undo step")?;
    let old_pan = APP.with(|s| s.borrow().as_ref().unwrap().graph.pan);
    let size = APP.with(|s| s.borrow().as_ref().unwrap().canvas_size());
    pointer(
        CANVAS,
        WM_LBUTTONDOWN,
        Point::new(size.x - 40., size.y - 40.),
    );
    pointer(CANVAS, WM_LBUTTONUP, Point::new(size.x - 40., size.y - 40.));
    check(
        APP.with(|s| s.borrow().as_ref().unwrap().graph.pan) != old_pan && project() == before,
        "Minimap did not navigate without editing the story",
    )?;
    command(hwnd, studio::MINIMAP);
    check(
        !APP.with(|s| s.borrow().as_ref().unwrap().graph.minimap),
        "Minimap hide failed",
    )?;
    command(hwnd, studio::MINIMAP);
    command(hwnd, GRAPH_FIT);
    combo(hwnd, studio::LEFT_WIDTH, 2);
    combo(hwnd, studio::RIGHT_WIDTH, 2);
    command(hwnd, studio::RESET_LAYOUT);
    check(
        APP.with(|s| {
            let s = s.borrow();
            let a = s.as_ref().unwrap();
            a.left_width == 224 && a.right_width == 300
        }),
        "Panel reset failed",
    )?;
    // Actual parent-window splitter drag, followed by Escape rollback and commit.
    let scale = APP.with(|s| s.borrow().as_ref().unwrap().scale);
    let point = |x: i32, y: i32| {
        ((x as f32 * scale) as i32 & 0xffff) as isize
            | ((((y as f32 * scale) as i32 & 0xffff) as isize) << 16)
    };
    SendMessageW(hwnd, WM_LBUTTONDOWN, 0, point(236, 400));
    SendMessageW(hwnd, WM_MOUSEMOVE, 0, point(260, 400));
    SendMessageW(hwnd, WM_KEYDOWN, VK_ESCAPE as usize, 0);
    pump();
    check(
        APP.with(|s| s.borrow().as_ref().unwrap().left_width) == 224,
        "Splitter Escape failed",
    )?;
    SendMessageW(hwnd, WM_LBUTTONDOWN, 0, point(236, 400));
    SendMessageW(hwnd, WM_MOUSEMOVE, 0, point(260, 400));
    SendMessageW(hwnd, WM_LBUTTONUP, 0, point(260, 400));
    pump();
    check(
        APP.with(|s| s.borrow().as_ref().unwrap().left_width) > 224,
        "Splitter drag failed",
    )?;
    command(hwnd, studio::RESET_LAYOUT);
    command(hwnd, VIEW_PREVIEW);
    combo(hwnd, studio::INSPECTOR, 3);
    edit(studio::SPRITE_X, "24");
    edit(studio::SPRITE_SCALE, "76");
    command(hwnd, studio::CHARACTER_APPLY);
    check(
        project().scenes[0].sprites[0].x == 24 && project().scenes[0].sprites[0].scale == 76,
        "Native sprite transform failed",
    )?;
    check(
        APP.with(|s| s.borrow().as_ref().unwrap().sprites.len()) == 2,
        "Multiple character bitmaps were not loaded",
    )?;
    paint::capture(hwnd, &output.join("studio-characters-ru.png"))?;
    combo(hwnd, studio::INSPECTOR, 2);
    edit(studio::MUSIC_VOLUME, "60");
    edit(studio::FADE, "300");
    command(hwnd, studio::AUDIO_APPLY);
    check(
        project().scenes[0]
            .audio
            .as_ref()
            .is_some_and(|a| a.volume == 60 && a.fade_ms == 300),
        "Native audio inspector failed",
    )?;
    paint::capture(hwnd, &output.join("studio-audio-ru.png"))?;
    command(hwnd, studio::AUDIO_PREVIEW);
    std::thread::sleep(std::time::Duration::from_millis(1000));
    pump();
    let probe = APP.with(|s| s.borrow().as_ref().unwrap().audio.probe())?;
    std::fs::write(output.join("audio-probe.txt"), &probe).map_err(|e| e.to_string())?;
    smoke_trace(&format!("audio probe: {probe}"));
    // Channel creation is mandatory. Actual speaker output needs a physical device.
    check(
        probe.contains("music=true") && probe.contains("effects=1"),
        "Native MediaPlayer channels did not open",
    )?;
    command(hwnd, studio::AUDIO_STOP);
    check(
        APP.with(|s| s.borrow().as_ref().unwrap().audio.probe())?
            .contains("music=false"),
        "Audio stop did not close channel",
    )?;
    command(hwnd, THEME);
    paint::capture(hwnd, &output.join("studio-light.png"))?;
    command(hwnd, THEME);
    command(hwnd, PLAY);
    combo(hwnd, reader_ui::SPEED, 1);
    APP.with(|s| {
        let mut s = s.borrow_mut();
        let a = s.as_mut().unwrap();
        a.reader.shown = 0;
        a.reader.started = std::time::Instant::now();
        a.reader.completed = None;
    });
    let position = APP.with(|s| {
        s.borrow()
            .as_ref()
            .unwrap()
            .player
            .as_ref()
            .unwrap()
            .line_key()
    });
    command(hwnd, ADVANCE);
    check(
        APP.with(|s| {
            s.borrow()
                .as_ref()
                .unwrap()
                .player
                .as_ref()
                .unwrap()
                .line_key()
        }) == position,
        "First advance did not reveal text",
    )?;
    command(hwnd, ADVANCE);
    check(
        APP.with(|s| {
            s.borrow()
                .as_ref()
                .unwrap()
                .player
                .as_ref()
                .unwrap()
                .line_key()
        }) != position,
        "Second advance did not change dialogue",
    )?;
    command(hwnd, ADVANCE);
    paint::capture(hwnd, &output.join("studio-reader-ru.png"))?;
    command(hwnd, reader_ui::LOG);
    paint::capture(hwnd, &output.join("studio-backlog-ru.png"))?;
    check(
        !ui::window_text(handle(3007)).is_empty(),
        "Backlog is empty after reading",
    )?;
    command(hwnd, reader_ui::LOG);
    command(hwnd, STOP);
    combo(hwnd, LANGUAGE, Language::Japanese.index());
    combo(hwnd, studio::INSPECTOR, 3);
    SetWindowPos(
        hwnd,
        null_mut(),
        0,
        0,
        (1120. * scale) as i32,
        (760. * scale) as i32,
        SWP_NOMOVE | SWP_NOZORDER,
    );
    pump();
    paint::capture(hwnd, &output.join("studio-small-ja.png"))?;
    SetWindowPos(
        hwnd,
        null_mut(),
        0,
        0,
        (1440. * scale) as i32,
        (960. * scale) as i32,
        SWP_NOMOVE | SWP_NOZORDER,
    );
    pump();
    // Export every new asset reference and restore a save from the exported project.
    let p = project();
    let destination = root.join("portable");
    storage::export_bundle(
        &p,
        &root,
        &destination,
        &std::env::current_exe().map_err(|e| e.to_string())?,
    )?;
    check(
        storage::assets(&p)
            .iter()
            .all(|path| destination.join(path).is_file()),
        "New media missing from exported bundle",
    )?;
    let exported = storage::read_project(&destination.join("story.rinva"))?;
    Player::new(&exported)?.restore(Player::new(&p)?.save())?;
    std::fs::remove_dir_all(&destination).map_err(|e| e.to_string())?;
    APP.with(|s| {
        let mut s = s.borrow_mut();
        let a = s.as_mut().unwrap();
        a.audio.stop();
        a.reader.reset();
        a.reader.settings.speed = 0;
        a.document = Document::new(original);
        a.asset_root = old_root;
        a.selected_scene = 0;
        a.selected_item = 0;
        a.choices_tab = false;
        a.studio = studio::Studio::default();
        a.graph = Graph::default();
        a.graph_mode = false;
        a.language = Language::Russian;
        a.image_key.clear();
        a.rebuild_resources();
        a.refresh();
    });
    pump();
    smoke_trace("studio: passed");
    Ok(())
}
