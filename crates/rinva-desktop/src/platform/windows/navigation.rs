//! Native search and diagnostics share stable scene IDs and precise field locations.
use super::*;
use rinva_core::{
    search::{self, SearchScope},
    Field,
};
use std::{
    ops::Range,
    time::{Duration, Instant},
};

pub const SEARCH: u16 = 120;
pub const PROBLEMS: u16 = 121;
pub const QUERY: u16 = 122;
pub const FILTER: u16 = 123;
pub const RESULTS: u16 = 124;
pub const DETAIL: u16 = 125;
pub const GO: u16 = 126;
pub const RECHECK: u16 = 127;
pub const CLEAR: u16 = 128;
pub const CLOSE: u16 = 129;
pub const NEXT: u16 = 130;
pub const PREVIOUS: u16 = 131;
pub const SUMMARY: u16 = 709;
const LIMIT: usize = 200;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Search,
    Problems,
}
#[derive(Clone, PartialEq, Eq)]
struct Key {
    scene_id: Option<String>,
    field: Field,
    range: Option<Range<usize>>,
}
#[derive(Clone)]
struct Row {
    key: Key,
    source: String,
    label: String,
    detail: String,
}
#[derive(Default)]
pub struct Navigation {
    pub mode: Option<Mode>,
    search_query: String,
    problem_query: String,
    filter: usize,
    rows: Vec<Row>,
    changed_at: Option<Instant>,
    return_focus: u16,
    truncated: bool,
}
impl Navigation {
    fn query(&self) -> &str {
        if self.mode == Some(Mode::Problems) {
            &self.problem_query
        } else {
            &self.search_query
        }
    }
    pub fn changed(&mut self) {
        if self.mode.is_some() {
            self.changed_at = Some(Instant::now());
        }
    }
}

pub unsafe fn create(
    controls: &mut Controls,
    hwnd: HWND,
    resources: &Resources,
) -> Result<(), String> {
    for (id, label) in [
        (SEARCH, "Поиск"),
        (PROBLEMS, "Проверка"),
        (GO, "Перейти"),
        (RECHECK, "Обновить"),
        (CLEAR, "Очистить"),
        (CLOSE, "Закрыть"),
    ] {
        controls.button(hwnd, id, label, resources.font)?;
    }
    controls.edit(hwnd, QUERY, false, resources.font)?;
    controls.combo(hwnd, FILTER, resources.font)?;
    controls.list(hwnd, RESULTS, resources.font)?;
    controls.edit(hwnd, DETAIL, true, resources.font)?;
    SendMessageW(controls.get(DETAIL), EM_SETREADONLY, 1, 0);
    SendMessageW(controls.get(DETAIL), EM_SETLIMITTEXT, 16_384, 0);
    controls.label(hwnd, SUMMARY, "", resources.small)?;
    Ok(())
}

pub unsafe fn layout(app: &App, w: i32, h: i32) {
    let open = app.navigation.mode.is_some() && app.player.is_none();
    for id in [
        QUERY, FILTER, RESULTS, DETAIL, GO, RECHECK, CLEAR, CLOSE, SUMMARY,
    ] {
        app.controls.show(id, open);
    }
    if !open {
        return;
    }
    for id in [
        CANVAS,
        TAB_LINES,
        TAB_CHOICES,
        ITEMS,
        ADD_ITEM,
        DELETE_ITEM,
        UP,
        DOWN,
        GRAPH_UNLINK,
        GRAPH_ARRANGE,
        GRAPH_FIT,
        ZOOM_IN,
        ZOOM_OUT,
        ZOOM_LABEL,
        PREVIEW_SCENE,
    ] {
        app.controls.show(id, false);
    }
    let x = 248;
    let width = w - 580;
    for (id, left, top, width, height) in [
        (QUERY, x, 128, width - 94, 32),
        (CLEAR, x + width - 86, 128, 86, 32),
        (FILTER, x, 172, 184, 220),
        (SUMMARY, x + 198, 179, width - 198, 24),
        (RESULTS, x, 212, width, h - 420),
        (DETAIL, x, h - 194, width, 104),
        (GO, x, h - 74, 110, 32),
        (RECHECK, x + 118, h - 74, 110, 32),
        (CLOSE, x + width - 94, h - 74, 94, 32),
    ] {
        app.controls.place(id, left, top, width, height, app.scale);
    }
    SendMessageW(
        app.controls.get(RESULTS),
        LB_SETITEMHEIGHT,
        0,
        (56. * app.scale) as isize,
    );
}

