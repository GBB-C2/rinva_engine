//! Portable story model, editor transactions, persistence, and deterministic playback.
//! This crate has no window, rendering, or operating-system UI dependencies.

mod diagnostics;
mod document;
mod model;
mod player;
pub mod recovery;
pub mod saves;
pub mod search;
pub mod storage;
mod story;

pub use diagnostics::inspect_project;
pub use document::Document;
pub use model::{
    Choice, Field, Issue, Line, NodePosition, Project, Scene, Severity, FORMAT_VERSION,
};
pub use player::{Frame, Player, SaveGame};
pub use story::{
    Assignment, AudioCue, Character, Comparison, Condition, Expression, MusicState, NodeKind,
    Operation, Sprite,
};
