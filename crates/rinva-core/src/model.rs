use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const FORMAT_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub format_version: u32,
    pub id: String,
    pub title: String,
    pub start_scene: String,
    pub scenes: Vec<Scene>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scene {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub background: Option<String>,
    #[serde(default)]
    pub portrait: Option<String>,
    pub lines: Vec<Line>,
    #[serde(default)]
    pub choices: Vec<Choice>,
    #[serde(default)]
    pub next: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Line {
    pub speaker: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Choice {
    pub text: String,
    pub target: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Issue {
    pub severity: Severity,
    pub scene_id: Option<String>,
    pub message: String,
}

impl Project {
    pub fn new() -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        Self {
            format_version: FORMAT_VERSION,
            id: format!("rinva-{}-{stamp:x}", std::process::id()),
            title: "Моя новелла".into(),
            start_scene: "scene-1".into(),
            scenes: vec![Scene::new("scene-1", "Начало")],
        }
    }

    pub fn demo() -> Self {
        serde_json::from_str(include_str!("../../../examples/first-light.rinva"))
            .expect("bundled story is validated in tests")
    }

    pub fn scene(&self, id: &str) -> Option<&Scene> {
        self.scenes.iter().find(|scene| scene.id == id)
    }

    pub fn next_scene_id(&self) -> String {
        (1..)
            .map(|i| format!("scene-{i}"))
            .find(|id| self.scene(id).is_none())
            .expect("finite project")
    }

    /// Stable content signature for preventing saves being loaded into a changed story.
    /// This is a compatibility check, not a cryptographic hash.
    pub fn signature(&self) -> String {
        let data = serde_json::to_vec(self).expect("model contains serializable data only");
        let hash = data.iter().fold(0xcbf29ce484222325u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        });
        format!("{hash:016x}")
    }

    pub fn validate(&self) -> Vec<Issue> {
        let mut issues = Vec::new();
        let mut error = |scene_id: Option<String>, message: String| {
            issues.push(Issue {
                severity: Severity::Error,
                scene_id,
                message,
            });
        };
        if self.format_version != FORMAT_VERSION {
            error(
                None,
                format!("Неподдерживаемая версия проекта: {}", self.format_version),
            );
        }
        if self.id.trim().is_empty() {
            error(None, "У проекта отсутствует идентификатор.".into());
        }
        if self.title.trim().is_empty() {
            error(None, "Введите название новеллы.".into());
        }
        if self.scene(&self.start_scene).is_none() {
            error(None, "Стартовая сцена не найдена.".into());
        }
        let mut ids = HashSet::new();
        for scene in &self.scenes {
            let id = Some(scene.id.clone());
            if scene.id.trim().is_empty() || !ids.insert(&scene.id) {
                error(
                    id.clone(),
                    "Пустой или повторяющийся идентификатор сцены.".into(),
                );
            }
            if scene.name.trim().is_empty() {
                error(id.clone(), "Введите название сцены.".into());
            }
            for line in &scene.lines {
                if line.text.trim().is_empty() {
                    error(
                        id.clone(),
                        format!("В сцене «{}» есть пустая реплика.", scene.name),
                    );
                }
                if line.text.chars().count() > 4000 {
                    error(
                        id.clone(),
                        "Разделите длинную реплику: максимум 4000 символов.".into(),
                    );
                }
            }
            for choice in &scene.choices {
                if choice.text.trim().is_empty() {
                    error(id.clone(), "Введите текст варианта ответа.".into());
                }
                if self.scene(&choice.target).is_none() {
                    error(
                        id.clone(),
                        format!(
                            "У ответа «{}» нет существующей сцены назначения.",
                            choice.text
                        ),
                    );
                }
            }
            if let Some(next) = &scene.next {
                if self.scene(next).is_none() {
                    error(
                        id.clone(),
                        format!("Переход из «{}» ведёт в отсутствующую сцену.", scene.name),
                    );
                }
                if !scene.choices.is_empty() {
                    error(
                        id.clone(),
                        "У сцены одновременно заданы варианты ответа и автоматический переход."
                            .into(),
                    );
                }
            }
            for path in [&scene.background, &scene.portrait].into_iter().flatten() {
                if !crate::storage::is_safe_asset_path(path) {
                    error(
                        id.clone(),
                        format!("Недопустимый путь к изображению: {path}"),
                    );
                }
            }
        }
        // Empty automatic cycles would otherwise spin without waiting for input.
        for initial in &self.scenes {
            let mut visited = HashSet::new();
            let mut current = Some(initial);
            while let Some(scene) = current {
                if !scene.lines.is_empty() || !scene.choices.is_empty() {
                    break;
                }
                if !visited.insert(&scene.id) {
                    error(
                        Some(initial.id.clone()),
                        "Цикл переходов без реплик или выбора.".into(),
                    );
                    break;
                }
                current = scene.next.as_deref().and_then(|id| self.scene(id));
            }
        }
        let mut reachable = HashSet::new();
        let mut pending = vec![self.start_scene.as_str()];
        while let Some(id) = pending.pop() {
            if !reachable.insert(id) {
                continue;
            }
            if let Some(scene) = self.scene(id) {
                pending.extend(scene.next.iter().map(String::as_str));
                pending.extend(scene.choices.iter().map(|choice| choice.target.as_str()));
            }
        }
        for scene in &self.scenes {
            if !reachable.contains(scene.id.as_str()) {
                issues.push(Issue {
                    severity: Severity::Warning,
                    scene_id: Some(scene.id.clone()),
                    message: format!("Сцена «{}» недоступна от начала истории.", scene.name),
                });
            }
        }
        issues
    }

    pub fn check(&self) -> Result<(), String> {
        let errors: Vec<_> = self
            .validate()
            .into_iter()
            .filter(|issue| issue.severity == Severity::Error)
            .map(|issue| issue.message)
            .collect();
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("\n"))
        }
    }
}

impl Default for Project {
    fn default() -> Self {
        Self::new()
    }
}

impl Scene {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            background: None,
            portrait: None,
            lines: vec![Line {
                speaker: "".into(),
                text: "Здесь начинается ваша история.".into(),
            }],
            choices: vec![],
            next: None,
        }
    }
}