fn field_name(app: &App, field: Field) -> String {
    match field {
        Field::Project => app.t("Проект").into(),
        Field::ProjectTitle => app.t("Название новеллы").into(),
        Field::Scene => app.t("Сцена").into(),
        Field::SceneName => app.t("Название сцены").into(),
        Field::Speaker(n) => format!("{} {:02}", app.t("Говорит"), n + 1),
        Field::Dialogue(n) => format!("{} {:02}", app.t("Реплика"), n + 1),
        Field::ChoiceText(n) | Field::ChoiceTarget(n) => format!("{} {:02}", app.t("Ответ"), n + 1),
        Field::NextScene => app.t("После сцены").into(),
        Field::Background => app.t("Фон").into(),
        Field::Portrait => app.t("Персонаж").into(),
    }
}
fn location(app: &App, key: &Key) -> String {
    let name = key
        .scene_id
        .as_deref()
        .and_then(|id| app.document.project.scene(id))
        .map(|s| s.name.as_str())
        .unwrap_or(&app.document.project.title);
    format!(
        "{} · {}",
        name.replace(['\r', '\n'], " "),
        field_name(app, key.field)
    )
}
fn row(app: &App, key: Key, message: Option<(Severity, String)>) -> Row {
    let source = search::field_text(&app.document.project, key.scene_id.as_deref(), key.field)
        .unwrap_or("")
        .to_owned();
    let location = location(app, &key);
    let (label, detail) = if let Some((severity, message)) = message {
        let severity = app.t(if severity == Severity::Error {
            "Ошибка"
        } else {
            "Предупреждение"
        });
        (
            format!(
                "{severity} · {location}\n{}",
                message.lines().next().unwrap_or("")
            ),
            format!(
                "{severity} · {location}\r\n\r\n{}",
                message.replace("\r\n", "\n").replace('\n', "\r\n")
            ),
        )
    } else {
        let excerpt = search::excerpt(&source, key.range.as_ref().unwrap(), 36);
        (
            format!("{location}\n{excerpt}"),
            format!(
                "{location}\r\n\r\n{}",
                source.replace("\r\n", "\n").replace('\n', "\r\n")
            ),
        )
    };
    Row {
        key,
        source,
        label,
        detail,
    }
}

pub unsafe fn refresh(app: &mut App) {
    let _guard = SyncGuard::new();
    for (id, label) in [
        (SEARCH, "Поиск"),
        (PROBLEMS, "Проверка"),
        (GO, "Перейти"),
        (RECHECK, "Обновить"),
        (CLEAR, "Очистить"),
        (CLOSE, "Закрыть"),
    ] {
        app.controls.text(id, app.t(label));
    }
    let Some(mode) = app.navigation.mode else {
        return;
    };
    if app.player.is_some() {
        return;
    }
    let previous = app
        .controls
        .selection(RESULTS, false)
        .and_then(|i| app.navigation.rows.get(i))
        .map(|r| r.key.clone());
    let query = app.navigation.query().to_owned();
    app.controls.text(QUERY, &query);
    let filters = if mode == Mode::Search {
        vec!["Везде", "Сцены", "Реплики", "Имена говорящих", "Ответы"]
    } else {
        vec!["Все проблемы", "Ошибки", "Предупреждения"]
    };
    app.navigation.filter = app.navigation.filter.min(filters.len() - 1);
    app.controls.fill_list(
        FILTER,
        &filters
            .iter()
            .map(|s| app.t(s).to_owned())
            .collect::<Vec<_>>(),
        app.navigation.filter,
        true,
    );
    SendMessageW(
        app.controls.get(QUERY),
        EM_SETCUEBANNER,
        0,
        wide(app.t(if mode == Mode::Search {
            "Найти в сценарии · Ctrl+F"
        } else {
            "Фильтр по сцене или сообщению"
        }))
        .as_ptr() as isize,
    );
    app.navigation.truncated = false;
    let (rows, summary) = if mode == Mode::Search {
        let scope = [
            SearchScope::All,
            SearchScope::Scenes,
            SearchScope::Dialogue,
            SearchScope::Speakers,
            SearchScope::Choices,
        ][app.navigation.filter];
        let result = search::search(&app.document.project, &query, scope, LIMIT);
        let summary = app.language.format(
            if result.truncated {
                "Первые {count} результатов · уточните запрос"
            } else {
                "Найдено: {count}"
            },
            &[("count", &result.hits.len().to_string())],
        );
        let rows = result
            .hits
            .into_iter()
            .map(|h| {
                row(
                    app,
                    Key {
                        scene_id: h.scene_id,
                        field: h.field,
                        range: Some(h.range),
                    },
                    None,
                )
            })
            .collect::<Vec<_>>();
        (rows, summary)
    } else {
        let issues = rinva_core::inspect_project(&app.document.project, &app.root());
        let errors = issues
            .iter()
            .filter(|i| i.severity == Severity::Error)
            .count();
        let warnings = issues.len() - errors;
        let summary = app.language.format(
            "Ошибки: {errors} · Предупреждения: {warnings}",
            &[
                ("errors", &errors.to_string()),
                ("warnings", &warnings.to_string()),
            ],
        );
        let query = query.trim().to_lowercase();
        let names: std::collections::HashMap<_, _> = app
            .document
            .project
            .scenes
            .iter()
            .map(|s| (s.id.as_str(), s.name.as_str()))
            .collect();
        let mut rows = issues
            .into_iter()
            .filter(|i| match app.navigation.filter {
                1 => i.severity == Severity::Error,
                2 => i.severity == Severity::Warning,
                _ => true,
            })
            .filter(|i| {
                query.is_empty()
                    || app
                        .language
                        .message(&i.message)
                        .to_lowercase()
                        .contains(&query)
                    || i.scene_id
                        .as_deref()
                        .and_then(|id| names.get(id))
                        .is_some_and(|name| name.to_lowercase().contains(&query))
            })
            .take(LIMIT + 1)
            .map(|i| {
                row(
                    app,
                    Key {
                        scene_id: i.scene_id,
                        field: i.field,
                        range: None,
                    },
                    Some((i.severity, app.language.message(&i.message))),
                )
            })
            .collect::<Vec<_>>();
        app.navigation.truncated = rows.len() > LIMIT;
        rows.truncate(LIMIT);
        (rows, summary)
    };
    let selected = previous
        .and_then(|key| rows.iter().position(|r| r.key == key))
        .unwrap_or(0);
    app.controls.fill_list(
        RESULTS,
        &rows.iter().map(|r| r.label.clone()).collect::<Vec<_>>(),
        if rows.is_empty() {
            usize::MAX
        } else {
            selected
        },
        false,
    );
    app.controls.text(SUMMARY, &summary);
    app.controls.enable(GO, !rows.is_empty());
    app.navigation.rows = rows;
    app.navigation.changed_at = None;
    update_detail(app);
}

