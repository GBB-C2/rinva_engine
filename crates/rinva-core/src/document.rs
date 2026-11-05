use crate::{Project, Scene};

/// Undo history uses whole-document snapshots. Small stories stay simple and reliable;
/// a future command log can replace this without changing the project format.
pub struct Document {
    pub project: Project,
    saved: Option<Project>,
    undo: Vec<Project>,
    redo: Vec<Project>,
    edit_origin: Option<Project>,
}

impl Document {
    pub fn new(project: Project) -> Self {
        Self {
            saved: Some(project.clone()),
            project,
            undo: vec![],
            redo: vec![],
            edit_origin: None,
        }
    }
    pub fn dirty(&self) -> bool {
        self.saved.as_ref() != Some(&self.project)
    }
    pub fn from_unsaved(project: Project) -> Self {
        let mut document = Self::new(project);
        document.saved = None;
        document
    }
    pub fn mark_saved(&mut self) {
        self.finish_edit();
        self.saved = Some(self.project.clone());
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
            || self
                .edit_origin
                .as_ref()
                .is_some_and(|old| old != &self.project)
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn begin_edit(&mut self) {
        if self.edit_origin.is_none() {
            self.edit_origin = Some(self.project.clone());
        }
    }
    pub fn finish_edit(&mut self) {
        if let Some(before) = self.edit_origin.take() {
            if before != self.project {
                self.undo.push(before);
                if self.undo.len() > 100 {
                    self.undo.remove(0);
                }
                self.redo.clear();
            }
        }
    }
    pub fn change(&mut self, edit: impl FnOnce(&mut Project)) {
        self.finish_edit();
        self.begin_edit();
        edit(&mut self.project);
        self.finish_edit();
    }
    pub fn undo(&mut self) {
        self.finish_edit();
        if let Some(before) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.project, before));
        }
    }
    pub fn redo(&mut self) {
        self.finish_edit();
        if let Some(after) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.project, after));
        }
    }
    pub fn add_scene(&mut self) -> usize {
        let index = self.project.scenes.len();
        self.change(|project| {
            project.scenes.push(Scene::new(
                project.next_scene_id(),
                format!("Сцена {}", index + 1),
            ));
        });
        index
    }
    pub fn delete_scene(&mut self, index: usize) -> Result<(), String> {
        if self.project.scenes.len() <= 1 {
            return Err("В проекте должна остаться хотя бы одна сцена.".into());
        }
        let scene = self.project.scenes.get(index).ok_or("Сцена не найдена.")?;
        if self.project.scenes.iter().any(|other| {
            other.id != scene.id
                && (other.next.as_ref() == Some(&scene.id)
                    || other.choices.iter().any(|c| c.target == scene.id))
        }) {
            return Err("На эту сцену ведут переходы. Сначала измените их назначения.".into());
        }
        self.change(|project| {
            let removed = project.scenes.remove(index);
            if project.start_scene == removed.id {
                project.start_scene = project.scenes[0].id.clone();
            }
        });
        Ok(())
    }
}
