mod paint;
mod ui;

use rinva_core::{storage, Choice, Document, Frame, Line, Player, Project, Severity};
use serde::{Deserialize, Serialize};
use std::{
    cell::{Cell, RefCell},
    ffi::OsString,
    path::{Path, PathBuf},
    ptr::{null, null_mut},
};
use ui::{wide, Controls, Palette, Resources};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::*,
    UI::{Controls::*, HiDpi::*, Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};

const NEW: u16 = 101;
const OPEN: u16 = 102;
const SAVE: u16 = 103;
const SAVE_AS: u16 = 104;
const UNDO: u16 = 105;
const REDO: u16 = 106;
const THEME: u16 = 107;
const EXPORT: u16 = 108;
const PLAY: u16 = 109;
const PROJECT_TITLE: u16 = 201;
const SCENES: u16 = 202;
const ADD_SCENE: u16 = 203;
const DUP_SCENE: u16 = 204;
const DELETE_SCENE: u16 = 205;
const START_SCENE: u16 = 206;
const SCENE_NAME: u16 = 301;
const BACKGROUND: u16 = 302;
const CLEAR_BG: u16 = 303;
const PORTRAIT: u16 = 304;
const CLEAR_PORTRAIT: u16 = 305;
const SPEAKER: u16 = 310;
const BODY: u16 = 311;
const TARGET: u16 = 312;
const NEXT_SCENE: u16 = 313;
const TAB_LINES: u16 = 401;
const TAB_CHOICES: u16 = 402;
const ITEMS: u16 = 403;
const ADD_ITEM: u16 = 404;
const DELETE_ITEM: u16 = 405;
const UP: u16 = 406;
const DOWN: u16 = 407;
const PREVIEW_SCENE: u16 = 408;
const CANVAS: u16 = 500;
const STATUS: u16 = 501;
const BACK: u16 = 601;
const ADVANCE: u16 = 602;
const STOP: u16 = 603;
const GAME_SAVE: u16 = 604;
const GAME_LOAD: u16 = 605;
const GAME_CHOICES: u16 = 606;
const TRANSCRIPT: u16 = 607;
const LABEL_PROJECT: u16 = 701;
const LABEL_SCENES: u16 = 702;
const LABEL_PREVIEW: u16 = 703;
const LABEL_SCENE: u16 = 704;
const LABEL_SPEAKER: u16 = 705;
const LABEL_BODY: u16 = 706;
const LABEL_TARGET: u16 = 707;
const LABEL_NEXT: u16 = 708;
const APP_COMMAND: u32 = WM_APP + 1;
const APP_LAYOUT: u32 = WM_APP + 2;
const APP_CLOSE: u32 = WM_APP + 3;

thread_local! {
    static APP:RefCell<Option<App>>=const { RefCell::new(None) };
    static SYNCING:Cell<bool>=const { Cell::new(false) };
}
struct SyncGuard(bool);
impl SyncGuard {
    fn new() -> Self {
        Self(SYNCING.with(|flag| flag.replace(true)))
    }
}
impl Drop for SyncGuard {
    fn drop(&mut self) {
        SYNCING.with(|flag| flag.set(self.0));
    }
}

#[derive(Serialize, Deserialize)]
struct Settings {
    dark: bool,
}

struct App {
    hwnd: HWND,
    controls: Controls,
    resources: Resources,
    palette: Palette,
    scale: f32,
    dark: bool,
    document: Document,
    file: Option<PathBuf>,
    selected_scene: usize,
    selected_item: usize,
    choices_tab: bool,
    player: Option<Player>,
    player_only: bool,
    status: String,
    background: Option<paint::Bitmap>,
    portrait: Option<paint::Bitmap>,
    image_key: String,
    export_result: Option<std::sync::mpsc::Receiver<Result<PathBuf, String>>>,
}

fn settings_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Rinva")
}

impl App {
    unsafe fn new(
        hwnd: HWND,
        project: Project,
        file: Option<PathBuf>,
        player_only: bool,
    ) -> Result<Self, String> {
        let scale = GetDpiForWindow(hwnd) as f32 / 96.;
        let dark = storage::read_json::<Settings>(&settings_dir().join("settings.json"))
            .map(|s| s.dark)
            .unwrap_or(true);
        let palette = Palette::new(dark);
        let resources = Resources::new(scale, palette);
        let mut controls = Controls::new();
        let font = resources.font;
        for (id, label) in [
            (NEW, "Новый"),
            (OPEN, "Открыть"),
            (SAVE, "Сохранить"),
            (SAVE_AS, "Сохранить как"),
            (UNDO, "Отмена"),
            (REDO, "Повтор"),
            (THEME, "Тема"),
            (EXPORT, "Сборка"),
            (PLAY, "Запустить · F5"),
            (ADD_SCENE, "+ Сцена"),
            (DUP_SCENE, "Копия"),
            (DELETE_SCENE, "Удалить"),
            (START_SCENE, "Сделать началом"),
            (BACKGROUND, "Выбрать фон…"),
            (CLEAR_BG, "Убрать"),
            (PORTRAIT, "Персонаж…"),
            (CLEAR_PORTRAIT, "Убрать"),
            (TAB_LINES, "Реплики"),
            (TAB_CHOICES, "Развилки"),
            (ADD_ITEM, "+ Реплика"),
            (DELETE_ITEM, "Удалить"),
            (UP, "Выше"),
            (DOWN, "Ниже"),
            (PREVIEW_SCENE, "Отсюда"),
            (BACK, "Назад"),
            (ADVANCE, "Далее"),
            (STOP, "В редактор · Esc"),
            (GAME_SAVE, "Сохранить · F6"),
            (GAME_LOAD, "Загрузить · F9"),
        ] {
            controls.button(hwnd, id, label, font)?;
        }
        for (id, label) in [
            (LABEL_PROJECT, "НАЗВАНИЕ НОВЕЛЛЫ"),
            (LABEL_SCENES, "СЦЕНЫ"),
            (LABEL_PREVIEW, "ПРЕДПРОСМОТР"),
            (LABEL_SCENE, "НАЗВАНИЕ СЦЕНЫ"),
            (LABEL_SPEAKER, "Говорит"),
            (LABEL_BODY, "Текст реплики"),
            (LABEL_TARGET, "Перейти к сцене"),
            (LABEL_NEXT, "После сцены"),
            (STATUS, "Готово"),
        ] {
            controls.label(hwnd, id, label, resources.small)?;
        }
        for id in [PROJECT_TITLE, SCENE_NAME, SPEAKER] {
            controls.edit(hwnd, id, false, font)?;
        }
        controls.edit(hwnd, BODY, true, font)?;
        controls.edit(hwnd, TRANSCRIPT, true, font)?;
        SendMessageW(controls.get(TRANSCRIPT), EM_SETREADONLY, 1, 0);
        for id in [SCENES, ITEMS, GAME_CHOICES] {
            controls.list(hwnd, id, font)?;
        }
        for id in [TARGET, NEXT_SCENE] {
            controls.combo(hwnd, id, font)?;
        }
        controls.add(hwnd, CANVAS, "RinvaCanvas", "Предпросмотр сцены", 0, font)?;
        let player = if player_only {
            Some(Player::new(&project)?)
        } else {
            None
        };
        theme_title(hwnd, dark);
        SetTimer(hwnd, 1, 200, None);
        Ok(Self {
            hwnd,
            controls,
            resources,
            palette,
            scale,
            dark,
            document: Document::new(project),
            file,
            selected_scene: 0,
            selected_item: 0,
            choices_tab: false,
            player,
            player_only,
            status: "Создайте сцену, напишите реплики и нажмите F5.".into(),
            background: None,
            portrait: None,
            image_key: String::new(),
            export_result: None,
        })
    }
    fn root(&self) -> PathBuf {
        self.file
            .as_ref()
            .and_then(|path| path.parent())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    }
    fn scene(&self) -> &rinva_core::Scene {
        &self.document.project.scenes[self.selected_scene]
    }
    fn game_save_path(&self) -> PathBuf {
        settings_dir()
            .join("saves")
            .join(format!("{}.json", self.document.project.signature()))
    }

