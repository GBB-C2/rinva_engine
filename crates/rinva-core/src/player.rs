use crate::{Choice, Line, Project, Scene};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Position {
    scene: String,
    line: usize,
    ended: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveGame {
    version: u32,
    project_id: String,
    signature: String,
    position: Position,
    history: Vec<Position>,
}

pub enum Frame<'a> {
    Dialogue {
        scene: &'a Scene,
        line: &'a Line,
    },
    Choices {
        scene: &'a Scene,
        choices: &'a [Choice],
    },
    End {
        scene: &'a Scene,
    },
}

pub struct Player {
    project: Project,
    position: Position,
    history: Vec<Position>,
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
            },
            history: vec![],
        };
        player.normalize()?;
        Ok(player)
    }
    fn normalize(&mut self) -> Result<(), String> {
        let mut seen = HashSet::new();
        loop {
            let scene = self
                .project
                .scene(&self.position.scene)
                .ok_or("Сцена не найдена.")?;
            if self.position.line < scene.lines.len() || !scene.choices.is_empty() {
                break;
            }
            if let Some(next) = &scene.next {
                if !seen.insert(scene.id.clone()) {
                    return Err("Цикл без ожидания ввода.".into());
                }
                self.position = Position {
                    scene: next.clone(),
                    line: 0,
                    ended: false,
                };
            } else {
                self.position.ended = true;
                break;
            }
        }
        Ok(())
    }
    pub fn frame(&self) -> Frame<'_> {
        let scene = self
            .project
            .scene(&self.position.scene)
            .expect("validated runtime position");
        if self.position.ended {
            Frame::End { scene }
        } else if let Some(line) = scene.lines.get(self.position.line) {
            Frame::Dialogue { scene, line }
        } else {
            Frame::Choices {
                scene,
                choices: &scene.choices,
            }
        }
    }
    fn checkpoint(&mut self) {
        self.history.push(self.position.clone());
        if self.history.len() > 1000 {
            self.history.remove(0);
        }
    }
    pub fn advance(&mut self) -> Result<(), String> {
        if !matches!(self.frame(), Frame::Dialogue { .. }) {
            return Ok(());
        }
        self.checkpoint();
        self.position.line += 1;
        self.normalize()
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
        self.checkpoint();
        self.position = Position {
            scene: target,
            line: 0,
            ended: false,
        };
        self.normalize()
    }
    pub fn back(&mut self) {
        if let Some(old) = self.history.pop() {
            self.position = old;
        }
    }
    pub fn can_go_back(&self) -> bool {
        !self.history.is_empty()
    }
    pub fn save(&self) -> SaveGame {
        SaveGame {
            version: 1,
            project_id: self.project.id.clone(),
            signature: self.project.story_signature(),
            position: self.position.clone(),
            history: self.history.clone(),
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
            let scene = self
                .project
                .scene(&pos.scene)
                .ok_or("В сохранении неизвестная сцена.")?;
            let terminal =
                pos.line == scene.lines.len() && scene.choices.is_empty() && scene.next.is_none();
            let valid = pos.line < scene.lines.len()
                || (pos.line == scene.lines.len() && !scene.choices.is_empty())
                || terminal;
            if !valid || pos.ended != terminal {
                return Err("Повреждена позиция в сохранении.".into());
            }
        }
        self.position = saved.position;
        self.history = saved.history;
        Ok(())
    }
}
