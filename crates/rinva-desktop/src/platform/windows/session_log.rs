//! Local startup evidence; never contains the project or player dialogue.
use std::{
    io::Write,
    path::PathBuf,
    sync::{Mutex, OnceLock},
};
static FILE: OnceLock<Mutex<std::fs::File>> = OnceLock::new();
static PATH: OnceLock<PathBuf> = OnceLock::new();
pub fn initialize() {
    let folder = super::settings_dir().join("logs");
    if std::fs::create_dir_all(&folder).is_err() {
        return;
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let path = folder.join(format!("session-{stamp}-{}.log", std::process::id()));
    if let Ok(file) = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)
    {
        let _ = FILE.set(Mutex::new(file));
        let _ = PATH.set(path);
    }
    event(concat!(
        "Rinva ",
        env!("CARGO_PKG_VERSION"),
        " Windows x64; startup reached"
    ));
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // The location is sufficient for a bug report; do not log user-controlled panic payloads.
        if let Some(location) = info.location() {
            event(&format!("panic at {}:{}", location.file(), location.line()));
        } else {
            event("panic in native callback");
        }
        previous(info);
    }));
}
pub fn event(text: &str) {
    if let Some(lock) = FILE.get() {
        if let Ok(mut file) = lock.lock() {
            if file.metadata().is_ok_and(|m| m.len() < 1024 * 1024) {
                let _ = writeln!(file, "{text}");
                let _ = file.flush();
            }
        }
    }
}
