use rinva_core::{
    saves::{self, Record, Slot},
    Player, Project,
};
use std::path::PathBuf;

struct Folder(PathBuf);
impl Folder {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!(
                "rinva-slots-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            )))
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn slots_restore_independently_and_keep_previous_atomic_record() {
    let root = Folder::new();
    let p = Project::demo();
    let mut player = Player::new(&p).unwrap();
    let first = Record::capture(&p, &player, vec![1, 2, 3]);
    saves::write(&root.0, Slot::Manual(1), &first).unwrap();
    player.advance().unwrap();
    let second = Record::capture(&p, &player, vec![4, 5]);
    saves::write(&root.0, Slot::Auto, &second).unwrap();
    let loaded = saves::read(&root.0, &p.id, Slot::Manual(1))
        .unwrap()
        .unwrap();
    assert_eq!(loaded.thumbnail, first.thumbnail);
    assert_eq!(
        serde_json::to_value(saves::restore(&p, loaded).unwrap().save()).unwrap(),
        serde_json::to_value(first.progress).unwrap()
    );
    saves::write(&root.0, Slot::Manual(1), &second).unwrap();
    assert!(saves::path(&root.0, &p.id, Slot::Manual(1))
        .unwrap()
        .with_extension("json.bak")
        .exists());
    assert!(saves::read(&root.0, &p.id, Slot::Quick).unwrap().is_none());
}

#[test]
fn corrupt_outdated_and_oversized_slots_do_not_replace_good_progress() {
    let root = Folder::new();
    let p = Project::demo();
    let record = Record::capture(&p, &Player::new(&p).unwrap(), vec![]);
    saves::write(&root.0, Slot::Quick, &record).unwrap();
    let mut too_large = record.clone();
    too_large.thumbnail = vec![0; saves::MAX_THUMBNAIL + 1];
    assert!(saves::write(&root.0, Slot::Quick, &too_large).is_err());
    assert!(saves::read(&root.0, &p.id, Slot::Quick).unwrap().is_some());
    let mut changed = p.clone();
    changed.scenes[0].lines[0].text.push('!');
    assert!(!record.compatible(&changed));
    assert!(saves::restore(&changed, record).is_err());
    std::fs::write(
        saves::path(&root.0, &p.id, Slot::Manual(2)).unwrap(),
        "broken",
    )
    .unwrap();
    assert!(saves::read(&root.0, &p.id, Slot::Manual(2)).is_err());
    assert!(saves::read(&root.0, &p.id, Slot::Quick).unwrap().is_some());
}

#[test]
fn slot_paths_are_bounded_and_project_scoped() {
    let root = Folder::new();
    let a = saves::path(&root.0, "../../escape", Slot::Manual(1)).unwrap();
    assert!(a.starts_with(&root.0));
    assert_eq!(a.components().count(), root.0.components().count() + 2);
    assert_ne!(a, saves::path(&root.0, "other", Slot::Manual(1)).unwrap());
    assert!(saves::path(&root.0, "a", Slot::Manual(0)).is_err());
    assert!(saves::path(&root.0, "a", Slot::Manual(7)).is_err());
    assert!(saves::Slot::at(saves::SLOT_COUNT).is_none());
}
