use crate::{storage, Field, Issue, Project, Scene, Severity, FORMAT_VERSION};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

pub(crate) fn validate(project: &Project) -> Vec<Issue> {
    let index: HashMap<&str, &Scene> = project.scenes.iter().map(|s| (s.id.as_str(), s)).collect();
    let mut issues = Vec::new();
    let mut error = |scene_id: Option<String>, field, message: String| {
        issues.push(Issue {
            severity: Severity::Error,
            scene_id,
            field,
            message,
        });
    };
    if project.format_version != FORMAT_VERSION {
        error(
            None,
            Field::Project,
            format!(
                "Неподдерживаемая версия проекта: {}",
                project.format_version
            ),
        );
    }
    if project.id.trim().is_empty() {
        error(
            None,
            Field::Project,
            "У проекта отсутствует идентификатор.".into(),
        );
    }
    if project.title.trim().is_empty() {
        error(
            None,
            Field::ProjectTitle,
            "Введите название новеллы.".into(),
        );
    }
    if !index.contains_key(project.start_scene.as_str()) {
        error(None, Field::Project, "Стартовая сцена не найдена.".into());
    }
    let mut ids = HashSet::new();
    for scene in &project.scenes {
        let id = Some(scene.id.clone());
        if scene.id.trim().is_empty() || !ids.insert(&scene.id) {
            error(
                id.clone(),
                Field::Scene,
                "Пустой или повторяющийся идентификатор сцены.".into(),
            );
        }
        if scene.name.trim().is_empty() {
            error(
                id.clone(),
                Field::SceneName,
                "Введите название сцены.".into(),
            );
        }
        for (i, line) in scene.lines.iter().enumerate() {
            if line.text.trim().is_empty() {
                error(
                    id.clone(),
                    Field::Dialogue(i),
                    format!("В сцене «{}» есть пустая реплика.", scene.name),
                );
            }
            if line.text.chars().count() > 4000 {
                error(
                    id.clone(),
                    Field::Dialogue(i),
                    "Разделите длинную реплику: максимум 4000 символов.".into(),
                );
            }
        }
        for (i, choice) in scene.choices.iter().enumerate() {
            if choice.text.trim().is_empty() {
                error(
                    id.clone(),
                    Field::ChoiceText(i),
                    "Введите текст варианта ответа.".into(),
                );
            }
            if !index.contains_key(choice.target.as_str()) {
                error(
                    id.clone(),
                    Field::ChoiceTarget(i),
                    format!(
                        "У ответа «{}» нет существующей сцены назначения.",
                        choice.text
                    ),
                );
            }
        }
        if let Some(next) = &scene.next {
            if !index.contains_key(next.as_str()) {
                error(
                    id.clone(),
                    Field::NextScene,
                    format!("Переход из «{}» ведёт в отсутствующую сцену.", scene.name),
                );
            }
            if !scene.choices.is_empty() {
                error(
                    id.clone(),
                    Field::NextScene,
                    "У сцены одновременно заданы варианты ответа и автоматический переход.".into(),
                );
            }
        }
        for (field, path) in [
            (Field::Background, &scene.background),
            (Field::Portrait, &scene.portrait),
        ] {
            if let Some(path) = path {
                if !storage::is_safe_asset_path(path) {
                    error(
                        id.clone(),
                        field,
                        format!("Недопустимый путь к изображению: {path}"),
                    );
                }
            }
        }
    }
    // Memoize whole empty chains, so checking a large graph is linear rather than
    // walking the same automatic path from every scene.
    let mut cycles: HashMap<&str, bool> = HashMap::new();
    for initial in &project.scenes {
        let mut path = Vec::new();
        let mut seen = HashSet::new();
        let mut current = Some(initial.id.as_str());
        let cyclic = loop {
            let Some(id) = current else {
                break false;
            };
            if let Some(&known) = cycles.get(id) {
                break known;
            }
            let Some(scene) = index.get(id) else {
                break false;
            };
            if !scene.lines.is_empty() || !scene.choices.is_empty() {
                break false;
            }
            if !seen.insert(id) {
                break true;
            }
            path.push(id);
            current = scene.next.as_deref();
        };
        for id in path {
            cycles.insert(id, cyclic);
        }
        if cyclic {
            error(
                Some(initial.id.clone()),
                Field::NextScene,
                "Цикл переходов без реплик или выбора.".into(),
            );
        }
    }
    let mut reachable = HashSet::new();
    let mut pending = vec![project.start_scene.as_str()];
    while let Some(id) = pending.pop() {
        if !reachable.insert(id) {
            continue;
        }
        if let Some(scene) = index.get(id) {
            // Match playback: choices take precedence over automatic transitions.
            if scene.choices.is_empty() {
                pending.extend(scene.next.iter().map(String::as_str));
            } else {
                pending.extend(scene.choices.iter().map(|choice| choice.target.as_str()));
            }
        }
    }
    for scene in &project.scenes {
        if !reachable.contains(scene.id.as_str()) {
            issues.push(Issue {
                severity: Severity::Warning,
                scene_id: Some(scene.id.clone()),
                field: Field::Scene,
                message: format!("Сцена «{}» недоступна от начала истории.", scene.name),
            });
        }
    }
    issues
}

/// Draft diagnostics also check that referenced files resolve inside the project.
/// Image decoding stays with the renderer; this function does not allocate bitmaps.
pub fn inspect_project(project: &Project, root: &Path) -> Vec<Issue> {
    let mut issues = validate(project);
    let mut assets = HashMap::new();
    for scene in &project.scenes {
        for (field, path) in [
            (Field::Background, &scene.background),
            (Field::Portrait, &scene.portrait),
        ] {
            let Some(path) = path else {
                continue;
            };
            if !storage::is_safe_asset_path(path) {
                continue;
            }
            let result = assets
                .entry(path)
                .or_insert_with(|| storage::resolve_asset(root, path));
            if let Err(error) = result {
                issues.push(Issue {
                    severity: Severity::Error,
                    scene_id: Some(scene.id.clone()),
                    field,
                    message: format!("Не удалось открыть изображение {path}.\n{error}"),
                });
            }
        }
    }
    issues
}