    unsafe fn layout(&self) {
        let mut rect: RECT = std::mem::zeroed();
        GetClientRect(self.hwnd, &mut rect);
        let w = (rect.right as f32 / self.scale) as i32;
        let h = (rect.bottom as f32 / self.scale) as i32;
        let place =
            |id, x, y, width, height| self.controls.place(id, x, y, width, height, self.scale);
        for (&id, &handle) in &self.controls.handles {
            let show = if self.player.is_some() {
                matches!(
                    id,
                    THEME
                        | CANVAS
                        | STATUS
                        | BACK
                        | ADVANCE
                        | GAME_SAVE
                        | GAME_LOAD
                        | GAME_CHOICES
                        | TRANSCRIPT
                ) || id == STOP && !self.player_only
            } else {
                !matches!(
                    id,
                    BACK | ADVANCE | STOP | GAME_SAVE | GAME_LOAD | GAME_CHOICES | TRANSCRIPT
                )
            };
            ShowWindow(handle, if show { SW_SHOW } else { SW_HIDE });
        }
        place(STATUS, 20, h - 28, w - 40, 20);
        if self.player.is_some() {
            place(THEME, w - 100, 20, 80, 32);
            place(STOP, 220, 20, 158, 32);
            place(CANVAS, 20, 72, w - 40, h - 296);
            place(TRANSCRIPT, 32, h - 210, (w - 80) / 2, 132);
            place(GAME_CHOICES, 48 + (w - 80) / 2, h - 210, (w - 80) / 2, 132);
            place(BACK, 32, h - 64, 94, 32);
            place(GAME_SAVE, 138, h - 64, 146, 32);
            place(GAME_LOAD, 296, h - 64, 146, 32);
            place(ADVANCE, w - 194, h - 64, 162, 32);
        } else {
            for (id, x, width) in [
                (NEW, 234, 62),
                (OPEN, 304, 74),
                (SAVE, 386, 92),
                (SAVE_AS, 486, 116),
                (UNDO, 614, 66),
                (REDO, 688, 66),
                (THEME, 766, 70),
                (EXPORT, 844, 86),
            ] {
                place(id, x, 20, width, 32);
            }
            place(PLAY, w - 158, 20, 138, 32);
            place(LABEL_PROJECT, 28, 98, 184, 20);
            place(PROJECT_TITLE, 28, 124, 184, 32);
            place(LABEL_SCENES, 28, 178, 184, 20);
            place(SCENES, 20, 206, 200, h - 374);
            place(START_SCENE, 28, h - 154, 184, 32);
            place(ADD_SCENE, 28, h - 114, 184, 32);
            place(DUP_SCENE, 28, h - 74, 88, 32);
            place(DELETE_SCENE, 124, h - 74, 88, 32);
            let right = w - 316;
            let center = 248;
            let cw = right - center - 16;
            let preview_h = (cw * 9 / 16).min(h - 360).max(180);
            place(LABEL_PREVIEW, center, 90, cw - 100, 20);
            place(PREVIEW_SCENE, center + cw - 94, 84, 94, 28);
            place(CANVAS, center, 120, cw, preview_h);
            let tabs_y = 120 + preview_h + 16;
            place(TAB_LINES, center, tabs_y, 108, 32);
            place(TAB_CHOICES, center + 116, tabs_y, 108, 32);
            place(ITEMS, center, tabs_y + 44, cw, h - tabs_y - 130);
            place(ADD_ITEM, center, h - 74, 112, 32);
            place(DELETE_ITEM, center + 120, h - 74, 88, 32);
            place(UP, center + 216, h - 74, 70, 32);
            place(DOWN, center + 294, h - 74, 70, 32);
            place(LABEL_SCENE, right + 16, 98, 268, 20);
            place(SCENE_NAME, right + 16, 124, 268, 32);
            place(BACKGROUND, right + 16, 174, 184, 32);
            place(CLEAR_BG, right + 208, 174, 76, 32);
            place(PORTRAIT, right + 16, 214, 184, 32);
            place(CLEAR_PORTRAIT, right + 208, 214, 76, 32);
            if self.choices_tab {
                self.controls.show(LABEL_SPEAKER, false);
                self.controls.show(SPEAKER, false);
                place(LABEL_BODY, right + 16, 274, 268, 20);
                place(BODY, right + 16, 300, 268, h - 510);
                place(LABEL_TARGET, right + 16, h - 194, 268, 20);
                place(TARGET, right + 16, h - 168, 268, 220);
            } else {
                self.controls.show(LABEL_TARGET, false);
                self.controls.show(TARGET, false);
                place(LABEL_SPEAKER, right + 16, 274, 268, 20);
                place(SPEAKER, right + 16, 300, 268, 32);
                place(LABEL_BODY, right + 16, 350, 268, 20);
                place(BODY, right + 16, 376, 268, h - 510);
            }
            place(LABEL_NEXT, right + 16, h - 114, 268, 20);
            place(NEXT_SCENE, right + 16, h - 88, 268, 220);
        }
        for id in [SCENES, ITEMS, GAME_CHOICES] {
            SendMessageW(
                self.controls.get(id),
                LB_SETITEMHEIGHT,
                0,
                (36. * self.scale) as isize,
            );
        }
        RedrawWindow(
            self.hwnd,
            null(),
            null_mut(),
            RDW_INVALIDATE | RDW_ALLCHILDREN,
        );
        InvalidateRect(self.controls.get(CANVAS), null(), 0);
    }

