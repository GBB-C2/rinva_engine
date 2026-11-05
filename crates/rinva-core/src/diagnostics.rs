use crate::{storage, Field, Issue, NodeKind, Project, Scene, Severity, FORMAT_VERSION};
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
    for name in project.variables.keys() {
        if name.trim().is_empty() || name.chars().count() > 64 {
            error(
                None,
                Field::Logic,
                "Имя переменной должно содержать от 1 до 64 символов.".into(),
            );
        }
    }
    let mut characters = HashSet::new();
    let mut character_names = HashSet::new();
    for character in &project.characters {
        if character.id.is_empty()
            || !characters.insert(&character.id)
            || !character_names.insert(&character.name)
            || character.name.trim().is_empty()
        {
            error(
                None,
                Field::Characters,
                "У персонажа должно быть уникальное имя и идентификатор.".into(),
            );
        }
        let mut expressions = HashSet::new();
        for expression in &character.expressions {
            if expression.id.is_empty()
                || !expressions.insert(&expression.id)
                || expression.name.trim().is_empty()
            {
                error(
                    None,
                    Field::Characters,
                    "У эмоции должно быть имя и уникальный идентификатор.".into(),
                );
            }
        }
    }
    for scene in &project.scenes {
        let id = Some(scene.id.clone());
        if scene.kind == NodeKind::Condition && scene.condition.is_none() {
            error(id.clone(), Field::Logic, "Настройте условие.".into());
        }
        if scene.kind == NodeKind::SetVariable && scene.assignment.is_none() {
            error(
                id.clone(),
                Field::Logic,
                "Настройте изменение переменной.".into(),
            );
        }
        if matches!(
            scene.kind,
            NodeKind::Condition | NodeKind::SetVariable | NodeKind::Music
        ) && scene.next.is_none()
        {
            error(
                id.clone(),
                Field::NextScene,
                "Соедините выход узла со следующей сценой.".into(),
            );
        }
        if scene.kind == NodeKind::Condition
            && scene
                .otherwise
                .as_ref()
                .is_none_or(|target| !index.contains_key(target.as_str()))
        {
            error(
                id.clone(),
                Field::Logic,
                "Укажите переход для ложного условия.".into(),
            );
        }
        for name in scene
            .condition
            .iter()
            .filter(|_| scene.kind == NodeKind::Condition)
            .map(|c| &c.variable)
            .chain(
                scene
                    .assignment
                    .iter()
                    .filter(|_| scene.kind == NodeKind::SetVariable)
                    .map(|a| &a.variable),
            )
            .chain(
                scene
                    .choices
                    .iter()
                    .filter(|_| scene.kind.interactive())
                    .filter_map(|c| c.condition.as_ref().map(|c| &c.variable)),
            )
        {
            if !project.variables.contains_key(name) {
                error(
                    id.clone(),
                    Field::Logic,
                    format!("Переменная не найдена: {name}"),
                );
            }
        }
        if scene.kind == NodeKind::Choice && scene.choices.is_empty() {
            error(
                id.clone(),
                Field::Logic,
                "Добавьте варианты ответа в узел выбора.".into(),
            );
        }
        if scene.kind.interactive()
            && !scene.choices.is_empty()
            && scene.choices.iter().all(|c| c.condition.is_some())
        {
            error(
                id.clone(),
                Field::Logic,
                "Оставьте хотя бы один ответ без условия.".into(),
            );
        }
        if let Some(audio) = &scene.audio {
            if audio.volume > 100
                || audio.fade_ms > 10_000
                || audio.stop_music && audio.music.is_some()
            {
                error(
                    id.clone(),
                    Field::Audio,
                    "Проверьте громкость и переход музыки.".into(),
                );
            }
        }
        for sprite in &scene.sprites {
            if !project.characters.iter().any(|c| {
                c.id == sprite.character && c.expressions.iter().any(|e| e.id == sprite.expression)
            }) {
                error(
                    id.clone(),
                    Field::Characters,
                    "Персонаж или эмоция не найдены.".into(),
                );
            }
            if !(-50..=150).contains(&sprite.x)
                || !(-50..=150).contains(&sprite.y)
                || !(10..=300).contains(&sprite.scale)
            {
                error(
                    id.clone(),
                    Field::Characters,
                    "Позиция персонажа: от −50 до 150%; масштаб: от 10 до 300%.".into(),
                );
            }
        }
        if scene.sprites.len() > 16 {
            error(
                id.clone(),
                Field::Characters,
                "В сцене допускается до 16 персонажей.".into(),
            );
        }
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
        for (i, line) in scene
            .lines
            .iter()
            .enumerate()
            .filter(|_| scene.kind.interactive())
        {
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
        for (i, choice) in scene
            .choices
            .iter()
            .enumerate()
            .filter(|_| scene.kind.interactive())
        {
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
            if scene.kind.interactive() && !scene.choices.is_empty() {
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
            if scene.kind == NodeKind::Condition
                || scene.kind == NodeKind::End
                || (scene.kind.interactive()
                    && (!scene.lines.is_empty() || !scene.choices.is_empty()))
            {
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
            if scene.kind == NodeKind::End {
                continue;
            }
            if scene.kind == NodeKind::Condition {
                pending.extend(scene.otherwise.iter().map(String::as_str));
            }
            if !scene.kind.interactive() || scene.choices.is_empty() {
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
    for (scene_id, field, path) in project.asset_references() {
        if !storage::is_safe_asset_path(path)
            && !matches!(field, Field::Background | Field::Portrait)
        {
            issues.push(Issue {
                severity: Severity::Error,
                scene_id,
                field,
                message: format!("Недопустимый путь к ресурсу: {path}"),
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
    for (scene_id, field, path) in project.asset_references() {
        if !storage::is_safe_asset_path(path) {
            continue;
        }
        let result = assets
            .entry(path)
            .or_insert_with(|| storage::resolve_asset(root, path));
        if let Err(error) = result {
            issues.push(Issue {
                severity: Severity::Error,
                scene_id,
                field,
                message: format!("Не удалось открыть ресурс {path}.\n{error}"),
            });
        }
    }
    issues.sort_by_key(|i| if i.severity == Severity::Error { 0 } else { 1 });
    issues
}