unsafe fn update_detail(app: &App) {
    let detail = app
        .controls
        .selection(RESULTS, false)
        .and_then(|i| app.navigation.rows.get(i))
        .map(|r| r.detail.as_str())
        .unwrap_or_else(|| {
            app.t(if app.navigation.mode == Some(Mode::Search) {
                if app.navigation.query().trim().is_empty() {
                    "Введите текст. Enter — перейти к результату, F3 — следующий, Esc — закрыть."
                } else {
                    "Совпадений нет. Измените запрос или область поиска."
                }
            } else if app.navigation.filter == 0 && app.navigation.query().trim().is_empty() {
                "Проблем не найдено. Можно запускать и собирать новеллу."
            } else {
                "Нет проблем, соответствующих фильтру."
            })
        });
    let detail = if app.navigation.truncated {
        format!(
            "{}\r\n\r\n{detail}",
            app.t("Показаны первые 200 проблем. Уточните фильтр.")
        )
    } else {
        detail.to_owned()
    };
    app.controls.text(DETAIL, &detail);
}

pub unsafe fn open(app: &mut App, mode: Mode) {
    app.document.finish_edit();
    app.graph.cancel();
    if GetCapture() == app.controls.get(CANVAS) {
        ReleaseCapture();
    }
    if app.navigation.mode.is_none() {
        app.navigation.return_focus = app
            .controls
            .handles
            .iter()
            .find(|(_, h)| **h == GetFocus())
            .map(|(&id, _)| id)
            .unwrap_or(CANVAS);
    }
    if app.navigation.mode != Some(mode) {
        app.navigation.filter = 0;
    }
    app.navigation.mode = Some(mode);
    app.refresh();
    SetFocus(app.controls.get(QUERY));
    SendMessageW(app.controls.get(QUERY), EM_SETSEL, 0, -1);
}

pub unsafe fn preflight(app: &mut App) -> bool {
    if !rinva_core::inspect_project(&app.document.project, &app.root())
        .iter()
        .any(|i| i.severity == Severity::Error)
    {
        return false;
    }
    app.navigation.problem_query.clear();
    app.navigation.filter = 0;
    open(app, Mode::Problems);
    app.status = "Перед запуском исправьте ошибки в панели «Проверка».".into();
    app.update_title();
    SetFocus(app.controls.get(RESULTS));
    true
}