    unsafe fn refresh(&mut self) {
        let _guard = SyncGuard::new();
        self.selected_scene = self
            .selected_scene
            .min(self.document.project.scenes.len() - 1);
        let scene = self.scene();
        let count = if self.choices_tab {
            scene.choices.len()
        } else {
            scene.lines.len()
        };
        self.selected_item = self.selected_item.min(count.saturating_sub(1));
        self.controls
            .text(PROJECT_TITLE, &self.document.project.title);
        self.controls.text(SCENE_NAME, &self.scene().name);
        let scenes: Vec<_> = self
            .document
            .project
            .scenes
            .iter()
            .map(|scene| {
                format!(
                    "{}{}",
                    if scene.id == self.document.project.start_scene {
                        "▶  "
                    } else {
                        "     "
                    },
                    scene.name
                )
            })
            .collect();
        self.controls
            .fill_list(SCENES, &scenes, self.selected_scene, false);
        let scene = self.scene();
        let items: Vec<_> = if self.choices_tab {
            scene
                .choices
                .iter()
                .enumerate()
                .map(|(i, c)| format!("{:02}  {}", i + 1, c.text))
                .collect()
        } else {
            scene
                .lines
                .iter()
                .enumerate()
                .map(|(i, line)| {
                    format!(
                        "{:02}  {}{}{}",
                        i + 1,
                        line.speaker,
                        if line.speaker.is_empty() { "" } else { " · " },
                        line.text.replace(['\r', '\n'], " ")
                    )
                })
                .collect()
        };
        self.controls
            .fill_list(ITEMS, &items, self.selected_item, false);
        let text = if self.choices_tab {
            scene
                .choices
                .get(self.selected_item)
                .map(|c| c.text.as_str())
                .unwrap_or("")
        } else {
            scene
                .lines
                .get(self.selected_item)
                .map(|l| l.text.as_str())
                .unwrap_or("")
        };
        let speaker = scene
            .lines
            .get(self.selected_item)
            .map(|l| l.speaker.as_str())
            .unwrap_or("");
        self.controls.text(SPEAKER, speaker);
        self.controls.text(BODY, text);
        self.controls.text(
            LABEL_BODY,
            if self.choices_tab {
                "Текст варианта ответа"
            } else {
                "Текст реплики"
            },
        );
        self.controls.text(
            ADD_ITEM,
            if self.choices_tab {
                "+ Ответ"
            } else {
                "+ Реплика"
            },
        );
        self.controls.text(
            BACKGROUND,
            if scene.background.is_some() {
                "Заменить фон…"
            } else {
                "Выбрать фон…"
            },
        );
        self.controls.text(
            PORTRAIT,
            if scene.portrait.is_some() {
                "Заменить персонажа…"
            } else {
                "Персонаж…"
            },
        );
        self.controls.enable(CLEAR_BG, scene.background.is_some());
        self.controls
            .enable(CLEAR_PORTRAIT, scene.portrait.is_some());
        let mut destinations = vec!["Конец истории".to_owned()];
        destinations.extend(self.document.project.scenes.iter().map(|s| s.name.clone()));
        let next = scene
            .next
            .as_ref()
            .and_then(|id| {
                self.document
                    .project
                    .scenes
                    .iter()
                    .position(|s| &s.id == id)
            })
            .map(|n| n + 1)
            .unwrap_or(0);
        self.controls
            .fill_list(NEXT_SCENE, &destinations, next, true);
        self.controls.enable(NEXT_SCENE, scene.choices.is_empty());
        let target = scene
            .choices
            .get(self.selected_item)
            .and_then(|c| {
                self.document
                    .project
                    .scenes
                    .iter()
                    .position(|s| s.id == c.target)
            })
            .unwrap_or(usize::MAX);
        self.controls
            .fill_list(TARGET, &destinations[1..], target, true);
        self.controls
            .enable(START_SCENE, scene.id != self.document.project.start_scene);
        for id in [SPEAKER, BODY, TARGET, DELETE_ITEM] {
            self.controls.enable(id, count > 0);
        }
        self.controls
            .enable(UP, self.selected_item > 0 && count > 0);
        self.controls.enable(DOWN, self.selected_item + 1 < count);
        self.controls
            .enable(DELETE_SCENE, self.document.project.scenes.len() > 1);
        self.controls.enable(UNDO, self.document.can_undo());
        self.controls.enable(REDO, self.document.can_redo());
        self.controls.enable(EXPORT, self.export_result.is_none());
        self.controls.text(
            THEME,
            if self.dark {
                "Светлая"
            } else {
                "Тёмная"
            },
        );
        if let Some(player) = &self.player {
            self.controls.enable(BACK, player.can_go_back());
            let (text, choices, label) = match player.frame() {
                Frame::Dialogue { line, .. } => (
                    format!(
                        "{}{}{}",
                        line.speaker,
                        if line.speaker.is_empty() {
                            ""
                        } else {
                            "\r\n\r\n"
                        },
                        line.text
                    ),
                    vec![],
                    "Далее · Enter",
                ),
                Frame::Choices { choices, .. } => (
                    "Выберите ответ. Стрелки — выбор, Enter — подтвердить.".into(),
                    choices.iter().map(|c| c.text.clone()).collect(),
                    "Выбрать · Enter",
                ),
                Frame::End { .. } => (
                    "Конец истории. Спасибо за чтение.".into(),
                    vec![],
                    "Начать заново",
                ),
            };
            self.controls.text(TRANSCRIPT, &text);
            self.controls.fill_list(GAME_CHOICES, &choices, 0, false);
            self.controls.text(ADVANCE, label);
        }
        self.load_images();
        self.update_title();
        self.layout();
    }

    unsafe fn update_title(&self) {
        let modified = if self.document.dirty() { " •" } else { "" };
        SetWindowTextW(
            self.hwnd,
            wide(&format!(
                "{}{} — Rinva{}",
                self.document.project.title,
                modified,
                if self.player_only {
                    " Player"
                } else {
                    " Studio"
                }
            ))
            .as_ptr(),
        );
        self.controls.text(STATUS, &self.status);
        InvalidateRect(self.controls.get(CANVAS), null(), 0);
    }

    unsafe fn load_images(&mut self) {
        let (bg, portrait) = if let Some(player) = &self.player {
            let scene = match player.frame() {
                Frame::Dialogue { scene, .. }
                | Frame::Choices { scene, .. }
                | Frame::End { scene } => scene,
            };
            (scene.background.clone(), scene.portrait.clone())
        } else {
            (
                self.scene().background.clone(),
                self.scene().portrait.clone(),
            )
        };
        let key = format!("{:?}|{:?}|{:?}", self.root(), bg, portrait);
        if self.image_key == key {
            return;
        }
        self.image_key = key;
        self.background = None;
        self.portrait = None;
        for (name, is_portrait) in [(bg, false), (portrait, true)] {
            if let Some(name) = name {
                match storage::resolve_asset(&self.root(), &name)
                    .and_then(|p| paint::Bitmap::load(&p))
                {
                    Ok(bitmap) => {
                        if is_portrait {
                            self.portrait = Some(bitmap)
                        } else {
                            self.background = Some(bitmap)
                        }
                    }
                    Err(error) => self.status = error,
                }
            }
        }
    }

