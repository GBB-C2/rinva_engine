use rinva_core::{
    inspect_project,
    search::{self, SearchScope},
    Choice, Field, Project, Scene, Severity,
};

#[test]
fn unicode_search_returns_original_ranges_and_native_selections() {
    let mut p = Project::new();
    p.scenes[0].lines[0].text = "🌙\r\nİstanbul · МОРЕ · 한국어 · 日本語 · 中文 · море".into();
    let source = &p.scenes[0].lines[0].text;
    let result = search::search(&p, "мОрЕ", SearchScope::Dialogue, 200);
    assert_eq!(result.hits.len(), 2);
    assert_eq!(&source[result.hits[0].range.clone()], "МОРЕ");
    assert_eq!(&source[result.hits[1].range.clone()], "море");
    for query in ["i", "한국어", "日本語", "中文", "🌙"] {
        let hit = search::search(&p, query, SearchScope::Dialogue, 200)
            .hits
            .remove(0);
        let units = source.encode_utf16().collect::<Vec<_>>();
        let selection = search::utf16_range(source, &hit.range).unwrap();
        assert_eq!(
            String::from_utf16(&units[selection]).unwrap(),
            &source[hit.range.clone()]
        );
        assert!(search::excerpt(source, &hit.range, 5).contains(&source[hit.range]));
    }
    assert_eq!(
        &source[search::search(&p, "i", SearchScope::All, 200).hits[0]
            .range
            .clone()],
        "İ"
    );
    assert!(search::utf16_range(source, &(1..2)).is_none());
}

#[test]
fn scopes_limits_and_read_only_search_are_predictable() {
    let mut p = Project::new();
    p.title = "Find".into();
    p.scenes[0].name = "find".into();
    p.scenes[0].lines[0].speaker = "FIND".into();
    p.scenes[0].lines[0].text = "find find find".into();
    let target = p.scenes[0].id.clone();
    p.scenes[0].choices.push(Choice {
        condition: None,
        text: "find".into(),
        target,
    });
    let original = p.clone();
    let result = search::search(&p, "find", SearchScope::All, 3);
    assert_eq!(result.hits.len(), 3);
    assert!(result.truncated);
    for (scope, count) in [
        (SearchScope::Scenes, 1),
        (SearchScope::Dialogue, 3),
        (SearchScope::Speakers, 1),
        (SearchScope::Choices, 1),
    ] {
        let result = search::search(&p, "find", scope, 200);
        assert_eq!(result.hits.len(), count);
        assert!(!result.truncated);
    }
    assert!(search::search(&p, "   ", SearchScope::All, 200)
        .hits
        .is_empty());
    assert_eq!(p, original);
}

#[test]
fn diagnostics_locate_fields_assets_and_unreachable_scenes() {
    let mut p = Project::demo();
    p.scenes[0].lines[1].text.clear();
    p.scenes[0].choices[1].target = "missing".into();
    p.scenes[1].background = Some("assets/no-such-rinva-image.png".into());
    p.scenes.push(Scene::new("orphan", "Unused"));
    let issues = inspect_project(&p, &std::env::current_dir().unwrap());
    for (id, field, severity) in [
        (p.scenes[0].id.as_str(), Field::Dialogue(1), Severity::Error),
        (
            p.scenes[0].id.as_str(),
            Field::ChoiceTarget(1),
            Severity::Error,
        ),
        (p.scenes[1].id.as_str(), Field::Background, Severity::Error),
        ("orphan", Field::Scene, Severity::Warning),
    ] {
        assert!(issues.iter().any(|i| i.scene_id.as_deref() == Some(id)
            && i.field == field
            && i.severity == severity));
    }
    p.scenes[0].lines[1].text = "Fixed".into();
    assert!(!p.validate().iter().any(|i| i.field == Field::Dialogue(1)));
}

#[test]
fn long_automatic_chains_are_checked_without_recursion() {
    let mut p = Project::new();
    p.scenes = (0..10_000)
        .map(|n| {
            let mut s = Scene::new(format!("s{n}"), format!("Scene {n}"));
            s.lines.clear();
            s.next = Some(format!("s{}", n + 1));
            s
        })
        .collect();
    p.start_scene = "s0".into();
    p.scenes.last_mut().unwrap().next = Some("s9998".into());
    let issues = p.validate();
    assert_eq!(
        issues
            .iter()
            .filter(|i| i.severity == Severity::Error)
            .count(),
        10_000
    );
    p.scenes.last_mut().unwrap().next = None;
    assert!(p.validate().is_empty());
}
