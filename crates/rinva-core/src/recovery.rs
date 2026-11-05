//! Per-session recovery snapshots. They never overwrite the authoring project.
use crate::{storage, Project, FORMAT_VERSION};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub version: u32,
    pub session_id: String,
    pub project: Project,
    pub source_path: Option<PathBuf>,
    pub source_signature: Option<String>,
    pub asset_root: PathBuf,
    pub saved_at: u64,
}

impl Snapshot {
    pub fn new(
        session_id: String,
        project: Project,
        source_path: Option<PathBuf>,
        source_signature: Option<String>,
        asset_root: PathBuf,
    ) -> Self {
        Self {
            version: 1,
            session_id,
            project,
            source_path,
            source_signature,
            asset_root,
            saved_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 || self.project.format_version != FORMAT_VERSION {
            return Err("Неподдерживаемая версия автокопии.".into());
        }
        if self.session_id.is_empty()
            || self.session_id.len() > 128
            || !self
                .session_id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            return Err("Повреждён идентификатор сеанса.".into());
        }
        let mut ids = HashSet::new();
        if self.project.scenes.is_empty()
            || self
                .project
                .scenes
                .iter()
                .any(|s| s.id.is_empty() || !ids.insert(&s.id))
        {
            return Err("Повреждена структура автокопии.".into());
        }
        if !self.asset_root.is_absolute()
            || self.source_path.as_ref().is_some_and(|p| !p.is_absolute())
        {
            return Err("В автокопии отсутствует полный путь к проекту.".into());
        }
        Ok(())
    }

    /// Only retain the original save destination if its content is still exactly
    /// the version from which this editing session started. Otherwise recover as
    /// an unsaved copy, while retaining the original resource root.
    pub fn unchanged_source(&self) -> Option<PathBuf> {
        let path = self.source_path.as_ref()?;
        let signature = self.source_signature.as_ref()?;
        let project = storage::read_project(path).ok()?;
        (project.signature() == *signature).then(|| path.clone())
    }
}

pub fn read(path: &Path) -> Result<Snapshot, String> {
    let snapshot: Snapshot = storage::read_json(path)?;
    snapshot.validate()?;
    Ok(snapshot)
}

pub fn write(path: &Path, snapshot: &Snapshot) -> Result<(), String> {
    snapshot.validate()?;
    storage::write_json(path, snapshot)
}

pub fn remove(path: &Path) -> Result<(), String> {
    for file in [
        path.to_path_buf(),
        PathBuf::from(format!("{}.bak", path.display())),
    ] {
        match std::fs::remove_file(file) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(())
}