unsafe fn activate(app: &mut App, leave: bool) {
    let Some(row) = app
        .controls
        .selection(RESULTS, false)
        .and_then(|i| app.navigation.rows.get(i))
        .cloned()
    else {
        return;
    };
    if search::field_text(
        &app.document.project,
        row.key.scene_id.as_deref(),
        row.key.field,
    ) != Some(row.source.as_str())
    {
        refresh(app);
        app.status = "Результаты обновлены. Выберите нужную строку ещё раз.".into();
        app.update_title();
        return;
    }
    app.document.finish_edit();
    if let Some(id) = row.key.scene_id.as_deref() {
        let Some(i) = app.document.project.scenes.iter().position(|s| s.id == id) else {
            refresh(app);
            return;
        };
        app.selected_scene = i;
    }
    app.graph.selected_port = None;
    app.selected_item = 0;
    app.choices_tab = false;
    let control = match row.key.field {
        Field::Project => START_SCENE,
        Field::ProjectTitle => PROJECT_TITLE,
        Field::Scene | Field::SceneName => SCENE_NAME,
        Field::Speaker(i) => {
            app.selected_item = i;
            SPEAKER
        }
        Field::Dialogue(i) => {
            app.selected_item = i;
            BODY
        }
        Field::ChoiceText(i) | Field::ChoiceTarget(i) => {
            app.selected_item = i;
            app.choices_tab = true;
            app.graph.selected_port = Some((app.selected_scene, Port::Choice(i)));
            if row.key.field == Field::ChoiceText(i) {
                BODY
            } else {
                TARGET
            }
        }
        Field::NextScene => NEXT_SCENE,
        Field::Background => BACKGROUND,
        Field::Portrait => PORTRAIT,
    };
    if leave {
        app.navigation.mode = None;
    }
    app.refresh();
    if leave {
        if app.graph_mode {
            let p = graph::positions(&app.document.project)[app.selected_scene];
            let size = app.canvas_size();
            app.graph.pan = Point::new(
                size.x / 2. - (p.x + graph::NODE_WIDTH / 2.) * app.graph.zoom,
                size.y / 2. - (p.y + graph::height(app.scene()) / 2.) * app.graph.zoom,
            );
            InvalidateRect(app.controls.get(CANVAS), null(), 0);
        }
        let handle = app.controls.get(control);
        SetFocus(handle);
        if let Some(range) = row
            .key
            .range
            .as_ref()
            .and_then(|r| search::utf16_range(&row.source, r))
        {
            SendMessageW(handle, EM_SETSEL, range.start, range.end as isize);
            SendMessageW(handle, EM_SCROLLCARET, 0, 0);
        }
    }
}

pub unsafe fn tick(app: &mut App) {
    if app
        .navigation
        .changed_at
        .is_some_and(|at| at.elapsed() >= Duration::from_millis(180))
    {
        refresh(app);
    }
}

pub unsafe fn command(app: &mut App, id: u16, notification: u16) -> bool {
    match id {
        SEARCH => open(app, Mode::Search),
        PROBLEMS => open(app, Mode::Problems),
        QUERY => {
            if notification as u32 == EN_CHANGE && app.navigation.mode.is_some() {
                let value = app.controls.read(QUERY);
                if app.navigation.mode == Some(Mode::Problems) {
                    app.navigation.problem_query = value;
                } else {
                    app.navigation.search_query = value;
                }
                app.navigation.changed();
            }
        }
        FILTER => {
            match notification as u32 {
                CBN_SELENDOK => {}
                CBN_SELCHANGE
                    if SendMessageW(app.controls.get(FILTER), CB_GETDROPPEDSTATE, 0, 0) == 0 => {}
                CBN_SELENDCANCEL => {
                    refresh(app);
                    return true;
                }
                _ => return true,
            }
            app.navigation.filter = app.controls.selection(FILTER, true).unwrap_or(0);
            refresh(app);
        }
        RESULTS => {
            if notification as u32 == LBN_SELCHANGE {
                activate(app, false);
            } else if notification as u32 == LBN_DBLCLK {
                activate(app, true);
            }
        }
        GO => {
            if app.navigation.changed_at.is_some() {
                refresh(app);
            }
            activate(app, true);
        }
        RECHECK => refresh(app),
        CLEAR => {
            if app.navigation.mode == Some(Mode::Problems) {
                app.navigation.problem_query.clear();
            } else {
                app.navigation.search_query.clear();
            }
            refresh(app);
            SetFocus(app.controls.get(QUERY));
        }
        CLOSE => {
            app.navigation.mode = None;
            app.document.finish_edit();
            app.refresh();
            let control = app.controls.get(app.navigation.return_focus);
            SetFocus(if !control.is_null() && IsWindowVisible(control) != 0 {
                control
            } else {
                app.controls.get(CANVAS)
            });
        }
        NEXT | PREVIOUS => {
            if app.navigation.changed_at.is_some() {
                refresh(app);
            }
            let len = app.navigation.rows.len();
            if len > 0 {
                let i = app.controls.selection(RESULTS, false).unwrap_or(0);
                let i = if id == NEXT {
                    (i + 1) % len
                } else {
                    (i + len - 1) % len
                };
                SendMessageW(app.controls.get(RESULTS), LB_SETCURSEL, i, 0);
                activate(app, false);
            }
        }
        _ => return false,
    }
    true
}
