//! UI catalogs are embedded; changing language never changes story content.
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    #[serde(rename = "en")]
    English,
    #[default]
    #[serde(rename = "ru")]
    Russian,
    #[serde(rename = "ko")]
    Korean,
    #[serde(rename = "ja")]
    Japanese,
    #[serde(rename = "zh-Hans")]
    Chinese,
}
pub const LANGUAGES: [Language; 5] = [
    Language::English,
    Language::Russian,
    Language::Korean,
    Language::Japanese,
    Language::Chinese,
];
type Catalog = BTreeMap<String, String>;
fn catalogs() -> &'static [Catalog; 5] {
    static CATALOGS: OnceLock<[Catalog; 5]> = OnceLock::new();
    CATALOGS.get_or_init(|| {
        [
            include_str!("locales/en.json"),
            include_str!("locales/ru.json"),
            include_str!("locales/ko.json"),
            include_str!("locales/ja.json"),
            include_str!("locales/zh-Hans.json"),
        ]
        .map(|s| serde_json::from_str(s).expect("embedded catalog is tested"))
    })
}
impl Language {
    pub fn index(self) -> usize {
        LANGUAGES.iter().position(|&l| l == self).unwrap()
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::Russian => "Русский",
            Self::Korean => "한국어",
            Self::Japanese => "日本語",
            Self::Chinese => "简体中文",
        }
    }
    pub fn font(self) -> &'static str {
        match self {
            Self::Korean => "Malgun Gothic",
            Self::Japanese => "Yu Gothic UI",
            Self::Chinese => "Microsoft YaHei UI",
            _ => "Segoe UI",
        }
    }
    pub fn text(self, key: &str) -> &str {
        catalogs()[self.index()]
            .get(key)
            .map(String::as_str)
            .unwrap_or(key)
    }
    pub fn format(self, key: &str, args: &[(&str, &str)]) -> String {
        let mut value = self.text(key).to_owned();
        for (name, text) in args {
            value = value.replace(&format!("{{{name}}}"), text);
        }
        value
    }
    /// Translate known diagnostics while leaving names, paths and OS details intact.
    pub fn message(self, source: &str) -> String {
        let translated = self.text(source);
        if translated != source || self == Self::Russian {
            return translated.to_owned();
        }
        for (key, translation) in &catalogs()[self.index()] {
            if let Some(open) = key.find('{') {
                if let Some(close) = key[open..].find('}').map(|n| n + open) {
                    let (prefix, suffix) = (&key[..open], &key[close + 1..]);
                    if let Some(value) = source
                        .strip_prefix(prefix)
                        .and_then(|s| s.strip_suffix(suffix))
                    {
                        return translation.replace(&key[open..=close], value);
                    }
                }
            }
        }
        if source.contains('\n') {
            source
                .lines()
                .map(|line| self.message(line))
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            source.to_owned()
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_five_catalogs_have_identical_keys_and_placeholders() {
        let base = &catalogs()[Language::Russian.index()];
        for c in catalogs() {
            assert_eq!(
                c.keys().collect::<Vec<_>>(),
                base.keys().collect::<Vec<_>>()
            );
            for (key, value) in c {
                assert!(!value.is_empty());
                for part in key.split('{').skip(1) {
                    let placeholder = part.split('}').next().unwrap();
                    assert!(
                        value.contains(&format!("{{{placeholder}}}")),
                        "{key}: {value}"
                    );
                }
            }
        }
    }
    #[test]
    fn diagnostics_keep_author_text_and_paths() {
        assert_eq!(
            Language::English.message("В сцене «夜の駅» есть пустая реплика."),
            "Scene “夜の駅” contains empty dialogue."
        );
        assert_eq!(
            Language::Korean.format("Сборка готова: {path}", &[("path", "C:\\夜の駅")]),
            "내보내기 완료: C:\\夜の駅"
        );
        assert_eq!(
            serde_json::from_str::<Language>("\"zh-Hans\"").unwrap(),
            Language::Chinese
        );
    }
}