    unsafe fn edit_changed(&mut self, id: u16) {
        if self.player.is_some() {
            return;
        }
        self.document.begin_edit();
        let text = self.controls.read(id);
        let scene = &mut self.document.project.scenes[self.selected_scene];
        match id {
            PROJECT_TITLE => self.document.project.title = text,
            SCENE_NAME => scene.name = text,
            SPEAKER => {
                if let Some(line) = scene.lines.get_mut(self.selected_item) {
                    line.speaker = text
                }
            }
            BODY => {
                if self.choices_tab {
                    if let Some(choice) = scene.choices.get_mut(self.selected_item) {
                        choice.text = text
                    }
                } else if let Some(line) = scene.lines.get_mut(self.selected_item) {
                    line.text = text
                }
            }
            _ => {}
        }
        self.status = "Есть несохранённые изменения · Ctrl+S — сохранить".into();
        self.controls.enable(UNDO, self.document.can_undo());
        self.update_title();
    }

    unsafe fn save(&mut self, save_as: bool) -> Result<bool, String> {
        self.document.finish_edit();
        let path = if save_as || self.file.is_none() {
            let initial = self
                .file
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Новелла.rinva".into());
            let Some(path) =
                ui::file_dialog(self.hwnd, true, false, &initial, "Сохранить новеллу")?
            else {
                return Ok(false);
            };
            path
        } else {
            self.file.clone().unwrap()
        };
        let root = path
            .parent()
            .ok_or("Не удалось определить папку проекта.")?;
        if self.file.as_ref() != Some(&path) && !storage::assets(&self.document.project).is_empty()
        {
            storage::copy_assets(&self.document.project, &self.root(), root)?;
        }
        storage::write_json(&path, &self.document.project)?;
        self.file = Some(path);
        self.document.mark_saved();
        self.status = "Проект сохранён.".into();
        self.update_title();
        Ok(true)
    }

    unsafe fn may_replace(&mut self) -> Result<bool, String> {
        self.document.finish_edit();
        if !self.document.dirty() {
            return Ok(true);
        }
        match ui::message(
            self.hwnd,
            "Несохранённые изменения",
            "Сохранить изменения в новелле перед продолжением?",
            MB_YESNOCANCEL | MB_ICONQUESTION,
        ) {
            IDYES => self.save(false),
            IDNO => Ok(true),
            _ => Ok(false),
        }
    }

