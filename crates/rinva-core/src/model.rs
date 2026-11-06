use crate::{Assignment, AudioCue, Character, Condition, NodeKind, Sprite};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const FORMAT_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub format_version: u32,
    pub id: String,
    pub title: String,
    pub start_scene: String,
    pub scenes: Vec<Scene>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub variables: BTreeMap<String, i64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub characters: Vec<Character>,
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
    /// Authoring coordinates; old projects need no migration. Excluded from game saves.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_position: Option<NodePosition>,
    #[serde(default, skip_serializing_if = "NodeKind::is_default")]
    pub kind: NodeKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<Condition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub otherwise: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignment: Option<Assignment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<AudioCue>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sprites: Vec<Sprite>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub group: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodePosition {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Line {
    pub speaker: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Choice {
    pub text: String,
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<Condition>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Field {
    Project,
    ProjectTitle,
    Scene,
    SceneName,
    Speaker(usize),
    Dialogue(usize),
    ChoiceText(usize),
    ChoiceTarget(usize),
    NextScene,
    Background,
    Portrait,
    Logic,
    Audio,
    Characters,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Issue {
    pub severity: Severity,
    pub field: Field,
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
            variables: BTreeMap::new(),
            characters: vec![],
        }
    }

    pub fn demo() -> Self {
        serde_json::from_str(include_str!("../../../examples/first-light.rinva"))
            .expect("bundled story is validated in tests")
    }

    pub fn scene(&self, id: &str) -> Option<&Scene> {
        self.scenes.iter().find(|scene| scene.id == id)
    }

    pub fn asset_references(&self) -> Vec<(Option<String>, Field, &str)> {
        let mut result = vec![];
        for scene in &self.scenes {
            for (field, path) in [
                (Field::Background, &scene.background),
                (Field::Portrait, &scene.portrait),
            ] {
                if let Some(path) = path {
                    result.push((Some(scene.id.clone()), field, path.as_str()));
                }
            }
            if let Some(audio) = &scene.audio {
                for path in [&audio.music, &audio.effect].into_iter().flatten() {
                    result.push((Some(scene.id.clone()), Field::Audio, path));
                }
            }
            for line in &scene.lines {
                if let Some(path) = &line.voice {
                    result.push((Some(scene.id.clone()), Field::Audio, path));
                }
            }
        }
        for character in &self.characters {
            for expression in &character.expressions {
                result.push((None, Field::Characters, expression.image.as_str()));
            }
        }
        result
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

    /// Rearranging the editor graph must not invalidate a player's progress.
    pub fn story_signature(&self) -> String {
        let mut story = self.clone();
        for scene in &mut story.scenes {
            scene.node_position = None;
            scene.group.clear();
            scene.note.clear();
        }
        story.signature()
    }

    pub fn validate(&self) -> Vec<Issue> {
        crate::diagnostics::validate(self)
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
                voice: None,
            }],
            choices: vec![],
            next: None,
            node_position: None,
            kind: NodeKind::Dialogue,
            condition: None,
            otherwise: None,
            assignment: None,
            audio: None,
            sprites: vec![],
            group: String::new(),
            note: String::new(),
        }
    }
}
