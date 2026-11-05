use rinva_core::{storage, Document, Frame, Player, Project, Scene, Severity};
use std::{fs, path::PathBuf};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "rinva-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn both_demo_branches_finish_and_rollback_restores_choice() {
    for branch in 0..2 {
        let project = Project::demo();
        assert!(project.validate().is_empty());
        let mut player = Player::new(&project).unwrap();
        for _ in 0..3 {
            player.advance().unwrap();
        }
        assert!(matches!(player.frame(), Frame::Choices { choices, .. } if choices.len() == 2));
        player.choose(branch).unwrap();
        assert!(
            matches!(player.frame(), Frame::Dialogue { scene, .. } if scene.id == ["lighthouse", "platform"][branch])
        );
        player.back();
        assert!(matches!(player.frame(), Frame::Choices { .. }));
        player.choose(branch).unwrap();
        for _ in 0..3 {
            player.advance().unwrap();
        }
        assert!(matches!(player.frame(), Frame::End { .. }));
        player.back();
        assert!(matches!(player.frame(), Frame::Dialogue { scene, .. } if scene.id == "ending"));
    }
}

#[test]
fn invalid_choice_does_not_change_player() {
    let mut player = Player::new(&Project::demo()).unwrap();
    let before = serde_json::to_string(&player.save()).unwrap();
    assert!(player.choose(0).is_err());
    assert_eq!(before, serde_json::to_string(&player.save()).unwrap());
    for _ in 0..3 {
        player.advance().unwrap();
    }
    let before = serde_json::to_string(&player.save()).unwrap();
    assert!(player.choose(100).is_err());
    assert_eq!(before, serde_json::to_string(&player.save()).unwrap());
}

#[test]
fn save_roundtrip_and_wrong_revision_rejection_are_atomic() {
    let mut project = Project::demo();
    let mut player = Player::new(&project).unwrap();
    player.advance().unwrap();
    let saved = player.save();
    player.advance().unwrap();
    player.restore(saved.clone()).unwrap();
    assert!(
        matches!(player.frame(), Frame::Dialogue { line, .. } if line.speaker == "Мира" && line.text.starts_with("До рассвета"))
    );
    let mut corrupt = serde_json::to_value(&saved).unwrap();
    corrupt["position"]["line"] = 900.into();
    let before = serde_json::to_string(&player.save()).unwrap();
    assert!(player
        .restore(serde_json::from_value(corrupt).unwrap())
        .is_err());
    assert_eq!(before, serde_json::to_string(&player.save()).unwrap());
    project.title.push('!');
    assert!(Player::new(&project).unwrap().restore(saved).is_err());
}

#[test]
fn empty_cycles_are_errors_but_interactive_cycles_are_valid() {
    let mut project = Project::new();
    project.scenes[0].next = Some(project.start_scene.clone());
    assert!(project.check().is_ok());
    project.scenes[0].lines.clear();
    assert!(project.check().is_err());
    assert!(Player::new(&project).is_err());
}

#[test]
fn unreachable_scenes_warn_and_dangling_links_fail() {
    let mut project = Project::new();
    project.scenes.push(Scene::new("other", "Другая сцена"));
    assert!(project
        .validate()
        .iter()
        .any(|i| i.severity == Severity::Warning));
    project.scenes[0].next = Some("missing".into());
    assert!(project.check().is_err());
}

#[test]
fn text_session_is_one_undo_step_and_new_edits_clear_redo() {
    let mut document = Document::new(Project::new());
    document.begin_edit();
    document.project.title = "О".into();
    document.begin_edit();
    document.project.title = "Океан".into();
    document.finish_edit();
    document.mark_saved();
    assert!(!document.dirty());
    document.undo();
    assert_eq!(document.project.title, "Моя новелла");
    assert!(!document.can_undo());
    assert!(document.dirty());
    document.redo();
    assert!(!document.dirty());
    document.undo();
    document.add_scene();
    assert!(!document.can_redo());
}

#[test]
fn deleting_referenced_scenes_is_blocked_and_start_scene_is_reassigned() {
    let mut document = Document::new(Project::demo());
    assert!(document.delete_scene(1).is_err());
    document.delete_scene(0).unwrap();
    assert_eq!(document.project.start_scene, "lighthouse");
    document.undo();
    assert_eq!(document.project.start_scene, "station");
}

#[test]
fn unicode_draft_roundtrip_backup_and_future_format_rejection() {
    let temp = Temp::new();
    let file = temp.0.join("Новелла.rinva");
    let mut project = Project::new();
    project.title = "海 / Море 🌊".into();
    storage::write_json(&file, &project).unwrap();
    project.scenes[0].lines[0].text.clear(); // unfinished drafts must reopen
    storage::write_json(&file, &project).unwrap();
    assert_eq!(storage::read_project(&file).unwrap(), project);
    assert!(temp.0.join("Новелла.rinva.bak").exists());
    project.format_version = 99;
    storage::write_json(&file, &project).unwrap();
    assert!(storage::read_project(&file).is_err());
}

#[test]
fn unsafe_asset_paths_are_rejected_on_every_host() {
    for path in [
        "../secret",
        "/tmp/a.png",
        "C:/a.png",
        "a\\b.png",
        "a/../../b",
        "a//b",
        "a/./b",
        "a./b",
        "",
    ] {
        assert!(!storage::is_safe_asset_path(path), "{path}");
    }
    assert!(storage::is_safe_asset_path("assets/море.png"));
}

