use crate::{Choice, Line, MusicState, NodeKind, Project, Scene};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Position {
    scene: String,
    line: usize,
    ended: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    variables: BTreeMap<String, i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    music: Option<MusicState>,
    #[serde(default)]
    music_fade_ms: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    ui: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveGame {
    version: u32,
    project_id: String,
    signature: String,
    position: Position,
    history: Vec<Position>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    read: BTreeSet<String>,
}
pub enum Frame<'a> {
    Dialogue {
        scene: &'a Scene,
        line: &'a Line,
    },
    Choices {
        scene: &'a Scene,
        choices: Vec<&'a Choice>,
    },
    End {
        scene: &'a Scene,
    },
    Screen {
        scene: &'a Scene,
    },
}
pub struct Player {
    project: Project,
    position: Position,
    history: Vec<Position>,
    read: BTreeSet<String>,
    effects: Vec<String>,
    serial: u64,
}
impl Player {
    pub fn new(project: &Project) -> Result<Self, String> {
        Self::from_scene(project, &project.start_scene)
    }
    pub fn from_scene(project: &Project, scene: &str) -> Result<Self, String> {
        project.check()?;
        if project.scene(scene).is_none() {
            return Err("Сцена не найдена.".into());
        }
        let mut player = Self {
            project: project.clone(),
            position: Position {
                scene: scene.into(),
                line: 0,
                ended: false,
                variables: project.variables.clone(),
                music: None,
                music_fade_ms: 600,
                ui: vec![],
            },
            history: vec![],
            read: BTreeSet::new(),
            effects: vec![],
            serial: 0,
        };
        player.normalize(true)?;
        Ok(player)
    }
    fn normalize(&mut self, mut enter: bool) -> Result<(), String> {
        // Conditions may form finite loops that change a variable; bound automatic work.
        for _ in 0..20_000 {
            let scene = self
                .project
                .scene(&self.position.scene)
                .ok_or("Сцена не найдена.")?;
            if enter {
                if scene.kind == NodeKind::UiClear {
                    self.position.ui.clear();
                } else if scene.kind.ui_element() && !self.position.ui.contains(&scene.id) {
                    if self.position.ui.len() >= 64 {
                        return Err("На экране может быть не более 64 элементов интерфейса.".into());
                    }
                    self.position.ui.push(scene.id.clone());
                }
                if let Some(audio) = &scene.audio {
                    if audio.stop_music || audio.music.is_some() {
                        self.position.music_fade_ms = audio.fade_ms;
                    }
                    if audio.stop_music {
                        self.position.music = None;
                    }
                    if let Some(file) = &audio.music {
                        self.position.music = Some(MusicState {
                            file: file.clone(),
                            volume: audio.volume,
                            fade_ms: audio.fade_ms,
                        });
                    }
                    if let Some(effect) = &audio.effect {
                        self.effects.push(effect.clone());
                    }
                }
                if scene.kind == NodeKind::SetVariable {
                    scene
                        .assignment
                        .as_ref()
                        .ok_or("Настройте изменение переменной.")?
                        .apply(&mut self.position.variables)?;
                }
            }
            if scene.kind == NodeKind::UiScreen {
                return Ok(());
            }
            if scene.kind.interactive()
                && (self.position.line < scene.lines.len()
                    || scene.choices.iter().any(|c| {
                        c.condition
                            .as_ref()
                            .is_none_or(|c| c.evaluate(&self.position.variables))
                    }))
            {
                return Ok(());
            }
            let next = match scene.kind {
                NodeKind::End => None,
                NodeKind::Condition => {
                    if scene
                        .condition
                        .as_ref()
                        .ok_or("Настройте условие.")?
                        .evaluate(&self.position.variables)
                    {
                        scene.next.as_ref()
                    } else {
                        scene.otherwise.as_ref()
                    }
                }
                _ => scene.next.as_ref(),
            };
            if let Some(next) = next {
                self.position.scene = next.clone();
                self.position.line = 0;
                self.position.ended = false;
                enter = true;
            } else {
                self.position.ended = true;
                self.position.line = if scene.kind.interactive() {
                    scene.lines.len()
                } else {
                    0
                };
                return Ok(());
            }
        }
        Err("Слишком много автоматических переходов. Проверьте условия цикла.".into())
    }
    pub fn frame(&self) -> Frame<'_> {
        let scene = self
            .project
            .scene(&self.position.scene)
            .expect("validated runtime position");
        if self.position.ended {
            Frame::End { scene }
        } else if scene.kind == NodeKind::UiScreen {
            Frame::Screen { scene }
        } else if let Some(line) = scene.lines.get(self.position.line) {
            Frame::Dialogue { scene, line }
        } else {
            Frame::Choices {
                scene,
                choices: scene
                    .choices
                    .iter()
                    .filter(|c| {
                        c.condition
                            .as_ref()
                            .is_none_or(|c| c.evaluate(&self.position.variables))
                    })
                    .collect(),
            }
        }
    }
    fn transition(&mut self, target: Option<String>) -> Result<(), String> {
        let before = self.position.clone();
        let old_effects = std::mem::take(&mut self.effects);
        let entering = target.is_some();
        if let Some(target) = target {
            self.position.scene = target;
            self.position.line = 0;
            self.position.ended = false;
        } else {
            self.position.line += 1;
        }
        if let Err(error) = self.normalize(entering) {
            self.position = before;
            self.effects = old_effects;
            return Err(error);
        }
        self.history.push(before);
        if self.history.len() > 1000 {
            self.history.remove(0);
        }
        self.serial = self.serial.wrapping_add(1);
        Ok(())
    }
    pub fn advance(&mut self) -> Result<(), String> {
        if matches!(self.frame(), Frame::Dialogue { .. }) {
            self.mark_read();
            self.transition(None)?;
        } else if let Frame::Screen { scene } = self.frame() {
            if let Some(next) = scene.next.clone() {
                self.transition(Some(next))?;
            }
        }
        Ok(())
    }
    pub fn choose(&mut self, index: usize) -> Result<(), String> {
        let target = match self.frame() {
            Frame::Choices { choices, .. } => choices
                .get(index)
                .ok_or("Вариант ответа не найден.")?
                .target
                .clone(),
            _ => return Err("Сейчас выбор ответа недоступен.".into()),
        };
        self.transition(Some(target))
    }
    /// Activate only a visible button, never an arbitrary node supplied by the view.
    pub fn click(&mut self, id: &str) -> Result<(), String> {
        let scene = self
            .project
            .scene(id)
            .filter(|s| {
                s.kind == NodeKind::UiButton && self.position.ui.iter().any(|active| active == id)
            })
            .ok_or("Кнопка сейчас недоступна.")?;
        let target = scene
            .ui
            .as_ref()
            .and_then(|ui| ui.target.clone())
            .ok_or("У кнопки не задано действие.")?;
        self.transition(Some(target))
    }
    pub fn interface(&self) -> Vec<&Scene> {
        self.position
            .ui
            .iter()
            .filter_map(|id| self.project.scene(id))
            .collect()
    }
    pub fn back(&mut self) {
        if let Some(old) = self.history.pop() {
            self.position = old;
            self.effects.clear();
            self.serial = self.serial.wrapping_add(1);
        }
    }
    pub fn can_go_back(&self) -> bool {
        !self.history.is_empty()
    }
    pub fn variables(&self) -> &BTreeMap<String, i64> {
        &self.position.variables
    }
    pub fn music(&self) -> Option<&MusicState> {
        self.position.music.as_ref()
    }
    pub fn music_fade_ms(&self) -> u32 {
        self.position.music_fade_ms
    }
    pub fn effects(&self) -> &[String] {
        &self.effects
    }
    pub fn serial(&self) -> u64 {
        self.serial
    }
    pub fn line_key(&self) -> String {
        format!("{}:{}", self.position.scene, self.position.line)
    }
    pub fn was_read(&self) -> bool {
        self.read.contains(&self.line_key())
    }
    pub fn mark_read(&mut self) {
        if matches!(self.frame(), Frame::Dialogue { .. }) {
            self.read.insert(self.line_key());
        }
    }
    pub fn read_lines(&self) -> &BTreeSet<String> {
        &self.read
    }
    pub fn merge_read(&mut self, read: BTreeSet<String>) {
        let valid: BTreeSet<_> = self
            .project
            .scenes
            .iter()
            .flat_map(|s| (0..s.lines.len()).map(move |i| format!("{}:{i}", s.id)))
            .collect();
        self.read
            .extend(read.into_iter().filter(|key| valid.contains(key)));
    }
    pub fn backlog(&self) -> String {
        self.history
            .iter()
            .filter_map(|p| {
                let scene = self.project.scene(&p.scene)?;
                let line = scene.lines.get(p.line)?;
                Some(if line.speaker.is_empty() {
                    line.text.clone()
                } else {
                    format!("{}\r\n{}", line.speaker, line.text)
                })
            })
            .collect::<Vec<_>>()
            .join("\r\n\r\n")
    }
    pub fn save(&self) -> SaveGame {
        SaveGame {
            version: 1,
            project_id: self.project.id.clone(),
            signature: self.project.story_signature(),
            position: self.position.clone(),
            history: self.history.clone(),
            read: self.read.clone(),
        }
    }
    pub fn restore(&mut self, saved: SaveGame) -> Result<(), String> {
        if saved.version != 1
            || saved.project_id != self.project.id
            || saved.signature != self.project.story_signature()
        {
            return Err("Сохранение относится к другой версии новеллы.".into());
        }
        if saved.history.len() > 1000 {
            return Err("Слишком длинная история сохранения.".into());
        }
        for pos in std::iter::once(&saved.position).chain(&saved.history) {
            let unique: BTreeSet<_> = pos.ui.iter().collect();
            if pos.ui.len() > 64
                || unique.len() != pos.ui.len()
                || pos
                    .ui
                    .iter()
                    .any(|id| self.project.scene(id).is_none_or(|s| !s.kind.ui_element()))
            {
                return Err("Повреждено состояние интерфейса.".into());
            }
            let scene = self
                .project
                .scene(&pos.scene)
                .ok_or("В сохранении неизвестная сцена.")?;
            if !pos.variables.keys().eq(self.project.variables.keys()) {
                return Err("Повреждены переменные сохранения.".into());
            }
            if pos.music_fade_ms > 10_000 {
                return Err("Повреждено состояние музыки.".into());
            }
            let has_choices = scene.choices.iter().any(|c| {
                c.condition
                    .as_ref()
                    .is_none_or(|c| c.evaluate(&pos.variables))
            });
            let terminal = if scene.kind == NodeKind::End {
                pos.line == 0
            } else {
                scene.kind.interactive()
                    && pos.line == scene.lines.len()
                    && !has_choices
                    && scene.next.is_none()
            };
            let valid = scene.kind.interactive()
                && (pos.line < scene.lines.len() || pos.line == scene.lines.len() && has_choices)
                || (scene.kind == NodeKind::UiScreen && pos.line == 0)
                || terminal;
            if !valid || pos.ended != terminal {
                return Err("Повреждена позиция в сохранении.".into());
            }
            if let Some(music) = &pos.music {
                if music.volume > 100
                    || music.fade_ms > 10_000
                    || !self.project.scenes.iter().any(|s| {
                        s.audio.as_ref().and_then(|a| a.music.as_ref()) == Some(&music.file)
                    })
                {
                    return Err("Повреждено состояние музыки.".into());
                }
            }
        }
        self.position = saved.position;
        self.history = saved.history;
        self.merge_read(saved.read);
        self.effects.clear();
        self.serial = self.serial.wrapping_add(1);
        Ok(())
    }
}
