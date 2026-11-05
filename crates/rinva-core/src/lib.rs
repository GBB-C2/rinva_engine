//! Portable story model, editor transactions, persistence, and deterministic playback.
//! This crate has no window, rendering, or operating-system UI dependencies.

mod document;
mod model;
mod player;
pub mod storage;

pub use document::Document;
pub use model::{Choice, Issue, Line, Project, Scene, Severity, FORMAT_VERSION};
pub use player::{Frame, Player, SaveGame};