    unsafe fn command(&mut self, id: u16, notification: u16) -> Result<(), String> {
        if matches!(id, PROJECT_TITLE | SCENE_NAME | SPEAKER | BODY) {
            match notification as u32 {
                EN_SETFOCUS => {
                    self.document.finish_edit();
                    self.document.begin_edit();
                }
                EN_CHANGE => self.edit_changed(id),
                EN_KILLFOCUS => self.document.finish_edit(),
                _ => {}
            }
            return Ok(());
        }
        if matches!(id, SCENES | ITEMS | GAME_CHOICES)
            && notification as u32 != LBN_SELCHANGE
            && notification as u32 != LBN_DBLCLK
        {
            return Ok(());
        }
        if matches!(id, TARGET | NEXT_SCENE) && notification as u32 != CBN_SELCHANGE {
            return Ok(());
        }
        self.document.finish_edit();
        let index = self.selected_scene;
        let item = self.selected_item;
        match id {
            NEW => {
                if self.may_replace()? {
                    self.document = Document::new(Project::new());
                    self.file = None;
                    self.selected_scene = 0;
                    self.selected_item = 0;
                    self.status = "Новая новелла. Начните с названия и первой реплики.".into();
                }
            }
            OPEN => {
                if self.may_replace()? {
                    if let Some(path) =
                        ui::file_dialog(self.hwnd, false, false, "", "Открыть новеллу")?
                    {
                        let project = storage::read_project(&path)?;
                        self.document = Document::new(project);
                        self.file = Some(path);
                        self.selected_scene = 0;
                        self.selected_item = 0;
                        self.status = "Проект открыт.".into();
                    }
                }
            }
            SAVE => {
                self.save(false)?;
            }
            SAVE_AS => {
                self.save(true)?;
            }
            UNDO => self.document.undo(),
            REDO => self.document.redo(),
            THEME => {
                self.dark = !self.dark;
                self.palette = Palette::new(self.dark);
                self.rebuild_resources();
                theme_title(self.hwnd, self.dark);
                std::fs::create_dir_all(settings_dir()).map_err(|e| e.to_string())?;
                storage::write_json(
                    &settings_dir().join("settings.json"),
                    &Settings { dark: self.dark },
                )?;
            }
            SCENES => {
                if let Some(i) = self.controls.selection(SCENES, false) {
                    self.selected_scene = i;
                    self.selected_item = 0;
                }
            }
            ITEMS => {
                if let Some(i) = self.controls.selection(ITEMS, false) {
                    self.selected_item = i;
                }
            }
            ADD_SCENE => {
                self.selected_scene = self.document.add_scene();
                self.selected_item = 0;
            }
            DUP_SCENE => {
                self.document.change(|project| {
                    let mut copy = project.scenes[index].clone();
                    copy.id = project.next_scene_id();
                    copy.name.push_str(" · копия");
                    project.scenes.push(copy);
                });
                self.selected_scene = self.document.project.scenes.len() - 1;
                self.selected_item = 0;
            }
            DELETE_SCENE => {
                if ui::message(
                    self.hwnd,
                    "Удалить сцену",
                    "Удалить выбранную сцену? Действие можно отменить.",
                    MB_YESNO | MB_ICONQUESTION,
                ) == IDYES
                {
                    self.document.delete_scene(index)?;
                    self.selected_scene = index.saturating_sub(1);
                    self.selected_item = 0;
                }
            }
            START_SCENE => self
                .document
                .change(|p| p.start_scene = p.scenes[index].id.clone()),
            TAB_LINES => {
                self.choices_tab = false;
                self.selected_item = 0;
            }
            TAB_CHOICES => {
                self.choices_tab = true;
                self.selected_item = 0;
            }
            ADD_ITEM => {
                let choices = self.choices_tab;
                self.document.change(|project| {
                    let target = project
                        .scenes
                        .iter()
                        .find(|s| s.id != project.scenes[index].id)
                        .unwrap_or(&project.scenes[index])
                        .id
                        .clone();
                    let scene = &mut project.scenes[index];
                    if choices {
                        scene.next = None;
                        scene.choices.push(Choice {
                            text: "Новый ответ".into(),
                            target,
                        });
                    } else {
                        scene.lines.push(Line {
                            speaker: String::new(),
                            text: "Новая реплика".into(),
                        });
                    }
                });
                self.selected_item = if choices {
                    self.scene().choices.len() - 1
                } else {
                    self.scene().lines.len() - 1
                };
            }
            DELETE_ITEM => {
                let choices = self.choices_tab;
                self.document.change(|p| {
                    let scene = &mut p.scenes[index];
                    if choices {
                        if item < scene.choices.len() {
                            scene.choices.remove(item);
                        }
                    } else if item < scene.lines.len() {
                        scene.lines.remove(item);
                    }
                });
                self.selected_item = item.saturating_sub(1);
            }
            UP | DOWN => {
                let choices = self.choices_tab;
                let count = if choices {
                    self.scene().choices.len()
                } else {
                    self.scene().lines.len()
                };
                let other = if id == UP {
                    item.saturating_sub(1)
                } else {
                    (item + 1).min(count.saturating_sub(1))
                };
                if item < count && other != item {
                    self.document.change(|p| {
                        if choices {
                            p.scenes[index].choices.swap(item, other)
                        } else {
                            p.scenes[index].lines.swap(item, other)
                        }
                    });
                    self.selected_item = other;
                }
            }
            TARGET => {
                if let Some(target) = self.controls.selection(TARGET, true) {
                    self.document.change(|p| {
                        let id = p.scenes[target].id.clone();
                        if let Some(choice) = p.scenes[index].choices.get_mut(item) {
                            choice.target = id;
                        }
                    });
                }
            }
            NEXT_SCENE => {
                if let Some(target) = self.controls.selection(NEXT_SCENE, true) {
                    self.document.change(|p| {
                        p.scenes[index].next = if target == 0 {
                            None
                        } else {
                            Some(p.scenes[target - 1].id.clone())
                        }
                    });
                }
            }
            BACKGROUND | PORTRAIT => {
                if self.file.is_none() && !self.save(false)? {
                    return Ok(());
                }
                if let Some(source) = ui::file_dialog(
                    self.hwnd,
                    false,
                    true,
                    "",
                    if id == BACKGROUND {
                        "Выберите фон"
                    } else {
                        "Выберите PNG персонажа"
                    },
                )? {
                    // Decode before copying, so unsupported/corrupt data never enters the document.
                    let _checked = paint::Bitmap::load(&source)?;
                    let asset = storage::import_asset(&self.root(), &source)?;
                    self.document.change(|p| {
                        if id == BACKGROUND {
                            p.scenes[index].background = Some(asset)
                        } else {
                            p.scenes[index].portrait = Some(asset)
                        }
                    });
                }
            }
            CLEAR_BG | CLEAR_PORTRAIT => self.document.change(|p| {
                if id == CLEAR_BG {
                    p.scenes[index].background = None
                } else {
                    p.scenes[index].portrait = None
                }
            }),
            PLAY | PREVIEW_SCENE => {
                let errors = self.document.project.validate();
                if let Some(issue) = errors.iter().find(|i| i.severity == Severity::Error) {
                    if let Some(scene) = &issue.scene_id {
                        if let Some(i) = self
                            .document
                            .project
                            .scenes
                            .iter()
                            .position(|s| &s.id == scene)
                        {
                            self.selected_scene = i;
                        }
                    }
                    self.refresh();
                    return Err(errors
                        .iter()
                        .filter(|i| i.severity == Severity::Error)
                        .map(|i| i.message.clone())
                        .collect::<Vec<_>>()
                        .join("\n"));
                }
                for asset in storage::assets(&self.document.project) {
                    storage::resolve_asset(&self.root(), asset)?;
                }
                self.player = Some(if id == PREVIEW_SCENE {
                    Player::from_scene(&self.document.project, &self.scene().id)?
                } else {
                    Player::new(&self.document.project)?
                });
                self.status =
                    "Enter — далее / выбрать · Назад — предыдущая реплика · Esc — редактор".into();
            }
            STOP => {
                if !self.player_only {
                    self.player = None;
                    self.status = "Редактор · Ctrl+S — сохранить · F5 — запустить".into();
                }
            }
            ADVANCE => {
                if let Some(player) = &mut self.player {
                    match player.frame() {
                        Frame::Dialogue { .. } => player.advance()?,
                        Frame::Choices { .. } => player
                            .choose(self.controls.selection(GAME_CHOICES, false).unwrap_or(0))?,
                        Frame::End { .. } => *player = Player::new(&self.document.project)?,
                    }
                }
            }
            BACK => {
                if let Some(player) = &mut self.player {
                    player.back();
                }
            }
            GAME_CHOICES => {
                if notification as u32 == LBN_DBLCLK {
                    return self.command(ADVANCE, 0);
                } else {
                    return Ok(());
                }
            }
            GAME_SAVE => {
                if let Some(player) = &self.player {
                    let path = self.game_save_path();
                    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
                    storage::write_json(&path, &player.save())?;
                    self.status = "Прогресс сохранён.".into();
                }
            }
            GAME_LOAD => {
                let save = storage::read_json(&self.game_save_path())?;
                if let Some(player) = &mut self.player {
                    player.restore(save)?;
                    self.status = "Прогресс восстановлен.".into();
                }
            }
            EXPORT => {
                if self.export_result.is_some() {
                    return Ok(());
                }
                self.document.project.check()?;
                if !self.save(false)? {
                    return Ok(());
                }
                if let Some(path) = ui::file_dialog(
                    self.hwnd,
                    true,
                    false,
                    "Моя новелла.rinva",
                    "Сборка: укажите новое имя; будет создана папка с этим именем",
                )? {
                    let destination = path.with_extension("build");
                    let project = self.document.project.clone();
                    let root = self.root();
                    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
                    let (sender, receiver) = std::sync::mpsc::channel();
                    self.export_result = Some(receiver);
                    std::thread::spawn(move || {
                        let result =
                            storage::export_bundle(&project, &root, &destination, &executable)
                                .map(|_| destination);
                        let _ = sender.send(result);
                    });
                    self.status = "Собираем новеллу…".into();
                }
            }
            _ => {}
        }
        self.refresh();
        if matches!(id, PLAY | PREVIEW_SCENE | ADVANCE | BACK | GAME_LOAD) {
            if let Some(player) = &self.player {
                let choice = matches!(player.frame(), Frame::Choices { .. });
                SetFocus(
                    self.controls
                        .get(if choice { GAME_CHOICES } else { ADVANCE }),
                );
            }
        } else if id == STOP {
            SetFocus(self.controls.get(ITEMS));
        }
        Ok(())
    }

    unsafe fn rebuild_resources(&mut self) {
        let resources = Resources::new(self.scale, self.palette);
        for (&id, &handle) in &self.controls.handles {
            SendMessageW(
                handle,
                WM_SETFONT,
                if id >= 700 || id == STATUS {
                    resources.small as usize
                } else {
                    resources.font as usize
                },
                1,
            );
        }
        self.resources = resources;
    }

