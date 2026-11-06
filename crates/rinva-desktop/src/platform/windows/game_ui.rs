//! Native player menus. The existing Player remains the sole owner of story progress.
use super::*;
use rinva_core::saves::{self, Record, Slot};
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

pub const MENU: u16 = 4200;
pub const FULLSCREEN: u16 = 4201;
pub const RESUME: u16 = 4202;
pub const NEW_GAME: u16 = 4203;
pub const SAVE_PAGE: u16 = 4204;
pub const LOAD_PAGE: u16 = 4205;
pub const OPTIONS: u16 = 4206;
pub const EXIT: u16 = 4207;
pub const RETURN: u16 = 4208;
pub const SLOTS: u16 = 4209;
pub const APPLY: u16 = 4210;
pub const STYLE: u16 = 4211;
pub const TEXT_SIZE: u16 = 4212;
const TITLE: u16 = 5300;
const DETAIL: u16 = 5301;
const OPTION_LABELS: [u16; 6] = [5302, 5303, 5304, 5305, 5306, 5307];
const BUTTONS: &[(u16, &str)] = &[
    (MENU, "Меню · Esc"),
    (FULLSCREEN, "Полный экран · F11"),
    (RESUME, "Продолжить"),
    (NEW_GAME, "Новая игра"),
    (SAVE_PAGE, "Сохранения"),
    (LOAD_PAGE, "Загрузить"),
    (OPTIONS, "Настройки"),
    (EXIT, "Выход"),
    (RETURN, "Назад"),
    (APPLY, "Загрузить"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Home,
    Save,
    Load,
    Settings,
}
type Slots = Vec<Result<Option<Record>, String>>;
pub struct Game {
    pub page: Option<Page>,
    pub session: bool,
    pub selected: usize,
    records: Slots,
    thumbnails: Vec<Option<paint::Bitmap>>,
    scan: Option<mpsc::Receiver<Slots>>,
    writer: Option<mpsc::Receiver<Result<(), String>>>,
    writing_slot: Option<Slot>,
    save_error: Option<String>,
    pub fullscreen: Option<(WINDOWPLACEMENT, isize)>,
    autosave_due: bool,
    autosave_at: Instant,
    paused_at: Option<Instant>,
    pub closing: bool,
}
impl Game {
    pub fn busy(&self) -> bool {
        self.writer.is_some() || self.scan.is_some()
    }
}
impl Default for Game {
    fn default() -> Self {
        Self {
            page: None,
            session: false,
            selected: 0,
            records: vec![],
            thumbnails: vec![],
            scan: None,
            writer: None,
            writing_slot: None,
            save_error: None,
            fullscreen: None,
            autosave_due: false,
            autosave_at: Instant::now(),
            paused_at: None,
            closing: false,
        }
    }
}
pub fn root() -> PathBuf {
    settings_dir().join("saves")
}
pub unsafe fn create(c: &mut Controls, hwnd: HWND, r: &Resources) -> Result<(), String> {
    for &(id, key) in BUTTONS {
        c.button(hwnd, id, key, r.font)?;
    }
    c.list(hwnd, SLOTS, r.font)?;
    c.label(hwnd, TITLE, "", r.title)?;
    c.label(hwnd, DETAIL, "", r.small)?;
    for id in OPTION_LABELS {
        c.label(hwnd, id, "", r.font)?;
    }
    for id in [STYLE, TEXT_SIZE] {
        c.combo(hwnd, id, r.font)?;
    }
    Ok(())
}
fn scan(app: &mut App) {
    if app.game.scan.is_some() {
        return;
    }
    let (tx, rx) = mpsc::channel();
    let id = app.document.project.id.clone();
    let root = root();
    std::thread::spawn(move || {
        let result = (0..saves::SLOT_COUNT)
            .map(|n| saves::read(&root, &id, Slot::at(n).unwrap()))
            .collect();
        let _ = tx.send(result);
    });
    app.game.scan = Some(rx);
}
pub unsafe fn open(app: &mut App, page: Page) {
    if app.game.page.is_none() {
        app.game.paused_at = Some(Instant::now());
        app.audio.pause(true);
    }
    app.game.page = Some(page);
    app.reader.log = false;
    if page == Page::Save && app.game.selected < 2 {
        app.game.selected = 2;
    }
    if matches!(page, Page::Save | Page::Load | Page::Home) {
        scan(app);
    }
    app.refresh();
    SetFocus(
        app.controls
            .get(if matches!(page, Page::Save | Page::Load) {
                SLOTS
            } else if page == Page::Settings {
                reader_ui::SPEED
            } else {
                NEW_GAME
            }),
    );
}
pub unsafe fn resume(app: &mut App) {
    if let Some(paused) = app.game.paused_at.take() {
        let elapsed = paused.elapsed();
        app.reader.started += elapsed;
        if let Some(completed) = &mut app.reader.completed {
            *completed += elapsed;
        }
    }
    app.game.page = None;
    app.audio.pause(false);
    app.refresh();
    SetFocus(app.controls.get(ADVANCE));
}
pub fn mark_progress(app: &mut App) {
    if app.game.session && app.player.is_some() {
        app.game.autosave_due = true;
    }
}
pub unsafe fn refresh(app: &mut App) {
    for &(id, key) in BUTTONS {
        app.controls.text(id, app.t(key));
    }
    app.controls.text(
        FULLSCREEN,
        app.t(if app.game.fullscreen.is_some() {
            "Оконный режим · F11"
        } else {
            "Полный экран · F11"
        }),
    );
    SendMessageW(
        app.controls.get(TITLE),
        WM_SETFONT,
        app.resources.title as usize,
        1,
    );
    app.controls.text(
        TITLE,
        match app.game.page {
            Some(Page::Home) => &app.document.project.title,
            Some(Page::Save) => app.t("Сохранения"),
            Some(Page::Load) => app.t("Загрузить"),
            _ => app.t("Настройки"),
        },
    );
    app.controls.text(
        EXIT,
        app.t(if app.player_only {
            "Выход"
        } else {
            "В редактор · Esc"
        }),
    );
    app.controls.text(
        DETAIL,
        app.t(if app.game.scan.is_some() {
            "Чтение сохранений…"
        } else if app.game.writer.is_some() {
            "Запись сохранения…"
        } else {
            "6 слотов · быстрое сохранение F6 · быстрая загрузка F9"
        }),
    );
    app.controls.text(
        APPLY,
        app.t(if app.game.page == Some(Page::Save) {
            "Сохранить в слот"
        } else {
            "Загрузить"
        }),
    );
    app.controls.enable(SAVE_PAGE, app.game.session);
    let latest = latest(app).is_some();
    app.controls.enable(
        RESUME,
        app.game.session || latest || app.game_save_path().is_file(),
    );
    app.controls.enable(NEW_GAME, app.game.writer.is_none());
    app.controls.enable(
        RETURN,
        app.game.session || app.game.page != Some(Page::Home),
    );
    let selected = app
        .game
        .records
        .get(app.game.selected)
        .and_then(|r| r.as_ref().ok())
        .and_then(Option::as_ref);
    app.controls.enable(
        APPLY,
        app.game.writer.is_none()
            && app.game.scan.is_none()
            && if app.game.page == Some(Page::Save) {
                app.game.session && app.game.selected >= 2
            } else {
                selected.is_some_and(|s| s.compatible(&app.document.project))
            },
    );
    let rows: Vec<_> = (0..saves::SLOT_COUNT)
        .map(|i| {
            let name = match i {
                0 => app.t("Автосохранение").to_owned(),
                1 => app.t("Быстрое сохранение").to_owned(),
                _ => format!("{} {}", app.t("Слот"), i - 1),
            };
            let detail = match app.game.records.get(i) {
                Some(Ok(Some(s))) => format!(
                    "{} · {}{}",
                    date(s.saved_at),
                    s.scene,
                    if s.compatible(&app.document.project) {
                        ""
                    } else {
                        app.t(" · другая версия")
                    }
                ),
                Some(Err(_)) => app.t("Не удалось прочитать слот").into(),
                _ => app.t("Пусто").into(),
            };
            format!("{name}\n{detail}")
        })
        .collect();
    app.controls
        .fill_list(SLOTS, &rows, app.game.selected, false);
    for (id, values, selected) in [
        (
            STYLE,
            vec![
                app.t("Тёмный диалог").into(),
                app.t("Светлый диалог").into(),
            ],
            app.reader.settings.dialog_style.min(1) as usize,
        ),
        (
            TEXT_SIZE,
            vec!["100%".into(), "125%".into(), "150%".into()],
            app.reader.settings.text_size.min(2) as usize,
        ),
    ] {
        app.controls.fill_list(id, &values, selected, true);
    }
}
pub unsafe fn layout(app: &App, w: i32, h: i32) {
    for id in BUTTONS
        .iter()
        .map(|b| b.0)
        .chain([SLOTS, STYLE, TEXT_SIZE, TITLE, DETAIL])
        .chain(OPTION_LABELS)
    {
        app.controls.show(id, false);
    }
    if app.player.is_none() {
        return;
    }
    let place = |id, x, y, width, height| {
        app.controls.show(id, true);
        app.controls.place(id, x, y, width, height, app.scale);
    };
    place(MENU, 220, 20, 150, 32);
    place(FULLSCREEN, 382, 20, 190, 32);
    app.controls.show(STOP, false);
    let Some(page) = app.game.page else {
        return;
    };
    for &id in app.controls.handles.keys() {
        app.controls
            .show(id, matches!(id, THEME | LANGUAGE | STATUS));
    }
    place(FULLSCREEN, 28, 20, 190, 32);
    place(TITLE, 300, 112, w - 340, 44);
    place(DETAIL, 300, h - 100, w - 340, 42);
    for (n, id) in [RESUME, NEW_GAME, SAVE_PAGE, LOAD_PAGE, OPTIONS, EXIT]
        .iter()
        .enumerate()
    {
        place(*id, 28, 160 + n as i32 * 48, 228, 36);
    }
    place(RETURN, 28, h - 72, 228, 36);
    if matches!(page, Page::Save | Page::Load) {
        place(SLOTS, 300, 176, w - 340, h - 340);
        SendMessageW(
            app.controls.get(SLOTS),
            LB_SETITEMHEIGHT,
            0,
            (84. * app.scale) as isize,
        );
        place(APPLY, w - 248, h - 148, 208, 36);
    }
    if page == Page::Settings {
        for (n, (id, key)) in [
            reader_ui::SPEED,
            reader_ui::MUSIC,
            reader_ui::EFFECTS,
            reader_ui::VOICE,
            STYLE,
            TEXT_SIZE,
        ]
        .into_iter()
        .zip([
            "Текст / с",
            "Музыка %",
            "Эффекты %",
            "Голос %",
            "Оформление диалога",
            "Размер текста",
        ])
        .enumerate()
        {
            app.controls.text(OPTION_LABELS[n], app.t(key));
            place(OPTION_LABELS[n], 320, 190 + n as i32 * 56, 240, 28);
            place(id, 580, 184 + n as i32 * 56, (w - 640).clamp(160, 300), 220);
        }
    }
}
pub unsafe fn paint(app: &App, dc: HDC, rect: RECT) -> bool {
    if app.game.page.is_none() || app.player.is_none() {
        return false;
    }
    ui::fill(dc, rect, app.palette.window);
    let px = |n: f32| (n * app.scale).round() as i32;
    ui::fill(
        dc,
        RECT {
            right: px(280.),
            ..rect
        },
        app.palette.surface,
    );
    ui::text(
        dc,
        "RINVA PLAYER",
        RECT {
            left: px(28.),
            top: px(88.),
            right: px(260.),
            bottom: px(124.),
        },
        app.palette.muted,
        app.resources.small,
        DT_SINGLELINE,
    );
    if app.game.page == Some(Page::Home) {
        let area = RECT {
            left: px(300.),
            top: px(184.),
            right: rect.right - px(40.),
            bottom: rect.bottom - px(140.),
        };
        ui::rounded(dc, area, app.palette.field, app.palette.border, px(4.));
        if let Some(image) = app.background.as_ref() {
            image.draw(dc, area, false);
        } else {
            ui::text(
                dc,
                app.t("Ваша история начинается здесь"),
                RECT {
                    left: area.left + px(24.),
                    top: area.top + px(32.),
                    right: area.right - px(24.),
                    bottom: area.bottom - px(24.),
                },
                app.palette.text,
                app.resources.title,
                DT_WORDBREAK,
            );
        }
    }
    true
}
pub unsafe fn draw_slot(app: &App, draw: &DRAWITEMSTRUCT) -> bool {
    if draw.CtlID as u16 != SLOTS {
        return false;
    }
    if draw.itemID == u32::MAX {
        return true;
    }
    let i = draw.itemID as usize;
    let r = draw.rcItem;
    let px = |n: f32| (n * app.scale).round() as i32;
    ui::fill(
        draw.hDC,
        r,
        if draw.itemState & ODS_SELECTED != 0 {
            app.palette.selected
        } else {
            app.palette.field
        },
    );
    let image_area = RECT {
        left: r.left + px(8.),
        top: r.top + px(8.),
        right: r.left + px(128.),
        bottom: r.bottom - px(8.),
    };
    ui::rounded(
        draw.hDC,
        image_area,
        app.palette.surface,
        app.palette.border,
        px(4.),
    );
    if let Some(Some(image)) = app.game.thumbnails.get(i) {
        image.draw(draw.hDC, image_area, false);
    }
    let n = SendMessageW(draw.hwndItem, LB_GETTEXTLEN, i, 0);
    if n >= 0 {
        let mut buf = vec![0u16; n as usize + 1];
        SendMessageW(draw.hwndItem, LB_GETTEXT, i, buf.as_mut_ptr() as isize);
        let value = String::from_utf16_lossy(&buf[..n as usize]);
        let (name, detail) = value.split_once('\n').unwrap_or((&value, ""));
        for (text, top, font, color) in [
            (name, r.top + px(12.), app.resources.font, app.palette.text),
            (
                detail,
                r.top + px(42.),
                app.resources.small,
                app.palette.muted,
            ),
        ] {
            ui::text(
                draw.hDC,
                text,
                RECT {
                    left: r.left + px(144.),
                    top,
                    right: r.right - px(12.),
                    bottom: top + px(26.),
                },
                color,
                font,
                DT_SINGLELINE | DT_END_ELLIPSIS,
            );
        }
    }
    if draw.itemState & ODS_FOCUS != 0 {
        DrawFocusRect(draw.hDC, &r);
    }
    true
}
fn latest(app: &App) -> Option<usize> {
    app.game
        .records
        .iter()
        .enumerate()
        .filter_map(|(i, r)| {
            r.as_ref()
                .ok()
                .and_then(Option::as_ref)
                .filter(|s| s.compatible(&app.document.project))
                .map(|s| (i, s.saved_at))
        })
        .max_by_key(|v| v.1)
        .map(|v| v.0)
}
unsafe fn load_slot(app: &mut App, index: usize) -> Result<(), String> {
    if app.game.writer.is_some() {
        return Err("Дождитесь завершения сохранения.".into());
    }
    let slot = Slot::at(index).ok_or("Неизвестный слот сохранения.")?;
    let record = saves::read(&root(), &app.document.project.id, slot)?;
    let next = if let Some(record) = record {
        saves::restore(&app.document.project, record)?
    } else if slot == Slot::Quick && app.game_save_path().exists() {
        let mut p = Player::new(&app.document.project)?;
        p.restore(storage::read_json(&app.game_save_path())?)?;
        p
    } else {
        return Err("В этом слоте нет сохранения.".into());
    };
    app.player = Some(next);
    app.game.session = true;
    app.reader.reset();
    reader_ui::restore_read(app);
    app.status = "Прогресс восстановлен.".into();
    // Reset frame timers after a load; do not add the time spent in a previous menu.
    app.game.paused_at = None;
    resume(app);
    mark_progress(app);
    Ok(())
}
pub unsafe fn save_slot(app: &mut App, slot: Slot) -> Result<(), String> {
    if app.game.writer.is_some() {
        return Err("Дождитесь завершения сохранения.".into());
    }
    if !app.game.session {
        return Ok(());
    }
    let Some(player) = &app.player else {
        return Ok(());
    };
    let thumbnail = paint::thumbnail(app)?;
    let record = Record::capture(&app.document.project, player, thumbnail);
    let (tx, rx) = mpsc::channel();
    let root = root();
    std::thread::spawn(move || {
        let _ = tx.send(saves::write(&root, slot, &record));
    });
    app.game.save_error = None;
    app.game.writer = Some(rx);
    app.game.writing_slot = Some(slot);
    app.status = "Запись сохранения…".into();
    if slot == Slot::Auto {
        app.game.autosave_due = false;
        app.game.autosave_at = Instant::now();
    }
    Ok(())
}
pub unsafe fn tick(app: &mut App) {
    if let Some(result) = app.game.writer.as_ref().and_then(|r| r.try_recv().ok()) {
        app.game.writer = None;
        let slot = app.game.writing_slot.take();
        match result {
            Ok(()) => {
                app.status = "Прогресс сохранён.".into();
                if app.game.page.is_some() {
                    scan(app);
                }
            }
            Err(error) => {
                app.game.save_error = Some(error.clone());
                if slot == Some(Slot::Auto) {
                    app.game.autosave_due = true;
                    app.game.autosave_at = Instant::now();
                }
                app.status = error;
                app.game.closing = false;
            }
        }
        app.refresh();
        if app.game.closing {
            PostMessageW(app.hwnd, APP_CLOSE, 0, 0);
        }
    }
    if let Some(records) = app.game.scan.as_ref().and_then(|r| r.try_recv().ok()) {
        app.game.scan = None;
        app.game.thumbnails = records
            .iter()
            .map(|r| {
                r.as_ref()
                    .ok()
                    .and_then(Option::as_ref)
                    .and_then(|s| paint::Bitmap::from_png(&s.thumbnail).ok())
            })
            .collect();
        app.game.records = records;
        app.refresh();
    }
    if app.game.session
        && app.player.is_some()
        && app.game.page.is_none()
        && app.game.autosave_due
        && app.game.writer.is_none()
        && app.game.autosave_at.elapsed() >= Duration::from_secs(10)
    {
        if let Err(error) = save_slot(app, Slot::Auto) {
            app.status = error;
            app.game.autosave_at = Instant::now();
            app.update_title();
        }
    }
}
pub unsafe fn prepare_close(app: &mut App) -> bool {
    if app.game.save_error.is_some() && app.game.autosave_due {
        return ui::message(
            app.hwnd,
            app.t("Не удалось сохранить"),
            app.t("Автосохранение не удалось. Выйти без сохранения прогресса?"),
            MB_YESNO | MB_ICONWARNING,
        ) == IDYES;
    }
    if app.game.writer.is_some() {
        app.game.closing = true;
        return false;
    }
    if app.game.autosave_due && app.game.session && app.player.is_some() {
        match save_slot(app, Slot::Auto) {
            Ok(()) => {
                app.game.closing = true;
                return false;
            }
            Err(error) => {
                app.status = error;
                app.update_title();
                return ui::message(
                    app.hwnd,
                    app.t("Не удалось сохранить"),
                    app.t("Автосохранение не удалось. Выйти без сохранения прогресса?"),
                    MB_YESNO | MB_ICONWARNING,
                ) == IDYES;
            }
        }
    }
    true
}
pub unsafe fn toggle_fullscreen(app: &mut App) -> Result<(), String> {
    if let Some((placement, style)) = app.game.fullscreen.take() {
        SetWindowLongPtrW(app.hwnd, GWL_STYLE, style);
        SetWindowPlacement(app.hwnd, &placement);
        SetWindowPos(
            app.hwnd,
            null_mut(),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
        );
    } else {
        let mut placement: WINDOWPLACEMENT = std::mem::zeroed();
        placement.length = std::mem::size_of::<WINDOWPLACEMENT>() as u32;
        let mut monitor: MONITORINFO = std::mem::zeroed();
        monitor.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetWindowPlacement(app.hwnd, &mut placement) == 0
            || GetMonitorInfoW(
                MonitorFromWindow(app.hwnd, MONITOR_DEFAULTTONEAREST),
                &mut monitor,
            ) == 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let style = GetWindowLongPtrW(app.hwnd, GWL_STYLE);
        app.game.fullscreen = Some((placement, style));
        SetWindowLongPtrW(
            app.hwnd,
            GWL_STYLE,
            (style as u32 & !WS_OVERLAPPEDWINDOW | WS_POPUP) as isize,
        );
        let r = monitor.rcMonitor;
        SetWindowPos(
            app.hwnd,
            HWND_TOP,
            r.left,
            r.top,
            r.right - r.left,
            r.bottom - r.top,
            SWP_FRAMECHANGED,
        );
    }
    app.refresh();
    Ok(())
}
pub unsafe fn command(app: &mut App, id: u16, notification: u16) -> Result<bool, String> {
    if !(MENU..=TEXT_SIZE).contains(&id) && ![GAME_SAVE, GAME_LOAD].contains(&id) {
        return Ok(false);
    }
    if app.player.is_none() {
        return Ok(true);
    }
    if [STYLE, TEXT_SIZE].contains(&id) {
        match notification as u32 {
            CBN_SELENDCANCEL => {
                app.refresh();
                return Ok(true);
            }
            CBN_SELENDOK => {}
            CBN_SELCHANGE if SendMessageW(app.controls.get(id), CB_GETDROPPEDSTATE, 0, 0) == 0 => {}
            _ => return Ok(true),
        }
        let n = app.controls.selection(id, true).unwrap_or(0) as u8;
        if id == STYLE {
            app.reader.settings.dialog_style = n.min(1);
        } else {
            app.reader.settings.text_size = n.min(2);
            app.rebuild_resources();
        }
        app.persist_settings()?;
        app.refresh();
        return Ok(true);
    }
    match id {
        MENU => {
            if app.game.page.is_some() && app.game.session {
                resume(app);
            } else {
                open(app, Page::Home);
            }
        }
        FULLSCREEN => toggle_fullscreen(app)?,
        RESUME => {
            if app.game.session {
                resume(app);
            } else if let Some(i) = latest(app) {
                load_slot(app, i)?;
            } else if app.game_save_path().is_file() {
                load_slot(app, 1)?;
            }
        }
        NEW_GAME => {
            if app.game.writer.is_some() {
                return Ok(true);
            }
            if app.game.session
                && ui::message(
                    app.hwnd,
                    app.t("Новая игра"),
                    app.t("Начать заново? Ручные сохранения останутся."),
                    MB_YESNO | MB_ICONQUESTION,
                ) != IDYES
            {
                return Ok(true);
            }
            app.player = Some(Player::new(&app.document.project)?);
            app.game.session = true;
            app.reader.reset();
            app.game.paused_at = None;
            reader_ui::restore_read(app);
            resume(app);
            mark_progress(app);
        }
        SAVE_PAGE => {
            if app.game.session {
                open(app, Page::Save);
            }
        }
        LOAD_PAGE => open(app, Page::Load),
        OPTIONS => open(app, Page::Settings),
        RETURN => {
            if app.game.page != Some(Page::Home) {
                open(app, Page::Home);
            } else if app.game.session {
                resume(app);
            }
        }
        EXIT => {
            if app.game.writer.is_some() {
                return Err("Дождитесь завершения сохранения.".into());
            }
            if app.player_only {
                PostMessageW(app.hwnd, WM_CLOSE, 0, 0);
            } else {
                app.game.page = None;
                app.game.paused_at = None;
                app.command(STOP, 0)?;
            }
        }
        SLOTS => {
            if notification as u32 == LBN_SELCHANGE {
                app.game.selected = app.controls.selection(SLOTS, false).unwrap_or(0);
                app.refresh();
            }
        }
        APPLY => {
            if app.game.page == Some(Page::Save) {
                if app.game.selected < 2 {
                    return Ok(true);
                }
                let slot = Slot::at(app.game.selected).ok_or("Неизвестный слот сохранения.")?;
                if saves::path(&root(), &app.document.project.id, slot)?.exists()
                    && ui::message(
                        app.hwnd,
                        app.t("Сохранения"),
                        app.t("Перезаписать выбранное сохранение?"),
                        MB_YESNO | MB_ICONQUESTION,
                    ) != IDYES
                {
                    return Ok(true);
                }
                save_slot(app, slot)?;
                app.refresh();
            } else {
                load_slot(app, app.game.selected)?;
            }
        }
        GAME_SAVE => {
            reader_ui::remember(app);
            save_slot(app, Slot::Quick)?;
            app.refresh();
        }
        GAME_LOAD => load_slot(app, 1)?,
        _ => {}
    }
    Ok(true)
}
fn date(seconds: u64) -> String {
    // Convert UTC Unix time to local Windows time without adding a date/time dependency.
    unsafe {
        let ticks = seconds
            .saturating_add(11_644_473_600)
            .saturating_mul(10_000_000);
        let file = FILETIME {
            dwLowDateTime: ticks as u32,
            dwHighDateTime: (ticks >> 32) as u32,
        };
        let mut utc: SYSTEMTIME = std::mem::zeroed();
        let mut local: SYSTEMTIME = std::mem::zeroed();
        if windows_sys::Win32::System::Time::FileTimeToSystemTime(&file, &mut utc) != 0
            && windows_sys::Win32::System::Time::SystemTimeToTzSpecificLocalTime(
                null(),
                &utc,
                &mut local,
            ) != 0
        {
            format!(
                "{:02}.{:02}.{} {:02}:{:02}",
                local.wDay, local.wMonth, local.wYear, local.wHour, local.wMinute
            )
        } else {
            seconds.to_string()
        }
    }
}
