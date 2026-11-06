use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ReadingSettings {
    pub speed: u32,
    pub dialog_style: u8,
    pub text_size: u8,
    pub volumes: [u8; 3],
}
impl Default for ReadingSettings {
    fn default() -> Self {
        Self {
            speed: 40,
            dialog_style: 0,
            text_size: 0,
            volumes: [75, 75, 100],
        }
    }
}

pub struct Reader {
    pub settings: ReadingSettings,
    pub auto: bool,
    pub skip: bool,
    pub log: bool,
    pub key: Option<(u64, String)>,
    pub seen_before: bool,
    pub shown: usize,
    pub started: Instant,
    pub completed: Option<Instant>,
}
impl Reader {
    pub fn new(mut settings: ReadingSettings) -> Self {
        settings.speed = settings.speed.min(120);
        settings.dialog_style = settings.dialog_style.min(1);
        settings.text_size = settings.text_size.min(2);
        settings.volumes = settings.volumes.map(|v| v.min(100));
        Self {
            settings,
            auto: false,
            skip: false,
            log: false,
            key: None,
            seen_before: false,
            shown: 0,
            started: Instant::now(),
            completed: None,
        }
    }
    pub fn reset(&mut self) {
        self.key = None;
        self.auto = false;
        self.skip = false;
        self.log = false;
    }
    pub fn enter(&mut self, serial: u64, key: String, seen: bool) -> bool {
        if self.key.as_ref() == Some(&(serial, key.clone())) {
            return false;
        }
        self.key = Some((serial, key));
        self.seen_before = seen;
        self.shown = 0;
        self.started = Instant::now();
        self.completed = None;
        true
    }
    pub fn reveal(&mut self, text: &str) {
        self.shown = text.chars().count();
        self.completed.get_or_insert_with(Instant::now);
    }
    pub fn tick(&mut self, text: &str, elapsed: Duration) -> bool {
        let total = text.chars().count();
        let shown = if self.settings.speed == 0 || self.skip && self.seen_before {
            total
        } else {
            (elapsed.as_secs_f64() * self.settings.speed as f64) as usize
        };
        let before = self.shown;
        self.shown = self.shown.max(shown.min(total));
        if self.shown == total {
            self.completed.get_or_insert_with(Instant::now);
        }
        before != self.shown
    }
    pub fn visible<'a>(&self, text: &'a str) -> &'a str {
        &text[..text
            .char_indices()
            .nth(self.shown)
            .map(|(i, _)| i)
            .unwrap_or(text.len())]
    }
    pub fn advance_due(&self, text: &str, voice_busy: bool) -> bool {
        if self.log || self.shown < text.chars().count() {
            return false;
        }
        if self.skip {
            return self.seen_before && self.started.elapsed() >= Duration::from_millis(100);
        }
        self.auto
            && !voice_busy
            && self.completed.is_some_and(|at| {
                at.elapsed()
                    >= Duration::from_millis(1000 + (text.chars().count() as u64 * 20).min(4000))
            })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn typewriter_preserves_unicode_and_skip_never_crosses_unread() {
        let mut reader = Reader::new(ReadingSettings {
            speed: 4,
            ..ReadingSettings::default()
        });
        let text = "夜🌅Мира";
        reader.enter(0, "s:0".into(), false);
        reader.tick(text, Duration::from_millis(500));
        assert_eq!(reader.visible(text), "夜🌅");
        reader.skip = true;
        reader.started = Instant::now() - Duration::from_secs(1);
        reader.reveal(text);
        assert!(!reader.advance_due(text, false));
        reader.enter(1, "s:1".into(), true);
        reader.started = Instant::now() - Duration::from_secs(1);
        reader.tick(text, Duration::ZERO);
        assert!(reader.advance_due(text, false));
        reader.log = true;
        assert!(!reader.advance_due(text, false));
    }
    #[test]
    fn auto_waits_for_voice_and_reveal_does_not_restart_frame() {
        let mut reader = Reader::new(ReadingSettings::default());
        reader.enter(0, "s:0".into(), false);
        reader.reveal("Hello");
        reader.auto = true;
        reader.completed = Some(Instant::now() - Duration::from_secs(6));
        assert!(!reader.advance_due("Hello", true));
        assert!(reader.advance_due("Hello", false));
        assert!(!reader.enter(0, "s:0".into(), true));
        assert_eq!(reader.shown, 5);
    }
}