    unsafe fn paint_frame(&self, dc: HDC, rect: RECT) {
        ui::fill(dc, rect, self.palette.window);
        let px = |v: i32| (v as f32 * self.scale).round() as i32;
        ui::fill(
            dc,
            RECT {
                left: 0,
                top: 0,
                right: rect.right,
                bottom: px(72),
            },
            self.palette.surface,
        );
        ui::text(
            dc,
            "RINVA",
            RECT {
                left: px(24),
                top: px(15),
                right: px(128),
                bottom: px(44),
            },
            self.palette.text,
            self.resources.title,
            DT_LEFT | DT_SINGLELINE,
        );
        ui::text(
            dc,
            if self.player_only {
                "PLAYER"
            } else {
                "STUDIO  /  0.1"
            },
            RECT {
                left: px(128),
                top: px(24),
                right: px(226),
                bottom: px(47),
            },
            self.palette.muted,
            self.resources.small,
            DT_LEFT | DT_SINGLELINE,
        );
        if self.player.is_none() {
            ui::fill(
                dc,
                RECT {
                    left: px(16),
                    top: px(84),
                    right: px(224),
                    bottom: rect.bottom - px(36),
                },
                self.palette.surface,
            );
            ui::fill(
                dc,
                RECT {
                    left: rect.right - px(316),
                    top: px(84),
                    right: rect.right - px(16),
                    bottom: rect.bottom - px(36),
                },
                self.palette.surface,
            );
        }
    }

    unsafe fn paint_canvas(&self, dc: HDC, rect: RECT) {
        let (scene, speaker, text, ended) = if let Some(player) = &self.player {
            match player.frame() {
                Frame::Dialogue { scene, line } => {
                    (scene, line.speaker.as_str(), line.text.as_str(), false)
                }
                Frame::Choices { scene, .. } => (scene, "", "Выберите ответ ниже", false),
                Frame::End { scene } => (scene, "", "", true),
            }
        } else {
            let scene = self.scene();
            let line = scene.lines.get(self.selected_item);
            (
                scene,
                line.map(|l| l.speaker.as_str()).unwrap_or(""),
                if self.choices_tab {
                    "Варианты ответа появятся после реплик сцены."
                } else {
                    line.map(|l| l.text.as_str())
                        .unwrap_or("Добавьте первую реплику кнопкой «+ Реплика».")
                },
                false,
            )
        };
        paint::scene(
            dc,
            rect,
            paint::SceneView {
                name: &scene.name,
                speaker,
                text,
                background: self.background.as_ref(),
                portrait: self.portrait.as_ref(),
                ended,
            },
            self.scale,
            self.resources.font,
            self.resources.small,
        );
    }

    unsafe fn draw_item(&self, draw: &DRAWITEMSTRUCT) {
        let id = draw.CtlID as u16;
        let selected = draw.itemState & ODS_SELECTED != 0;
        let disabled = draw.itemState & ODS_DISABLED != 0;
        let active =
            (id == TAB_LINES && !self.choices_tab) || (id == TAB_CHOICES && self.choices_tab);
        let primary = matches!(id, PLAY | ADVANCE);
        let mut pointer: POINT = std::mem::zeroed();
        GetCursorPos(&mut pointer);
        ScreenToClient(draw.hwndItem, &mut pointer);
        let hover = draw.CtlType != ODT_LISTBOX && PtInRect(&draw.rcItem, pointer) != 0;
        let color = if disabled {
            self.palette.surface
        } else if primary {
            self.palette.accent
        } else if selected || active || hover {
            self.palette.selected
        } else {
            self.palette.field
        };
        ui::fill(draw.hDC, draw.rcItem, color);
        if draw.CtlType != ODT_LISTBOX {
            let border = CreateSolidBrush(self.palette.border);
            FrameRect(draw.hDC, &draw.rcItem, border);
            DeleteObject(border);
        }
        let value = if draw.CtlType == ODT_LISTBOX {
            if draw.itemID == u32::MAX {
                return;
            }
            let n = SendMessageW(draw.hwndItem, LB_GETTEXTLEN, draw.itemID as usize, 0);
            if n < 0 {
                return;
            }
            let mut buffer = vec![0u16; n as usize + 1];
            SendMessageW(
                draw.hwndItem,
                LB_GETTEXT,
                draw.itemID as usize,
                buffer.as_mut_ptr() as isize,
            );
            String::from_utf16_lossy(&buffer[..n as usize])
        } else {
            ui::window_text(draw.hwndItem)
        };
        let mut rect = draw.rcItem;
        rect.left += (10. * self.scale) as i32;
        rect.right -= (8. * self.scale) as i32;
        ui::text(
            draw.hDC,
            &value,
            rect,
            if disabled {
                self.palette.muted
            } else if primary {
                self.palette.on_accent
            } else {
                self.palette.text
            },
            self.resources.font,
            DT_VCENTER
                | DT_SINGLELINE
                | DT_END_ELLIPSIS
                | if draw.CtlType == ODT_LISTBOX {
                    DT_LEFT
                } else {
                    DT_CENTER
                },
        );
        if draw.itemState & ODS_FOCUS != 0 {
            let mut focus = draw.rcItem;
            InflateRect(&mut focus, -3, -3);
            DrawFocusRect(draw.hDC, &focus);
        }
    }
}

unsafe fn theme_title(hwnd: HWND, dark: bool) {
    ui::theme_title(hwnd, dark);
}

