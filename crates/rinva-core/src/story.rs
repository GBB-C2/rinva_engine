//! Story actions are data, shared by the editor, exported player and diagnostics.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    #[default]
    Dialogue,
    Choice,
    Condition,
    SetVariable,
    Music,
    End,
    UiPanel,
    UiText,
    UiButton,
    UiClear,
    UiScreen,
}
impl NodeKind {
    pub fn is_default(&self) -> bool {
        *self == Self::Dialogue
    }
    pub fn interactive(self) -> bool {
        matches!(self, Self::Dialogue | Self::Choice)
    }
    pub fn ui_element(self) -> bool {
        matches!(self, Self::UiPanel | Self::UiText | Self::UiButton)
    }
    pub fn interface(self) -> bool {
        self.ui_element() || matches!(self, Self::UiClear | Self::UiScreen)
    }
}

/// A screen-space rectangle in percentages of the scene, independent of DPI.
/// Buttons continue construction through `Scene::next`; `target` runs only on click.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiElement {
    pub x: u8,
    pub y: u8,
    pub width: u8,
    pub height: u8,
    #[serde(default)]
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}
impl Default for UiElement {
    fn default() -> Self {
        Self {
            x: 30,
            y: 40,
            width: 40,
            height: 12,
            text: "…".into(),
            target: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Comparison {
    #[default]
    Equal,
    NotEqual,
    Greater,
    AtLeast,
    Less,
    AtMost,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Condition {
    pub variable: String,
    pub comparison: Comparison,
    pub value: i64,
}
impl Condition {
    pub fn evaluate(&self, variables: &BTreeMap<String, i64>) -> bool {
        let Some(&value) = variables.get(&self.variable) else {
            return false;
        };
        match self.comparison {
            Comparison::Equal => value == self.value,
            Comparison::NotEqual => value != self.value,
            Comparison::Greater => value > self.value,
            Comparison::AtLeast => value >= self.value,
            Comparison::Less => value < self.value,
            Comparison::AtMost => value <= self.value,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    #[default]
    Set,
    Add,
    Subtract,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assignment {
    pub variable: String,
    pub operation: Operation,
    pub value: i64,
}
impl Assignment {
    pub fn apply(&self, variables: &mut BTreeMap<String, i64>) -> Result<(), String> {
        let current = variables
            .get_mut(&self.variable)
            .ok_or("Переменная не найдена.")?;
        *current = match self.operation {
            Operation::Set => Some(self.value),
            Operation::Add => current.checked_add(self.value),
            Operation::Subtract => current.checked_sub(self.value),
        }
        .ok_or("Значение переменной выходит за допустимый диапазон.")?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioCue {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub music: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub stop_music: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect: Option<String>,
    #[serde(default)]
    pub fade_ms: u32,
    #[serde(default = "full_volume")]
    pub volume: u8,
}
fn full_volume() -> u8 {
    100
}
impl AudioCue {
    pub fn new() -> Self {
        Self::default()
    }
}
impl Default for AudioCue {
    fn default() -> Self {
        Self {
            music: None,
            stop_music: false,
            effect: None,
            fade_ms: 600,
            volume: 100,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MusicState {
    pub file: String,
    pub volume: u8,
    pub fade_ms: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Character {
    pub id: String,
    pub name: String,
    pub expressions: Vec<Expression>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expression {
    pub id: String,
    pub name: String,
    pub image: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sprite {
    pub character: String,
    pub expression: String,
    /// Percentage of the scene, measured at the sprite's bottom centre.
    pub x: i32,
    pub y: i32,
    pub scale: u32,
    pub layer: i32,
}
impl Sprite {
    pub fn new(character: String, expression: String) -> Self {
        Self {
            character,
            expression,
            x: 50,
            y: 100,
            scale: 100,
            layer: 0,
        }
    }
}

impl UiElement {
    /// Substitute known variable tokens once; retain unknown braces as literal author text.
    pub fn render_text(&self, variables: &BTreeMap<String, i64>) -> String {
        let mut result = String::new();
        let mut rest = self.text.as_str();
        while let Some(open) = rest.find('{') {
            result.push_str(&rest[..open]);
            rest = &rest[open..];
            let Some(close) = rest.find('}') else { break };
            if let Some(value) = variables.get(&rest[1..close]) {
                result.push_str(&value.to_string());
            } else {
                result.push_str(&rest[..=close]);
            }
            rest = &rest[close + 1..];
        }
        result.push_str(rest);
        result
    }
}
