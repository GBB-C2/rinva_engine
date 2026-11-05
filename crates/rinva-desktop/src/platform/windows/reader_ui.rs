use super::*;
pub const AUTO: u16 = 3000;
pub const SKIP: u16 = 3001;
pub const LOG: u16 = 3002;
pub const SPEED: u16 = 3003;
pub const MUSIC: u16 = 3004;
pub const EFFECTS: u16 = 3005;
pub const VOICE: u16 = 3006;
const LOG_TEXT: u16 = 3007;
const LABELS: [u16; 4] = [5100, 5101, 5102, 5103];
pub unsafe fn create(
    controls: &mut Controls,
    hwnd: HWND,
    resources: &Resources,
) -> Result<(), String> {
    for (id, label) in [(AUTO, "Авто"), (SKIP, "Прочитанное"), (LOG, "Журнал")]
    {
        controls.button(hwnd, id, label, resources.font)?;
    }
    for id in [SPEED, MUSIC, EFFECTS, VOICE] {
        controls.combo(hwnd, id, resources.font)?;
    }
    controls.edit(hwnd, LOG_TEXT, true, resources.font)?;
    SendMessageW(controls.get(LOG_TEXT), EM_SETREADONLY, 1, 0);
    SendMessageW(controls.get(LOG_TEXT), EM_SETLIMITTEXT, 4_000_000, 0);
    for id in LABELS {
        controls.label(hwnd, id, "", resources.small)?;
    }
    Ok(())
}
pub unsafe fn layout(app: &App, w: i32, h: i32) {
    for id in [AUTO, SKIP, LOG, SPEED, MUSIC, EFFECTS, VOICE, LOG_TEXT]
        .into_iter()
        .chain(LABELS)
    {
        app.controls.show(id, app.player.is_some());
    }
    if app.player.is_none() {
        return;
    }
    let place = |id, x, y, width, height| app.controls.place(id, x, y, width, height, app.scale);
    place(AUTO, 32, 76, 76, 30);
    place(SKIP, 116, 76, 124, 30);
    place(LOG, 248, 76, 82, 30);
    let labels = ["Текст / с", "Музыка %", "Эффекты %", "Голос %"];
    for (n, (id, label)) in [SPEED, MUSIC, EFFECTS, VOICE]
        .into_iter()
        .zip(labels)
        .enumerate()
    {
        let x = 350 + n as i32 * 176;
        app.controls.text(LABELS[n], app.t(label));
        place(LABELS[n], x, 80, 90, 22);
        place(id, x + 92, 76, 74, 220);
    }
    place(CANVAS, 20, 116, w - 40, h - 340);
    app.controls.show(LOG_TEXT, app.reader.log);
    if app.reader.log {
        app.controls.show(CANVAS, false);
        place(LOG_TEXT, 32, 116, w - 64, h - 340);
    }
}
pub unsafe fn refresh(app: &mut App) {
    for (id, key) in [(AUTO, "Авто"), (SKIP, "Прочитанное"), (LOG, "Журнал")] {
        app.controls.text(id, app.t(key));
    }
    app.controls.fill_list(
        SPEED,
        &[app.t("Сразу").into(), "20".into(), "40".into(), "60".into()],
        if app.reader.settings.speed == 0 {
            0
        } else if app.reader.settings.speed <= 20 {
            1
        } else if app.reader.settings.speed <= 40 {
            2
        } else {
            3
        },
        true,
    );
    for (i, id) in [MUSIC, EFFECTS, VOICE].iter().enumerate() {
        app.controls.fill_list(
            *id,
            &[
                "0".into(),
                "25".into(),
                "50".into(),
                "75".into(),
                "100".into(),
            ],
            usize::from(app.reader.settings.volumes[i])
                .div_ceil(25)
                .min(4),
            true,
        );
    }
    let Some(player) = &mut app.player else {
        if app.reader.key.take().is_some() {
            app.audio.stop();
        }
        return;
    };
    let entering = app
        .reader
        .enter(player.serial(), player.line_key(), player.was_read());
    if entering {
        let voice = match player.frame() {
            Frame::Dialogue { line, .. } => line.voice.clone(),
            _ => None,
        };
        app.audio.play(audio::Request {
            root: app.asset_root.clone(),
            music: player.music().cloned(),
            voice,
            effects: player.effects().to_vec(),
            volumes: app.reader.settings.volumes,
            fade_ms: player.music_fade_ms(),
        });
        if !matches!(player.frame(), Frame::Dialogue { .. })
            || app.reader.skip && !app.reader.seen_before
        {
            app.reader.skip = false;
        }
    }
    if let Frame::Dialogue { line, .. } = player.frame() {
        app.reader.tick(&line.text, app.reader.started.elapsed());
    }
    app.controls.text(LOG_TEXT, &player.backlog());
    update_text(app);
}
unsafe fn update_text(app: &mut App) {
    if let Some(player) = &mut app.player {
        if let Frame::Dialogue { line, .. } = player.frame() {
            let text = app.reader.visible(&line.text);
            app.controls.text(
                TRANSCRIPT,
                &format!(
                    "{}{}{}",
                    line.speaker,
                    if line.speaker.is_empty() {
                        ""
                    } else {
                        "\r\n\r\n"
                    },
                    text
                ),
            );
            if app.reader.completed.is_some() {
                player.mark_read();
            }
        }
    }
    InvalidateRect(app.controls.get(CANVAS), null(), 0);
}
pub unsafe fn tick(app: &mut App) {
    if let Some(error) = app.audio.error() {
        app.status = error;
        app.update_title();
    }
    let Some(player) = &app.player else {
        return;
    };
    let Frame::Dialogue { line, .. } = player.frame() else {
        return;
    };
    if app.reader.log {
        return;
    }
    let changed = app.reader.tick(&line.text, app.reader.started.elapsed());
    let advance = app.reader.advance_due(&line.text, app.audio.voice_busy());
    if changed {
        update_text(app);
    }
    if advance {
        if let Err(error) = app.command(ADVANCE, 0) {
            app.reader.auto = false;
            app.reader.skip = false;
            app.status = error;
            app.update_title();
        }
    }
}
pub unsafe fn reveal_first(app: &mut App) -> bool {
    if app.reader.log {
        return true;
    }
    if let Some(player) = &app.player {
        if let Frame::Dialogue { line, .. } = player.frame() {
            if app.reader.shown < line.text.chars().count() {
                app.reader.reveal(&line.text);
                update_text(app);
                return true;
            }
        }
    }
    false
}
pub fn remember(app: &App) {
    if let Some(player) = &app.player {
        let folder = settings_dir().join("read");
        if std::fs::create_dir_all(&folder).is_ok() {
            let _ = storage::write_json(
                &folder.join(format!("{}.json", app.document.project.story_signature())),
                player.read_lines(),
            );
        }
    }
}
pub fn restore_read(app: &mut App) {
    if let Some(player) = &mut app.player {
        if let Ok(read) = storage::read_json(
            &settings_dir()
                .join("read")
                .join(format!("{}.json", app.document.project.story_signature())),
        ) {
            player.merge_read(read);
        }
    }
}
pub unsafe fn command(app: &mut App, id: u16, notification: u16) -> Result<bool, String> {
    if !(AUTO..=VOICE).contains(&id) {
        return Ok(false);
    }
    if [SPEED, MUSIC, EFFECTS, VOICE].contains(&id) {
        match notification as u32 {
            CBN_SELENDCANCEL => {
                app.refresh();
                return Ok(true);
            }
            CBN_SELENDOK => {}
            CBN_SELCHANGE if SendMessageW(app.controls.get(id), CB_GETDROPPEDSTATE, 0, 0) == 0 => {}
            _ => return Ok(true),
        }
    }
    match id {
        AUTO => {
            app.reader.auto = !app.reader.auto;
            app.reader.skip = false;
        }
        SKIP => {
            app.reader.skip = !app.reader.skip;
            app.reader.auto = false;
            if app.reader.skip && !app.reader.seen_before {
                app.reader.skip = false;
                app.status = "Пропуск остановлен на непрочитанной реплике.".into();
            }
        }
        LOG => {
            app.reader.log = !app.reader.log;
            if !app.reader.log {
                app.reader.started = std::time::Instant::now();
                app.reader.completed = None;
            }
        }
        SPEED => {
            app.reader.settings.speed =
                [0, 20, 40, 60][app.controls.selection(id, true).unwrap_or(0).min(3)];
            app.reader.started = std::time::Instant::now();
            app.persist_settings()?;
        }
        _ => {
            let i = [MUSIC, EFFECTS, VOICE]
                .iter()
                .position(|n| *n == id)
                .unwrap();
            app.reader.settings.volumes[i] =
                (app.controls.selection(id, true).unwrap_or(4).min(4) * 25) as u8;
            app.audio.volumes(app.reader.settings.volumes);
            app.persist_settings()?;
        }
    }
    app.refresh();
    Ok(true)
}