#[test]
fn export_is_portable_and_existing_destination_is_untouched() {
    let temp = Temp::new();
    fs::create_dir(temp.0.join("assets")).unwrap();
    fs::write(temp.0.join("assets/bg.png"), b"fixture").unwrap();
    fs::write(temp.0.join("app.exe"), b"exe").unwrap();
    fs::create_dir(temp.0.join("fonts")).unwrap();
    fs::write(
        temp.0.join("fonts/NotoSansCJK-Regular.ttc"),
        b"font-fixture",
    )
    .unwrap();
    let mut project = Project::new();
    project.scenes[0].background = Some("assets/bg.png".into());
    let destination = temp.0.join("build");
    storage::export_bundle(&project, &temp.0, &destination, &temp.0.join("app.exe")).unwrap();
    assert_eq!(
        storage::read_project(&destination.join("story.rinva")).unwrap(),
        project
    );
    assert_eq!(
        fs::read(destination.join("fonts/NotoSansCJK-Regular.ttc")).unwrap(),
        b"font-fixture"
    );
    assert!(fs::read_to_string(destination.join("fonts/OFL.txt"))
        .unwrap()
        .contains("SIL OPEN FONT LICENSE"));
    assert_eq!(
        fs::read(destination.join("assets/bg.png")).unwrap(),
        b"fixture"
    );
    assert!(
        storage::export_bundle(&project, &temp.0, &destination, &temp.0.join("app.exe")).is_err()
    );
    assert_eq!(
        fs::read(destination.join("RinvaPlayer.exe")).unwrap(),
        b"exe"
    );
}

#[test]
fn failed_export_does_not_publish_partial_bundle() {
    let temp = Temp::new();
    let mut project = Project::new();
    project.scenes[0].background = Some("assets/missing.png".into());
    let destination = temp.0.join("build");
    assert!(
        storage::export_bundle(&project, &temp.0, &destination, &temp.0.join("app.exe")).is_err()
    );
    assert!(!destination.exists());
    assert_eq!(fs::read_dir(&temp.0).unwrap().count(), 0);
}

#[test]
fn recovery_keeps_drafts_and_never_overwrites_a_changed_source() {
    use rinva_core::recovery::{self, Snapshot};
    let temp = Temp::new();
    let source = temp.0.join("story.rinva");
    let original = Project::demo();
    storage::write_json(&source, &original).unwrap();
    let mut edited = original.clone();
    edited.scenes[0].lines[0].text.clear(); // an incomplete draft is recoverable
    let snapshot = Snapshot::new(
        "test-session-1".into(),
        edited.clone(),
        Some(source.clone()),
        Some(original.signature()),
        temp.0.clone(),
    );
    let path = temp.0.join("session.json");
    recovery::write(&path, &snapshot).unwrap();
    let loaded = recovery::read(&path).unwrap();
    assert_eq!(loaded.project, edited);
    assert_eq!(loaded.unchanged_source(), Some(source.clone()));
    assert!(Document::from_unsaved(loaded.project.clone()).dirty());
    let mut external = original.clone();
    external.title = "Изменено другим автором".into();
    storage::write_json(&source, &external).unwrap();
    assert_eq!(loaded.unchanged_source(), None);
    assert_eq!(storage::read_project(&source).unwrap(), external);
    assert_eq!(loaded.asset_root, temp.0);
    recovery::write(&path, &snapshot).unwrap();
    recovery::remove(&path).unwrap();
    assert!(!path.exists());
    assert!(!temp.0.join("session.json.bak").exists());
}

#[test]
fn recovery_rejects_corrupt_metadata_and_supports_untitled_projects() {
    use rinva_core::recovery::Snapshot;
    let temp = Temp::new();
    let mut snapshot = Snapshot::new(
        "valid-session".into(),
        Project::new(),
        None,
        None,
        temp.0.clone(),
    );
    assert!(snapshot.validate().is_ok());
    assert_eq!(snapshot.unchanged_source(), None);
    snapshot.session_id = "../other".into();
    assert!(snapshot.validate().is_err());
    snapshot.session_id = "valid-session".into();
    snapshot.version = 2;
    assert!(snapshot.validate().is_err());
    snapshot.version = 1;
    snapshot.project.scenes.clear();
    assert!(snapshot.validate().is_err());
}

#[cfg(unix)]
#[test]
fn symlink_asset_escape_is_rejected() {
    let root = Temp::new();
    let outside = Temp::new();
    fs::write(outside.0.join("secret.png"), b"outside").unwrap();
    std::os::unix::fs::symlink(&outside.0, root.0.join("assets")).unwrap();
    assert!(storage::resolve_asset(&root.0, "assets/secret.png").is_err());
    assert!(storage::import_asset(&root.0, &outside.0.join("secret.png")).is_err());
}

#[test]
fn graph_layout_roundtrip_preserves_player_progress_and_old_projects() {
    let old = Project::demo();
    let saved = Player::new(&old).unwrap().save();
    let mut moved = old.clone();
    moved.scenes[0].node_position = Some(rinva_core::NodePosition { x: -24, y: 360 });
    assert_ne!(moved.signature(), old.signature());
    assert_eq!(moved.story_signature(), old.signature());
    let mut player = Player::new(&moved).unwrap();
    player.restore(saved).unwrap();
    let json = serde_json::to_vec(&moved).unwrap();
    assert_eq!(serde_json::from_slice::<Project>(&json).unwrap(), moved);
    assert!(!serde_json::to_string(&old)
        .unwrap()
        .contains("node_position"));
    let mut edited = moved.clone();
    edited.scenes[0].lines[0].text.push('!');
    assert!(Player::new(&edited)
        .unwrap()
        .restore(player.save())
        .is_err());
}