/// Win32 can re-enter a callback while SendMessage/MessageBox is running. Commands
/// are queued, and RefCell borrows are never bypassed with aliased &mut pointers.
unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    std::panic::catch_unwind(|| dispatch(hwnd, msg, w, l)).unwrap_or_else(|_| {
        PostQuitMessage(1);
        0
    })
}
unsafe fn dispatch(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match msg {
        WM_ERASEBKGND => 1,
        WM_COMMAND => {
            if !SYNCING.with(Cell::get) {
                PostMessageW(hwnd, APP_COMMAND, w, l);
            }
            0
        }
        APP_COMMAND => {
            APP.with(|slot| {
                if let Ok(mut slot) = slot.try_borrow_mut() {
                    if let Some(app) = slot.as_mut() {
                        if let Err(error) =
                            app.command((w & 0xffff) as u16, ((w >> 16) & 0xffff) as u16)
                        {
                            app.status = error.clone();
                            app.update_title();
                            ui::message(hwnd, "Rinva", &error, MB_OK | MB_ICONERROR);
                        }
                    }
                }
            });
            0
        }
        WM_SIZE => {
            PostMessageW(hwnd, APP_LAYOUT, 0, 0);
            0
        }
        APP_LAYOUT => {
            APP.with(|slot| {
                if let Ok(slot) = slot.try_borrow() {
                    if let Some(app) = slot.as_ref() {
                        app.layout();
                    }
                }
            });
            0
        }
        WM_GETMINMAXINFO => {
            let info = &mut *(l as *mut MINMAXINFO);
            let scale = GetDpiForWindow(hwnd).max(96) as f32 / 96.;
            info.ptMinTrackSize.x = (1100. * scale) as i32;
            info.ptMinTrackSize.y = (760. * scale) as i32;
            0
        }
        WM_DPICHANGED => {
            let rect = &*(l as *const RECT);
            SetWindowPos(
                hwnd,
                null_mut(),
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            APP.with(|slot| {
                if let Ok(mut slot) = slot.try_borrow_mut() {
                    if let Some(app) = slot.as_mut() {
                        app.scale = (w & 0xffff) as f32 / 96.;
                        app.rebuild_resources();
                        app.layout();
                    }
                }
            });
            0
        }
        WM_PAINT | WM_PRINTCLIENT => {
            let mut paint: PAINTSTRUCT = std::mem::zeroed();
            let dc = if msg == WM_PAINT {
                BeginPaint(hwnd, &mut paint)
            } else {
                w as HDC
            };
            let mut rect: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rect);
            APP.with(|slot| {
                if let Ok(slot) = slot.try_borrow() {
                    if let Some(app) = slot.as_ref() {
                        app.paint_frame(dc, rect);
                    }
                }
            });
            if msg == WM_PAINT {
                EndPaint(hwnd, &paint);
            }
            0
        }
        WM_DRAWITEM => {
            APP.with(|slot| {
                if let Ok(slot) = slot.try_borrow() {
                    if let Some(app) = slot.as_ref() {
                        app.draw_item(&*(l as *const DRAWITEMSTRUCT));
                    }
                }
            });
            1
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => APP.with(|slot| {
            if let Ok(slot) = slot.try_borrow() {
                if let Some(app) = slot.as_ref() {
                    let dc = w as HDC;
                    SetTextColor(dc, app.palette.text);
                    let field =
                        msg != WM_CTLCOLORSTATIC || l as HWND == app.controls.get(TRANSCRIPT);
                    SetBkColor(
                        dc,
                        if field {
                            app.palette.field
                        } else {
                            app.palette.surface
                        },
                    );
                    return if field {
                        app.resources.field_brush
                    } else {
                        app.resources.surface_brush
                    } as isize;
                }
            }
            DefWindowProcW(hwnd, msg, w, l)
        }),
        WM_TIMER => {
            APP.with(|slot|if let Ok(mut slot)=slot.try_borrow_mut(){if let Some(app)=slot.as_mut(){
            if let Some(result)=app.export_result.as_ref().and_then(|rx|rx.try_recv().ok()){
                app.export_result=None;
                match result {Ok(path)=>{app.status=format!("Сборка готова: {}",path.display());ui::message(hwnd,"Сборка готова",&format!("Папка: {}\n\nЗапустите RinvaPlayer.exe. Для передачи игры скопируйте всю папку.",path.display()),MB_OK|MB_ICONINFORMATION);},Err(error)=>{app.status=error.clone();ui::message(hwnd,"Не удалось собрать новеллу",&error,MB_OK|MB_ICONERROR);}}
                app.refresh();
            }
        }});
            0
        }
        WM_CLOSE => {
            PostMessageW(hwnd, APP_CLOSE, 0, 0);
            0
        }
        APP_CLOSE => {
            let close = APP.with(|slot| {
                if let Ok(mut slot) = slot.try_borrow_mut() {
                    if let Some(app) = slot.as_mut() {
                        if app.export_result.is_some() {
                            ui::message(
                                hwnd,
                                "Сборка выполняется",
                                "Дождитесь завершения сборки перед закрытием.",
                                MB_OK,
                            );
                            return false;
                        }
                        return app.player_only
                            || match app.may_replace() {
                                Ok(value) => value,
                                Err(error) => {
                                    ui::message(
                                        hwnd,
                                        "Не удалось сохранить",
                                        &error,
                                        MB_OK | MB_ICONERROR,
                                    );
                                    false
                                }
                            };
                    }
                }
                true
            });
            if close {
                DestroyWindow(hwnd);
            }
            0
        }
        WM_DESTROY => {
            KillTimer(hwnd, 1);
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, w, l),
    }
}

unsafe extern "system" fn canvas_proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    std::panic::catch_unwind(|| {
        if msg == WM_ERASEBKGND {
            return 1;
        }
        if msg == WM_PAINT || msg == WM_PRINTCLIENT {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let dc = if msg == WM_PAINT {
                BeginPaint(hwnd, &mut ps)
            } else {
                w as HDC
            };
            let mut rect: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rect);
            let memory = CreateCompatibleDC(dc);
            let bitmap = CreateCompatibleBitmap(dc, rect.right.max(1), rect.bottom.max(1));
            let old = SelectObject(memory, bitmap);
            APP.with(|slot| {
                if let Ok(slot) = slot.try_borrow() {
                    if let Some(app) = slot.as_ref() {
                        app.paint_canvas(memory, rect);
                    }
                }
            });
            BitBlt(dc, 0, 0, rect.right, rect.bottom, memory, 0, 0, SRCCOPY);
            SelectObject(memory, old);
            DeleteObject(bitmap);
            DeleteDC(memory);
            if msg == WM_PAINT {
                EndPaint(hwnd, &ps);
            }
            return 0;
        }
        DefWindowProcW(hwnd, msg, w, l)
    })
    .unwrap_or_else(|_| {
        PostQuitMessage(1);
        0
    })
}

unsafe fn shortcut(message: &MSG) -> bool {
    if message.message != WM_KEYDOWN {
        return false;
    }
    let ctrl = GetKeyState(VK_CONTROL as i32) < 0;
    let shift = GetKeyState(VK_SHIFT as i32) < 0;
    let key = message.wParam as u16;
    let command = APP.with(|slot| {
        let slot = slot.borrow();
        let app = slot.as_ref()?;
        let focus = GetFocus();
        let text_entry = [PROJECT_TITLE, SCENE_NAME, SPEAKER, BODY, TRANSCRIPT]
            .iter()
            .any(|id| focus == app.controls.get(*id));
        let command = if app.player.is_some() {
            match key {
                VK_ESCAPE if !app.player_only => STOP,
                VK_F6 => GAME_SAVE,
                VK_F9 => GAME_LOAD,
                VK_RETURN
                    if focus == app.controls.get(GAME_CHOICES)
                        || focus == app.controls.get(ADVANCE)
                        || focus == app.hwnd =>
                {
                    ADVANCE
                }
                VK_SPACE if focus == app.hwnd => ADVANCE,
                _ => return None,
            }
        } else if ctrl {
            match key {
                0x53 => {
                    if shift {
                        SAVE_AS
                    } else {
                        SAVE
                    }
                }
                0x4f => OPEN,
                0x4e => NEW,
                0x5a if !text_entry => {
                    if shift {
                        REDO
                    } else {
                        UNDO
                    }
                }
                0x59 if !text_entry => REDO,
                _ => return None,
            }
        } else if key == VK_F5 {
            PLAY
        } else {
            return None;
        };
        Some((app.hwnd, command))
    });
    if let Some((hwnd, command)) = command {
        PostMessageW(hwnd, APP_COMMAND, command as usize, 0);
        true
    } else {
        false
    }
}

