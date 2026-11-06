//! Literal Unicode search. Matches retain original UTF-8 boundaries for editing.
use crate::{Field, Project};
use std::ops::Range;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SearchScope {
    #[default]
    All,
    Scenes,
    Dialogue,
    Speakers,
    Choices,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchHit {
    pub scene_id: Option<String>,
    pub field: Field,
    pub range: Range<usize>,
}

#[derive(Clone, Debug, Default)]
pub struct SearchResults {
    pub hits: Vec<SearchHit>,
    pub truncated: bool,
}

fn fold(text: &str) -> String {
    text.chars().flat_map(char::to_lowercase).collect()
}

/// Uses simple Unicode lowercase, without regular expressions or normalization.
/// A bounded result set prevents a broad query from flooding the native list.
pub fn search(project: &Project, query: &str, scope: SearchScope, limit: usize) -> SearchResults {
    let query = fold(query.trim());
    let mut result = SearchResults::default();
    if query.is_empty() || limit == 0 {
        return result;
    }
    let mut visit = |scene: Option<&str>, field: Field, source: &str| {
        let mut folded = String::with_capacity(source.len());
        let mut boundaries = Vec::new();
        for (byte, character) in source.char_indices() {
            boundaries.push((folded.len(), byte));
            folded.extend(character.to_lowercase());
        }
        boundaries.push((folded.len(), source.len()));
        let mut previous = None;
        for (start, _) in folded.match_indices(&query) {
            let first = boundaries.partition_point(|&(n, _)| n <= start) - 1;
            let last = boundaries.partition_point(|&(n, _)| n < start + query.len());
            let range = boundaries[first].1..boundaries[last].1;
            // Lowercase can expand a character; never select half of the original glyph.
            if previous.as_ref() == Some(&range) {
                continue;
            }
            previous = Some(range.clone());
            if result.hits.len() == limit {
                result.truncated = true;
                return false;
            }
            result.hits.push(SearchHit {
                scene_id: scene.map(str::to_owned),
                field,
                range,
            });
        }
        true
    };
    if scope == SearchScope::All && !visit(None, Field::ProjectTitle, &project.title) {
        return result;
    }
    for scene in &project.scenes {
        let id = Some(scene.id.as_str());
        if matches!(scope, SearchScope::All | SearchScope::Scenes)
            && !visit(id, Field::SceneName, &scene.name)
        {
            break;
        }
        for (i, line) in scene.lines.iter().enumerate() {
            if matches!(scope, SearchScope::All | SearchScope::Speakers)
                && !visit(id, Field::Speaker(i), &line.speaker)
            {
                return result;
            }
            if matches!(scope, SearchScope::All | SearchScope::Dialogue)
                && !visit(id, Field::Dialogue(i), &line.text)
            {
                return result;
            }
        }
        if matches!(scope, SearchScope::All | SearchScope::Choices) {
            for (i, choice) in scene.choices.iter().enumerate() {
                if !visit(id, Field::ChoiceText(i), &choice.text) {
                    return result;
                }
            }
        }
    }
    result
}

pub fn field_text<'a>(
    project: &'a Project,
    scene_id: Option<&str>,
    field: Field,
) -> Option<&'a str> {
    match field {
        Field::Project => return Some(&project.id),
        Field::ProjectTitle => return Some(&project.title),
        Field::Logic | Field::Audio | Field::Characters => return Some(""),
        _ => {}
    }
    let scene = project.scene(scene_id?)?;
    Some(match field {
        Field::Scene => &scene.id,
        Field::SceneName => &scene.name,
        Field::Speaker(i) => &scene.lines.get(i)?.speaker,
        Field::Dialogue(i) => &scene.lines.get(i)?.text,
        Field::ChoiceText(i) => &scene.choices.get(i)?.text,
        Field::ChoiceTarget(i) => &scene.choices.get(i)?.target,
        Field::NextScene => scene.next.as_deref().unwrap_or(""),
        Field::Background => scene.background.as_deref().unwrap_or(""),
        Field::Portrait => scene.portrait.as_deref().unwrap_or(""),
        _ => unreachable!(),
    })
}

/// Context is counted in characters, not bytes; CJK and emoji remain intact.
pub fn excerpt(text: &str, range: &Range<usize>, radius: usize) -> String {
    let start = text[..range.start]
        .char_indices()
        .rev()
        .nth(radius.saturating_sub(1))
        .map(|(n, _)| n)
        .unwrap_or(0);
    let end = text[range.end..]
        .char_indices()
        .nth(radius)
        .map(|(n, _)| range.end + n)
        .unwrap_or(text.len());
    format!(
        "{}{}{}",
        if start > 0 { "…" } else { "" },
        text[start..end].replace(['\r', '\n', '\t'], " "),
        if end < text.len() { "…" } else { "" }
    )
}

/// EDIT selections use UTF-16 code units, including both halves of a surrogate pair.
pub fn utf16_range(text: &str, range: &Range<usize>) -> Option<Range<usize>> {
    let prefix = text.get(..range.start)?;
    let matched = text.get(range.clone())?;
    let start = prefix.encode_utf16().count();
    Some(start..start + matched.encode_utf16().count())
}
