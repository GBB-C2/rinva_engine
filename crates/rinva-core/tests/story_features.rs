use rinva_core::*;
use std::collections::BTreeMap;

fn story() -> Project {
    let mut p = Project::new();
    p.variables = BTreeMap::from([("trust".into(), 0)]);
    let mut set = Scene::new("set", "Trust +1");
    set.kind = NodeKind::SetVariable;
    set.assignment = Some(Assignment {
        variable: "trust".into(),
        operation: Operation::Add,
        value: 1,
    });
    set.next = Some("condition".into());
    set.audio = Some(AudioCue {
        music: Some("assets/theme.wav".into()),
        ..AudioCue::new()
    });
    let mut condition = Scene::new("condition", "Trust >= 1");
    condition.kind = NodeKind::Condition;
    condition.condition = Some(Condition {
        variable: "trust".into(),
        comparison: Comparison::AtLeast,
        value: 1,
    });
    condition.next = Some("good".into());
    condition.otherwise = Some("end".into());
    let mut good = Scene::new("good", "Good ending");
    good.next = Some("end".into());
    let mut end = Scene::new("end", "End");
    end.kind = NodeKind::End;
    p.scenes[0].choices = vec![
        Choice {
            text: "Help".into(),
            target: "set".into(),
            ..Choice::default()
        },
        Choice {
            text: "Leave".into(),
            target: "end".into(),
            ..Choice::default()
        },
    ];
    p.scenes.extend([set, condition, good, end]);
    p
}

#[test]
fn variables_music_save_and_rollback_follow_actual_branch() {
    let project = story();
    project.check().unwrap();
    let mut player = Player::new(&project).unwrap();
    player.advance().unwrap();
    player.choose(0).unwrap();
    assert_eq!(player.variables()["trust"], 1);
    assert_eq!(player.music().unwrap().file, "assets/theme.wav");
    assert!(matches!(player.frame(),Frame::Dialogue{scene,..} if scene.id == "good"));
    let save = serde_json::to_string(&player.save()).unwrap();
    let mut loaded = Player::new(&project).unwrap();
    loaded
        .restore(serde_json::from_str(&save).unwrap())
        .unwrap();
    assert_eq!(loaded.variables(), player.variables());
    assert_eq!(loaded.music(), player.music());
    loaded.back();
    assert_eq!(loaded.variables()["trust"], 0);
    assert!(loaded.music().is_none());
    assert!(matches!(loaded.frame(), Frame::Choices { .. }));
    loaded.choose(1).unwrap();
    assert!(matches!(loaded.frame(), Frame::End { .. }));
    loaded
        .restore(serde_json::from_str(&save).unwrap())
        .unwrap();
    loaded.advance().unwrap();
    assert!(matches!(loaded.frame(), Frame::End { .. }));
    assert!(loaded.backlog().contains("Здесь начинается"));
}

#[test]
fn hidden_choices_keep_visible_indices_and_unconditional_fallback() {
    let mut project = story();
    project.scenes[0].choices[0].condition = Some(Condition {
        variable: "trust".into(),
        comparison: Comparison::Greater,
        value: 2,
    });
    let mut player = Player::new(&project).unwrap();
    assert!(!player.was_read());
    player.mark_read();
    assert!(player.was_read());
    player.advance().unwrap();
    assert!(
        matches!(player.frame(),Frame::Choices{choices,..} if choices.len()==1 && choices[0].text=="Leave")
    );
    player.choose(0).unwrap();
    assert!(matches!(player.frame(),Frame::End{scene} if scene.id=="end"));
    project.scenes[0].choices[1].condition = project.scenes[0].choices[0].condition.clone();
    assert!(project.check().is_err());
}

#[test]
fn bounded_conditional_loop_and_failed_transition_are_atomic() {
    let mut project = story();
    project.scenes[2].condition.as_mut().unwrap().value = 3;
    project.scenes[2].otherwise = Some("set".into());
    let mut player = Player::new(&project).unwrap();
    player.advance().unwrap();
    player.choose(0).unwrap();
    assert_eq!(player.variables()["trust"], 3);
    project.scenes[1].assignment.as_mut().unwrap().operation = Operation::Set;
    let mut player = Player::new(&project).unwrap();
    player.advance().unwrap();
    let before = serde_json::to_string(&player.save()).unwrap();
    assert!(player.choose(0).is_err());
    assert_eq!(serde_json::to_string(&player.save()).unwrap(), before);
}

#[test]
fn old_story_signature_is_preserved_and_metadata_does_not_break_saves() {
    let original = Project::demo();
    let json: serde_json::Value =
        serde_json::from_str(include_str!("../../../examples/first-light.rinva")).unwrap();
    let encoded = serde_json::to_value(&original).unwrap();
    assert!(encoded.get("variables").is_none());
    assert!(encoded.get("characters").is_none());
    assert_eq!(encoded["scenes"][0]["lines"], json["scenes"][0]["lines"]);
    let mut modified = original.clone();
    modified.scenes[0].group = "Chapter 1".into();
    modified.scenes[0].note = "Author's note".into();
    assert_eq!(modified.story_signature(), original.story_signature());
    let player = Player::new(&original).unwrap();
    let mut legacy = serde_json::to_value(player.save()).unwrap();
    legacy.as_object_mut().unwrap().remove("read");
    Player::new(&modified)
        .unwrap()
        .restore(serde_json::from_value(legacy).unwrap())
        .unwrap();
}

#[test]
fn resources_cover_voice_music_effects_and_character_expressions() {
    let mut p = story();
    p.scenes[0].lines[0].voice = Some("assets/voice.mp3".into());
    p.scenes[0].audio = Some(AudioCue {
        effect: Some("assets/bell.wav".into()),
        ..AudioCue::new()
    });
    p.characters.push(Character {
        id: "mira".into(),
        name: "Мира".into(),
        expressions: vec![Expression {
            id: "smile".into(),
            name: "Улыбка".into(),
            image: "assets/mira.png".into(),
        }],
    });
    p.scenes[0]
        .sprites
        .push(Sprite::new("mira".into(), "smile".into()));
    p.check().unwrap();
    assert_eq!(storage::assets(&p).len(), 4);
    p.scenes[0].sprites[0].expression = "missing".into();
    assert!(p
        .validate()
        .iter()
        .any(|i| i.field == Field::Characters && i.severity == Severity::Error));
    p.scenes[0].lines[0].voice = Some("../outside.mp3".into());
    assert!(p
        .validate()
        .iter()
        .any(|i| i.field == Field::Audio && i.severity == Severity::Error));
}