pub fn show_fatal(error: &str) {
    unsafe {
        ui::message(null_mut(), "Rinva", error, MB_OK | MB_ICONERROR);
    }
}

pub fn run(args: Vec<OsString>) -> Result<(), String> {
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let instance = GetModuleHandleW(null());
        let class_name = wide("RinvaStudioWindow");
        let mut class: WNDCLASSW = std::mem::zeroed();
        class.lpfnWndProc = Some(window_proc);
        class.hInstance = instance;
        class.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
        class.lpszClassName = class_name.as_ptr();
        if RegisterClassW(&class) == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        paint::register(instance, Some(canvas_proc))?;
        let executable = std::env::current_exe().map_err(|e| e.to_string())?;
        let standalone = executable
            .file_stem()
            .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("RinvaPlayer"));
        let play = args.first().is_some_and(|arg| arg == "--play") || standalone;
        let smoke = args.first().is_some_and(|arg| arg == "--smoke-test");
        let file = if standalone {
            Some(executable.parent().unwrap().join("story.rinva"))
        } else if play {
            Some(PathBuf::from(
                args.get(1).ok_or("Укажите файл после --play.")?,
            ))
        } else if !smoke {
            args.first().map(PathBuf::from)
        } else {
            None
        };
        let project = if let Some(path) = &file {
            storage::read_project(path)?
        } else {
            Project::demo()
        };
        let hwnd = CreateWindowExW(
            WS_EX_CONTROLPARENT,
            class_name.as_ptr(),
            wide("Rinva Studio").as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1340,
            900,
            null_mut(),
            null_mut(),
            instance,
            null(),
        );
        if hwnd.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let app = App::new(hwnd, project, file, play)?;
        APP.with(|slot| *slot.borrow_mut() = Some(app));
        APP.with(|slot| slot.borrow_mut().as_mut().unwrap().refresh());
        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);
        if smoke {
            let output = args
                .get(1)
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("smoke-output"));
            let result = smoke_test(hwnd, &output);
            DestroyWindow(hwnd);
            APP.with(|slot| *slot.borrow_mut() = None);
            return result;
        }
        let mut message: MSG = std::mem::zeroed();
        loop {
            let result = GetMessageW(&mut message, null_mut(), 0, 0);
            if result == -1 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            if result == 0 {
                break;
            }
            if shortcut(&message) {
                continue;
            }
            if IsDialogMessageW(hwnd, &message) == 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        APP.with(|slot| *slot.borrow_mut() = None);
        if message.wParam != 0 {
            return Err("Непредвиденная ошибка интерфейса.".into());
        }
        Ok(())
    }
}

unsafe fn pump() {
    let mut msg: MSG = std::mem::zeroed();
    while PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) != 0 {
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
}

unsafe fn smoke_test(hwnd: HWND, output: &Path) -> Result<(), String> {
    std::fs::create_dir_all(output).map_err(|e| e.to_string())?;
    pump();
    let command = |id: u16| {
        SendMessageW(hwnd, WM_COMMAND, id as usize, 0);
        pump();
    };
    let original = APP.with(|slot| slot.borrow().as_ref().unwrap().document.project.clone());
    command(ADD_SCENE);
    if APP.with(|slot| {
        slot.borrow()
            .as_ref()
            .unwrap()
            .document
            .project
            .scenes
            .len()
    }) != original.scenes.len() + 1
    {
        return Err("Native add scene failed".into());
    }
    command(UNDO);
    command(REDO);
    command(UNDO);
    let equal = APP.with(|slot| slot.borrow().as_ref().unwrap().document.project == original);
    if !equal {
        return Err("Native undo/redo failed".into());
    }
    APP.with(|slot| {
        let mut slot = slot.borrow_mut();
        let app = slot.as_mut().unwrap();
        app.selected_scene = 0;
        app.selected_item = 0;
        app.refresh();
    });
    pump();
    let speaker = APP.with(|slot| slot.borrow().as_ref().unwrap().controls.get(SPEAKER));
    SetFocus(speaker);
    pump();
    SetWindowTextW(speaker, wide("Проверка ввода — 海").as_ptr());
    pump();
    if APP.with(|slot| {
        slot.borrow().as_ref().unwrap().document.project.scenes[0].lines[0]
            .speaker
            .clone()
    }) != "Проверка ввода — 海"
    {
        return Err("Native Unicode editing notification failed".into());
    }
    SetFocus(hwnd);
    pump();
    command(UNDO);
    if APP.with(|slot| slot.borrow().as_ref().unwrap().document.project.clone()) != original {
        return Err("Native text session undo failed".into());
    }
    let original_dark = APP.with(|slot| slot.borrow().as_ref().unwrap().dark);
    if !original_dark {
        command(THEME);
    }
    command(THEME);
    UpdateWindow(hwnd);
    pump();
    paint::capture(hwnd, &output.join("editor-light.png"))?;
    command(THEME);
    UpdateWindow(hwnd);
    pump();
    paint::capture(hwnd, &output.join("editor-dark.png"))?;
    command(TAB_CHOICES);
    let target = APP.with(|slot| slot.borrow().as_ref().unwrap().controls.get(TARGET));
    SendMessageW(target, CB_SHOWDROPDOWN, 1, 0);
    pump();
    paint::capture(hwnd, &output.join("choices-dropdown.png"))?;
    SendMessageW(target, CB_SHOWDROPDOWN, 0, 0);
    pump();
    if APP.with(|slot| slot.borrow().as_ref().unwrap().document.project.clone()) != original {
        return Err("Dismissing a dropdown changed the document".into());
    }
    command(TAB_LINES);
    command(PLAY);
    command(ADVANCE);
    command(ADVANCE);
    command(ADVANCE);
    if !APP.with(|slot| {
        matches!(
            slot.borrow()
                .as_ref()
                .unwrap()
                .player
                .as_ref()
                .unwrap()
                .frame(),
            Frame::Choices { .. }
        )
    }) {
        return Err("Native playback did not reach choices".into());
    }
    UpdateWindow(hwnd);
    pump();
    paint::capture(hwnd, &output.join("player.png"))?;
    command(ADVANCE);
    command(BACK);
    command(STOP);
    let path = output.join("roundtrip.rinva");
    APP.with(|slot| storage::write_json(&path, &slot.borrow().as_ref().unwrap().document.project))?;
    if storage::read_project(&path)? != original {
        return Err("Native project roundtrip failed".into());
    }
    if !original_dark {
        command(THEME);
    }
    std::fs::write(output.join("native-smoke.txt"),"PASS: native window and controls, Unicode edit notifications and grouped undo, scene command, undo/redo, dropdown dismissal, live theme switch, playback choice and rollback, project roundtrip, screenshots.\n").map_err(|e|e.to_string())?;
    Ok(())
}
