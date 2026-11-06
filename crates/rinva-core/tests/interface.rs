use rinva_core::*;
fn fixture() -> Project {
    serde_json::from_str(include_str!("../../../examples/node-ui.rinva")).unwrap()
}
fn state(p: &Player) -> serde_json::Value {
    serde_json::to_value(p.save()).unwrap()
}
#[test]
fn interface_buttons_execute_mechanics_and_dialogue_branches() {
    let project = fixture();
    project.check().unwrap();
    let mut player = Player::new(&project).unwrap();
    assert!(matches!(player.frame(), Frame::Screen { scene } if scene.id == "screen"));
    assert_eq!(player.interface().len(), 5);
    player.click("earn").unwrap();
    assert_eq!(player.variables()["coins"], 1);
    assert_eq!(player.interface().len(), 5); // Rebuilding is idempotent.
    player.click("start").unwrap();
    assert!(player.interface().is_empty());
    assert!(matches!(player.frame(),Frame::Dialogue {scene,..} if scene.id == "dialogue"));
    player.advance().unwrap();
    assert!(matches!(player.frame(),Frame::Dialogue {scene,..} if scene.id == "ticket"));
    let mut poor = Player::new(&project).unwrap();
    poor.click("start").unwrap();
    poor.advance().unwrap();
    assert!(matches!(poor.frame(),Frame::Dialogue {scene,..} if scene.id == "walk"));
}
#[test]
fn buttons_save_load_and_rollback_restore_ui_and_variables_together() {
    let project = fixture();
    let mut player = Player::new(&project).unwrap();
    player.click("earn").unwrap();
    let saved = player.save();
    player.click("start").unwrap();
    player.back();
    assert!(matches!(player.frame(), Frame::Screen { .. }));
    assert_eq!(player.interface().len(), 5);
    player.back();
    assert_eq!(player.variables()["coins"], 0);
    player.restore(saved).unwrap();
    assert_eq!(player.variables()["coins"], 1);
    assert_eq!(player.interface().len(), 5);
    assert!(player.backlog().is_empty());
}
#[test]
fn invisible_buttons_and_corrupt_saved_ui_are_rejected_atomically() {
    let mut player = Player::new(&fixture()).unwrap();
    player.click("start").unwrap();
    let before = state(&player);
    assert!(player.click("earn").is_err());
    assert_eq!(state(&player), before);
    for ui in [
        serde_json::json!(["missing"]),
        serde_json::json!(["dialogue"]),
        serde_json::json!(["earn", "earn"]),
    ] {
        let mut corrupt = before.clone();
        corrupt["position"]["ui"] = ui;
        assert!(player
            .restore(serde_json::from_value(corrupt).unwrap())
            .is_err());
        assert_eq!(state(&player), before);
    }
}
#[test]
fn failed_button_transition_rolls_back_construction_and_arithmetic() {
    let mut project = fixture();
    project.variables.insert("coins".into(), i64::MAX);
    let mut player = Player::new(&project).unwrap();
    let before = state(&player);
    assert!(player.click("earn").is_err());
    assert_eq!(state(&player), before);
}
#[test]
fn automatic_ui_cycles_fail_but_event_cycles_are_allowed() {
    let mut project = fixture();
    assert!(project.check().is_ok());
    project.scenes[5].next = Some("panel".into());
    assert!(project.check().is_err());
}
#[test]
fn button_links_geometry_search_and_deletion_are_validated() {
    let project = fixture();
    assert!(!project
        .validate()
        .iter()
        .any(|i| i.severity == Severity::Warning));
    let mut doc = Document::new(project.clone());
    assert!(doc.delete_scene(8).is_err());
    let mut broken = project.clone();
    broken.scenes[4].ui.as_mut().unwrap().target = Some("missing".into());
    assert!(broken
        .validate()
        .iter()
        .any(|i| i.field == Field::Interface && i.severity == Severity::Error));
    broken = project.clone();
    broken.scenes[4].ui.as_mut().unwrap().width = 100;
    assert!(broken.check().is_err());
    let hits = search::search(&project, "До отправления", search::SearchScope::All, 10);
    assert_eq!(hits.hits.len(), 1);
    assert_eq!(hits.hits[0].field, Field::Interface);
}
#[test]
fn screen_waits_without_auto_advance_and_optional_next_is_explicit() {
    let mut project = fixture();
    let mut player = Player::new(&project).unwrap();
    let before = state(&player);
    player.advance().unwrap();
    assert_eq!(state(&player), before);
    project.scenes[6].next = Some("clear-story".into());
    let mut player = Player::new(&project).unwrap();
    player.advance().unwrap();
    assert!(player.interface().is_empty());
}
#[test]
fn unicode_variable_labels_are_single_pass_and_unknown_tokens_stay_literal() {
    let ui = UiElement {
        text: "海 {coins} / {other} / {".into(),
        ..Default::default()
    };
    assert_eq!(
        ui.render_text(&std::collections::BTreeMap::from([("coins".into(), 7)])),
        "海 7 / {other} / {"
    );
}
#[test]
fn exceeding_interface_limit_rolls_back_the_whole_transition() {
    let mut p = Project::new();
    p.scenes[0].next = Some("ui-0".into());
    for n in 0..65 {
        let mut s = Scene::new(format!("ui-{n}"), format!("UI {n}"));
        s.kind = NodeKind::UiText;
        s.ui = Some(UiElement::default());
        s.next = Some(if n == 64 {
            "screen".into()
        } else {
            format!("ui-{}", n + 1)
        });
        p.scenes.push(s);
    }
    let mut screen = Scene::new("screen", "Screen");
    screen.kind = NodeKind::UiScreen;
    p.scenes.push(screen);
    let mut player = Player::new(&p).unwrap();
    let before = state(&player);
    assert!(player.advance().is_err());
    assert_eq!(state(&player), before);
}
