//! Game slots are independent of editor recovery. One atomic file owns the image and progress.
use crate::{storage, Frame, Player, Project, SaveGame};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const SLOT_COUNT: usize = 8;
pub const MAX_THUMBNAIL: usize = 256 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Auto,
    Quick,
    Manual(u8),
}
impl Slot {
    pub fn at(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::Auto),
            1 => Some(Self::Quick),
            2..SLOT_COUNT => Some(Self::Manual(index as u8 - 1)),
            _ => None,
        }
    }
    fn filename(self) -> Result<String, String> {
        match self {
            Self::Auto => Ok("auto.json".into()),
            Self::Quick => Ok("quick.json".into()),
            Self::Manual(n @ 1..=6) => Ok(format!("slot-{n}.json")),
            _ => Err("Неизвестный слот сохранения.".into()),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub version: u32,
    pub project_id: String,
    pub signature: String,
    pub saved_at: u64,
    pub scene: String,
    pub speaker: String,
    pub thumbnail: Vec<u8>,
    pub progress: SaveGame,
}
impl Record {
    pub fn capture(project: &Project, player: &Player, thumbnail: Vec<u8>) -> Self {
        let (scene, speaker) = match player.frame() {
            Frame::Dialogue { scene, line } => (scene, line.speaker.clone()),
            Frame::Choices { scene, .. } | Frame::Screen { scene } | Frame::End { scene } => {
                (scene, String::new())
            }
        };
        Self {
            version: 1,
            project_id: project.id.clone(),
            signature: project.story_signature(),
            saved_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            scene: scene.name.clone(),
            speaker,
            thumbnail,
            progress: player.save(),
        }
    }
    pub fn compatible(&self, project: &Project) -> bool {
        self.version == 1
            && self.project_id == project.id
            && self.signature == project.story_signature()
    }
    fn check(&self) -> Result<(), String> {
        if self.version != 1
            || self.thumbnail.len() > MAX_THUMBNAIL
            || self.scene.len() > 4096
            || self.speaker.len() > 4096
            || self.project_id.len() > 4096
        {
            return Err("Повреждённое или неподдерживаемое сохранение.".into());
        }
        Ok(())
    }
}

/// Hash only provides a portable directory name. Identity is also checked inside every record.
pub fn directory(root: &Path, project_id: &str) -> PathBuf {
    let hash = project_id.bytes().fold(0xcbf29ce484222325u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
    });
    root.join(format!("game-{hash:016x}"))
}
pub fn path(root: &Path, project_id: &str, slot: Slot) -> Result<PathBuf, String> {
    Ok(directory(root, project_id).join(slot.filename()?))
}
pub fn write(root: &Path, slot: Slot, record: &Record) -> Result<(), String> {
    record.check()?;
    let path = path(root, &record.project_id, slot)?;
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    storage::write_json(&path, record)
}
pub fn read(root: &Path, project_id: &str, slot: Slot) -> Result<Option<Record>, String> {
    let path = path(root, project_id, slot)?;
    if !path.try_exists().map_err(|e| e.to_string())? {
        return Ok(None);
    }
    let record: Record = storage::read_json(&path)?;
    record.check()?;
    if record.project_id != project_id {
        return Err("Сохранение принадлежит другой новелле.".into());
    }
    Ok(Some(record))
}
pub fn restore(project: &Project, record: Record) -> Result<Player, String> {
    record.check()?;
    if !record.compatible(project) {
        return Err("Сохранение относится к другой версии новеллы.".into());
    }
    let mut player = Player::new(project)?;
    player.restore(record.progress)?;
    Ok(player)
}
