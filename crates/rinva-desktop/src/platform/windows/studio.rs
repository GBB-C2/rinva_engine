use super::*;
use rinva_core::{
    Assignment, AudioCue, Character, Comparison, Condition, Expression, NodeKind, Operation, Sprite,
};

pub const INSPECTOR: u16 = 2000;
pub const KIND: u16 = 2001;
pub const RULE_VAR: u16 = 2002;
pub const RULE_OP: u16 = 2003;
pub const RULE_VALUE: u16 = 2004;
pub const OTHERWISE: u16 = 2005;
pub const APPLY_RULE: u16 = 2006;
pub const VARIABLES: u16 = 2007;
pub const VAR_NAME: u16 = 2008;
pub const VAR_VALUE: u16 = 2009;
pub const VAR_ADD: u16 = 2010;
pub const VAR_UPDATE: u16 = 2011;
pub const VAR_DELETE: u16 = 2012;
pub const MUSIC_MODE: u16 = 2020;
pub const MUSIC_FILE: u16 = 2021;
pub const MUSIC_VOLUME: u16 = 2022;
pub const FADE: u16 = 2023;
pub const EFFECT_FILE: u16 = 2024;
pub const EFFECT_CLEAR: u16 = 2025;
pub const VOICE_FILE: u16 = 2026;
pub const VOICE_CLEAR: u16 = 2027;
pub const AUDIO_APPLY: u16 = 2028;
pub const AUDIO_PREVIEW: u16 = 2029;
pub const AUDIO_STOP: u16 = 2030;
pub const CHARACTERS: u16 = 2040;
pub const CHARACTER_NAME: u16 = 2041;
pub const CHARACTER_ADD: u16 = 2042;
pub const CHARACTER_DELETE: u16 = 2043;
pub const EXPRESSIONS: u16 = 2044;
pub const EXPRESSION_NAME: u16 = 2045;
pub const EXPRESSION_ADD: u16 = 2046;
pub const EXPRESSION_IMAGE: u16 = 2047;
pub const SPRITES: u16 = 2048;
pub const SPRITE_ADD: u16 = 2049;
pub const SPRITE_DELETE: u16 = 2050;
pub const SPRITE_X: u16 = 2051;
pub const SPRITE_Y: u16 = 2052;
pub const SPRITE_SCALE: u16 = 2053;
pub const SPRITE_LAYER: u16 = 2054;
pub const CHARACTER_APPLY: u16 = 2055;
pub const GROUP: u16 = 2060;
pub const NOTE: u16 = 2061;
pub const NOTES_APPLY: u16 = 2062;
pub const SELECT_GROUP: u16 = 2063;
pub const LEFT_WIDTH: u16 = 2064;
pub const RIGHT_WIDTH: u16 = 2065;
pub const RESET_LAYOUT: u16 = 2066;
pub const NEW_KIND: u16 = 2070;
pub const NEW_NODE: u16 = 2071;
pub const MINIMAP: u16 = 2072;
pub const PREVIEW_HEIGHT: u16 = 2073;
const LABEL_BASE: u16 = 5000;
const LABEL_COUNT: u16 = 14;
pub const KINDS: [NodeKind; 11] = [
    NodeKind::Dialogue,
    NodeKind::Choice,
    NodeKind::Condition,
    NodeKind::SetVariable,
    NodeKind::Music,
    NodeKind::End,
    NodeKind::UiPanel,
    NodeKind::UiText,
    NodeKind::UiButton,
    NodeKind::UiClear,
    NodeKind::UiScreen,
];
pub fn kind_name(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Dialogue => "Диалог",
        NodeKind::Choice => "Выбор",
        NodeKind::Condition => "Условие",
        NodeKind::SetVariable => "Переменная",
        NodeKind::Music => "Музыка",
        NodeKind::End => "Конец",
        NodeKind::UiPanel => "UI · Панель",
        NodeKind::UiText => "UI · Текст",
        NodeKind::UiButton => "UI · Кнопка",
        NodeKind::UiClear => "UI · Очистить",
        NodeKind::UiScreen => "UI · Экран",
    }
}
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Pane {
    #[default]
    Text,
    Logic,
    Audio,
    Characters,
    Notes,
    Interface,
}
pub const PANES: [Pane; 6] = [
    Pane::Text,
    Pane::Logic,
    Pane::Audio,
    Pane::Characters,
    Pane::Notes,
    Pane::Interface,
];
#[derive(Default)]
pub struct Studio {
    pub pane: Pane,
    pub character: usize,
    pub expression: usize,
    pub sprite: usize,
    pub variable: usize,
    pub new_kind: usize,
    inspected: String,
}
const BUTTONS: &[(u16, &str)] = &[
    (APPLY_RULE, "Применить правило"),
    (VAR_ADD, "Добавить"),
    (VAR_UPDATE, "Изменить"),
    (VAR_DELETE, "Удалить"),
    (MUSIC_FILE, "Выбрать музыку…"),
    (EFFECT_FILE, "Выбрать эффект…"),
    (EFFECT_CLEAR, "Убрать"),
    (VOICE_FILE, "Выбрать голос…"),
    (VOICE_CLEAR, "Убрать"),
    (AUDIO_APPLY, "Применить"),
    (AUDIO_PREVIEW, "Прослушать"),
    (AUDIO_STOP, "Стоп"),
    (CHARACTER_ADD, "+"),
    (CHARACTER_DELETE, "Удалить"),
    (EXPRESSION_ADD, "+"),
    (EXPRESSION_IMAGE, "Изображение…"),
    (SPRITE_ADD, "На сцену"),
    (SPRITE_DELETE, "Убрать"),
    (CHARACTER_APPLY, "Применить"),
    (NOTES_APPLY, "Применить к выделенным"),
    (SELECT_GROUP, "Выделить группу"),
    (RESET_LAYOUT, "Сбросить панели"),
    (NEW_NODE, "+ Узел"),
    (MINIMAP, "Карта"),
];
const EDITS: &[u16] = &[
    RULE_VALUE,
    VAR_NAME,
    VAR_VALUE,
    MUSIC_VOLUME,
    FADE,
    CHARACTER_NAME,
    EXPRESSION_NAME,
    SPRITE_X,
    SPRITE_Y,
    SPRITE_SCALE,
    SPRITE_LAYER,
    GROUP,
    NOTE,
];
const COMBOS: &[u16] = &[
    INSPECTOR,
    KIND,
    RULE_VAR,
    RULE_OP,
    OTHERWISE,
    VARIABLES,
    MUSIC_MODE,
    CHARACTERS,
    EXPRESSIONS,
    LEFT_WIDTH,
    RIGHT_WIDTH,
    NEW_KIND,
    PREVIEW_HEIGHT,
];
pub unsafe fn create(
    controls: &mut Controls,
    hwnd: HWND,
    resources: &Resources,
) -> Result<(), String> {
    for &(id, label) in BUTTONS {
        controls.button(hwnd, id, label, resources.font)?;
    }
    for &id in EDITS {
        controls.edit(hwnd, id, id == NOTE, resources.font)?;
    }
    for &id in COMBOS {
        controls.combo(hwnd, id, resources.font)?;
    }
    controls.list(hwnd, SPRITES, resources.font)?;
    for n in 0..LABEL_COUNT {
        controls.label(hwnd, LABEL_BASE + n, "", resources.small)?;
    }
    Ok(())
}
pub unsafe fn layout(app: &App, w: i32, h: i32) {
    for id in BUTTONS
        .iter()
        .map(|x| x.0)
        .chain(EDITS.iter().copied())
        .chain(COMBOS.iter().copied())
        .chain(std::iter::once(SPRITES))
        .chain(LABEL_BASE..LABEL_BASE + LABEL_COUNT)
    {
        app.controls.show(id, false);
    }
    if app.player.is_some() {
        return;
    }
    let (center, right, cw, iw) = app.panel_geometry(w);
    let x = right + 16;
    let place = |id, x, y, width, height| {
        app.controls.show(id, true);
        app.controls.place(id, x, y, width, height, app.scale);
    };
    let label = |n, y, key: &str| {
        let id = LABEL_BASE + n;
        app.controls.text(id, app.t(key));
        place(id, x, y, iw, 20);
    };
    place(INSPECTOR, x, 96, iw, 220);
    app.controls.show(LABEL_SCENE, false);
    place(SCENE_NAME, x, 140, iw, 32);
    if app.graph_mode && app.navigation.mode.is_none() {
        let toolbar_y = 132 + app.preview_height(h);
        place(
            NEW_KIND,
            center + 140,
            toolbar_y,
            (cw - 246).clamp(120, 200),
            360,
        );
        place(
            NEW_NODE,
            center + 148 + (cw - 246).clamp(120, 200),
            toolbar_y,
            90,
            28,
        );
        place(MINIMAP, center + cw - 88, h - 114, 88, 28);
    }
    if app.studio.pane == Pane::Text {
        place(BACKGROUND, x, 184, iw - 84, 32);
        place(CLEAR_BG, x + iw - 76, 184, 76, 32);
        place(PORTRAIT, x, 224, iw - 84, 32);
        place(CLEAR_PORTRAIT, x + iw - 76, 224, 76, 32);
        // The legacy inspector remains available for existing projects and image diagnostics.
        if app.choices_tab {
            place(LABEL_BODY, x, 276, iw, 20);
            place(BODY, x, 302, iw, h - 512);
            place(LABEL_TARGET, x, h - 194, iw, 20);
            place(TARGET, x, h - 168, iw, 220);
        } else {
            place(LABEL_SPEAKER, x, 276, iw, 20);
            place(SPEAKER, x, 302, iw, 32);
            place(LABEL_BODY, x, 352, iw, 20);
            place(BODY, x, 378, iw, h - 512);
        }
        place(LABEL_NEXT, x, h - 114, iw, 20);
        place(NEXT_SCENE, x, h - 88, iw, 220);
        return;
    }
    for id in [
        BACKGROUND,
        CLEAR_BG,
        PORTRAIT,
        CLEAR_PORTRAIT,
        LABEL_BODY,
        BODY,
        LABEL_SPEAKER,
        SPEAKER,
        LABEL_TARGET,
        TARGET,
        LABEL_NEXT,
        NEXT_SCENE,
    ] {
        app.controls.show(id, false);
    }
    match app.studio.pane {
        Pane::Logic => {
            label(0, 184, "Тип узла");
            place(KIND, x, 206, iw, 240);
            label(
                1,
                246,
                if app.choices_tab {
                    "Условие ответа"
                } else {
                    "Правило узла"
                },
            );
            place(RULE_VAR, x, 268, iw, 220);
            place(RULE_OP, x, 310, 96, 220);
            place(RULE_VALUE, x + 104, 310, iw - 104, 32);
            if app.scene().kind == NodeKind::Condition && !app.choices_tab {
                label(2, 354, "Если нет");
                place(OTHERWISE, x, 376, iw, 220);
            }
            place(APPLY_RULE, x, 418, iw, 32);
            label(3, 466, "Переменные проекта");
            place(VARIABLES, x, 488, iw, 220);
            place(VAR_NAME, x, 530, iw - 96, 32);
            place(VAR_VALUE, x + iw - 88, 530, 88, 32);
            let b = (iw - 16) / 3;
            place(VAR_ADD, x, 572, b, 32);
            place(VAR_UPDATE, x + b + 8, 572, b, 32);
            place(VAR_DELETE, x + 2 * (b + 8), 572, b, 32);
            label(
                4,
                620,
                if app.scene().kind == NodeKind::Condition {
                    "Если да"
                } else {
                    "После сцены"
                },
            );
            place(NEXT_SCENE, x, 642, iw, 220);
        }
        Pane::Audio => {
            label(0, 188, "Музыка сцены");
            place(MUSIC_MODE, x, 210, iw, 220);
            place(MUSIC_FILE, x, 250, iw, 32);
            label(1, 292, "Громкость % · переход, мс");
            place(MUSIC_VOLUME, x, 314, (iw - 8) / 2, 32);
            place(FADE, x + (iw + 8) / 2, 314, (iw - 8) / 2, 32);
            label(2, 366, "Звуковой эффект");
            place(EFFECT_FILE, x, 388, iw - 84, 32);
            place(EFFECT_CLEAR, x + iw - 76, 388, 76, 32);
            label(3, 440, "Голос выбранной реплики");
            place(VOICE_FILE, x, 462, iw - 84, 32);
            place(VOICE_CLEAR, x + iw - 76, 462, 76, 32);
            place(AUDIO_APPLY, x, 520, iw, 32);
            place(AUDIO_PREVIEW, x, 564, (iw - 8) / 2, 32);
            place(AUDIO_STOP, x + (iw + 8) / 2, 564, (iw - 8) / 2, 32);
            label(4, 616, "WAV · MP3 · M4A · FLAC");
        }
        Pane::Characters => {
            label(0, 184, "Каталог персонажей");
            place(CHARACTERS, x, 206, iw - 40, 220);
            place(CHARACTER_ADD, x + iw - 32, 206, 32, 30);
            place(CHARACTER_NAME, x, 246, iw - 84, 32);
            place(CHARACTER_DELETE, x + iw - 76, 246, 76, 32);
            label(1, 288, "Эмоция");
            place(EXPRESSIONS, x, 310, iw - 40, 220);
            place(EXPRESSION_ADD, x + iw - 32, 310, 32, 30);
            place(EXPRESSION_NAME, x, 350, iw - 124, 32);
            place(EXPRESSION_IMAGE, x + iw - 116, 350, 116, 32);
            label(2, 396, "Персонажи в сцене");
            place(SPRITES, x, 418, iw, 76);
            place(SPRITE_ADD, x, 502, (iw - 8) / 2, 32);
            place(SPRITE_DELETE, x + (iw + 8) / 2, 502, (iw - 8) / 2, 32);
            label(3, 548, "X % · Y % · масштаб % · слой");
            let b = (iw - 18) / 4;
            for (n, id) in [SPRITE_X, SPRITE_Y, SPRITE_SCALE, SPRITE_LAYER]
                .iter()
                .enumerate()
            {
                place(*id, x + n as i32 * (b + 6), 570, b, 32);
            }
            place(CHARACTER_APPLY, x, 620, iw, 32);
            SendMessageW(
                app.controls.get(SPRITES),
                LB_SETITEMHEIGHT,
                0,
                (32. * app.scale) as isize,
            );
        }
        Pane::Notes => {
            label(0, 188, "Группа узлов");
            place(GROUP, x, 210, iw, 32);
            label(1, 260, "Комментарий автора");
            place(NOTE, x, 282, iw, (h - 584).max(100));
            place(NOTES_APPLY, x, h - 282, iw, 32);
            place(SELECT_GROUP, x, h - 242, iw, 32);
            label(2, h - 192, "Ширина левой / правой панели");
            place(LEFT_WIDTH, x, h - 166, (iw - 8) / 2, 220);
            place(RIGHT_WIDTH, x + (iw + 8) / 2, h - 166, (iw - 8) / 2, 220);
            place(RESET_LAYOUT, x, h - 112, iw, 32);
            label(6, h - 66, "Просмотр, %");
            place(PREVIEW_HEIGHT, x + iw - 84, h - 72, 84, 180);
        }
        Pane::Text | Pane::Interface => {}
    }
}
unsafe fn fill(app: &App, id: u16, values: &[&str], selected: usize) {
    app.controls.fill_list(
        id,
        &values
            .iter()
            .map(|s| app.t(s).to_owned())
            .collect::<Vec<_>>(),
        selected,
        true,
    );
}
pub unsafe fn refresh(app: &mut App) {
    let mut heights = vec![25, 35, 45, 55, 65, app.preview_share];
    heights.sort_unstable();
    heights.dedup();
    app.controls.fill_list(
        PREVIEW_HEIGHT,
        &heights.iter().map(|n| format!("{n}%")).collect::<Vec<_>>(),
        heights
            .iter()
            .position(|n| *n == app.preview_share)
            .unwrap_or(0),
        true,
    );
    if app.studio.inspected != app.scene().id {
        app.studio.inspected = app.scene().id.clone();
        if app.scene().kind.interface() {
            app.studio.pane = Pane::Interface;
        } else if app.studio.pane == Pane::Interface {
            app.studio.pane = Pane::Text;
        }
    }
    for &(id, label) in BUTTONS {
        app.controls.text(id, app.t(label));
    }
    fill(
        app,
        INSPECTOR,
        &[
            "Текст и сцена",
            "Логика",
            "Звук",
            "Персонажи",
            "Группы и заметки",
            "Интерфейс",
        ],
        PANES
            .iter()
            .position(|p| *p == app.studio.pane)
            .unwrap_or(0),
    );
    let kinds: Vec<_> = KINDS.iter().map(|k| kind_name(*k)).collect();
    fill(
        app,
        KIND,
        &kinds,
        KINDS
            .iter()
            .position(|k| *k == app.scene().kind)
            .unwrap_or(0),
    );
    fill(app, NEW_KIND, &kinds, app.studio.new_kind);
    let vars: Vec<_> = app.document.project.variables.keys().cloned().collect();
    app.studio.variable = app.studio.variable.min(vars.len().saturating_sub(1));
    app.controls
        .fill_list(VARIABLES, &vars, app.studio.variable, true);
    let var = vars.get(app.studio.variable);
    app.controls
        .text(VAR_NAME, var.map(String::as_str).unwrap_or(""));
    app.controls.text(
        VAR_VALUE,
        &var.map(|v| app.document.project.variables[v])
            .unwrap_or(0)
            .to_string(),
    );
    let scene = app.scene();
    let condition = if app.choices_tab {
        scene
            .choices
            .get(app.selected_item)
            .and_then(|c| c.condition.as_ref())
    } else {
        scene.condition.as_ref()
    };
    let assignment = (!app.choices_tab && scene.kind == NodeKind::SetVariable)
        .then_some(scene.assignment.as_ref())
        .flatten();
    let name = assignment
        .map(|a| &a.variable)
        .or_else(|| condition.map(|c| &c.variable));
    let mut rule_vars = vec![app.t("Без условия").to_owned()];
    rule_vars.extend(vars.clone());
    app.controls.fill_list(
        RULE_VAR,
        &rule_vars,
        name.and_then(|name| vars.iter().position(|v| v == name))
            .map(|i| i + 1)
            .unwrap_or(0),
        true,
    );
    if scene.kind == NodeKind::SetVariable && !app.choices_tab {
        fill(
            app,
            RULE_OP,
            &["=", "+", "−"],
            assignment
                .map(|a| match a.operation {
                    Operation::Set => 0,
                    Operation::Add => 1,
                    Operation::Subtract => 2,
                })
                .unwrap_or(0),
        );
    } else {
        fill(
            app,
            RULE_OP,
            &["=", "≠", ">", "≥", "<", "≤"],
            condition
                .map(|c| match c.comparison {
                    Comparison::Equal => 0,
                    Comparison::NotEqual => 1,
                    Comparison::Greater => 2,
                    Comparison::AtLeast => 3,
                    Comparison::Less => 4,
                    Comparison::AtMost => 5,
                })
                .unwrap_or(0),
        );
    }
    app.controls.text(
        RULE_VALUE,
        &assignment
            .map(|a| a.value)
            .or_else(|| condition.map(|c| c.value))
            .unwrap_or(0)
            .to_string(),
    );
    let mut targets = vec![app.t("Выберите сцену").into()];
    targets.extend(app.document.project.scenes.iter().map(|s| s.name.clone()));
    app.controls.fill_list(
        OTHERWISE,
        &targets,
        scene
            .otherwise
            .as_ref()
            .and_then(|id| app.document.project.scenes.iter().position(|s| &s.id == id))
            .map(|i| i + 1)
            .unwrap_or(0),
        true,
    );
    let cue = scene.audio.clone().unwrap_or_default();
    fill(
        app,
        MUSIC_MODE,
        &["Продолжить музыку", "Сменить музыку", "Остановить музыку"],
        if cue.stop_music {
            2
        } else if cue.music.is_some() {
            1
        } else {
            0
        },
    );
    for (id, path, fallback) in [
        (MUSIC_FILE, cue.music.as_deref(), "Выбрать музыку…"),
        (EFFECT_FILE, cue.effect.as_deref(), "Выбрать эффект…"),
        (
            VOICE_FILE,
            scene
                .lines
                .get(app.selected_item)
                .and_then(|l| l.voice.as_deref()),
            "Выбрать голос…",
        ),
    ] {
        app.controls.text(
            id,
            path.and_then(|p| Path::new(p).file_name())
                .and_then(|n| n.to_str())
                .unwrap_or(app.t(fallback)),
        );
    }
    app.controls.text(MUSIC_VOLUME, &cue.volume.to_string());
    app.controls.text(FADE, &cue.fade_ms.to_string());
    for id in [VOICE_FILE, VOICE_CLEAR] {
        app.controls.enable(
            id,
            !app.choices_tab
                && scene.kind.interactive()
                && scene.lines.get(app.selected_item).is_some(),
        );
    }
    app.controls.enable(EFFECT_CLEAR, cue.effect.is_some());
    app.controls.text(GROUP, &scene.group);
    app.controls.text(NOTE, &scene.note);
    app.controls.enable(SELECT_GROUP, !scene.group.is_empty());
    fill(
        app,
        LEFT_WIDTH,
        &["180", "224", "280"],
        if app.left_width < 210 {
            0
        } else if app.left_width < 260 {
            1
        } else {
            2
        },
    );
    fill(
        app,
        RIGHT_WIDTH,
        &["280", "300", "380"],
        if app.right_width < 290 {
            0
        } else if app.right_width < 340 {
            1
        } else {
            2
        },
    );
    refresh_characters(app);
}
unsafe fn refresh_characters(app: &mut App) {
    let chars = &app.document.project.characters;
    app.studio.character = app.studio.character.min(chars.len().saturating_sub(1));
    app.controls.fill_list(
        CHARACTERS,
        &chars.iter().map(|c| c.name.clone()).collect::<Vec<_>>(),
        app.studio.character,
        true,
    );
    let character = chars.get(app.studio.character);
    app.controls.text(
        CHARACTER_NAME,
        character.map(|c| c.name.as_str()).unwrap_or(""),
    );
    let expressions = character.map(|c| c.expressions.as_slice()).unwrap_or(&[]);
    app.studio.expression = app
        .studio
        .expression
        .min(expressions.len().saturating_sub(1));
    app.controls.fill_list(
        EXPRESSIONS,
        &expressions
            .iter()
            .map(|e| e.name.clone())
            .collect::<Vec<_>>(),
        app.studio.expression,
        true,
    );
    app.controls.text(
        EXPRESSION_NAME,
        expressions
            .get(app.studio.expression)
            .map(|e| e.name.as_str())
            .unwrap_or(""),
    );
    for id in [CHARACTER_DELETE, CHARACTER_NAME, EXPRESSION_ADD] {
        app.controls.enable(id, character.is_some());
    }
    for id in [EXPRESSION_IMAGE, EXPRESSION_NAME, SPRITE_ADD] {
        app.controls.enable(id, !expressions.is_empty());
    }
    app.studio.sprite = app
        .studio
        .sprite
        .min(app.scene().sprites.len().saturating_sub(1));
    let sprites = &app.scene().sprites;
    app.controls.fill_list(
        SPRITES,
        &sprites
            .iter()
            .map(|s| {
                let c = chars.iter().find(|c| c.id == s.character);
                let e = c.and_then(|c| c.expressions.iter().find(|e| e.id == s.expression));
                format!(
                    "{} · {} · {}",
                    s.layer,
                    c.map(|c| c.name.as_str()).unwrap_or(&s.character),
                    e.map(|e| e.name.as_str()).unwrap_or(&s.expression)
                )
            })
            .collect::<Vec<_>>(),
        app.studio.sprite,
        false,
    );
    let sprite = sprites.get(app.studio.sprite);
    for (id, value) in [
        (SPRITE_X, sprite.map(|s| s.x).unwrap_or(50)),
        (SPRITE_Y, sprite.map(|s| s.y).unwrap_or(100)),
        (SPRITE_SCALE, sprite.map(|s| s.scale as i32).unwrap_or(100)),
        (SPRITE_LAYER, sprite.map(|s| s.layer).unwrap_or(0)),
    ] {
        app.controls.text(id, &value.to_string());
        app.controls.enable(id, sprite.is_some());
    }
    app.controls.enable(SPRITE_DELETE, sprite.is_some());
    app.controls.enable(CHARACTER_APPLY, character.is_some());
}
unsafe fn number(app: &App, id: u16, min: i64, max: i64) -> Result<i64, String> {
    let n = app
        .controls
        .read(id)
        .trim()
        .parse::<i64>()
        .map_err(|_| "Введите целое число.")?;
    if !(min..=max).contains(&n) {
        return Err(format!("Допустимое значение: {min}…{max}"));
    }
    Ok(n)
}
pub unsafe fn command(app: &mut App, id: u16, notification: u16) -> Result<bool, String> {
    if !(2000..2100).contains(&id) {
        return Ok(false);
    }
    if EDITS.contains(&id) {
        return Ok(true);
    }
    if COMBOS.contains(&id) {
        match notification as u32 {
            CBN_SELENDCANCEL => {
                app.refresh();
                return Ok(true);
            }
            CBN_SELENDOK => {}
            CBN_SELCHANGE if SendMessageW(app.controls.get(id), CB_GETDROPPEDSTATE, 0, 0) == 0 => {}
            _ => return Ok(true),
        }
    }
    if id == SPRITES && notification as u32 != LBN_SELCHANGE {
        return Ok(true);
    }
    app.document.finish_edit();
    app.graph.cancel();
    if id != AUDIO_PREVIEW {
        app.audio.stop();
    }
    let i = app.selected_scene;
    let item = app.selected_item;
    match id {
        INSPECTOR => {
            app.studio.pane = PANES[app
                .controls
                .selection(id, true)
                .unwrap_or(0)
                .min(PANES.len() - 1)];
        }
        NEW_KIND => {
            app.studio.new_kind = app
                .controls
                .selection(id, true)
                .unwrap_or(0)
                .min(KINDS.len() - 1);
        }
        NEW_NODE => {
            let kind = KINDS[app.studio.new_kind];
            let name = app.t(kind_name(kind)).to_owned();
            let text = app.t("Новая реплика").to_owned();
            app.document.change(|p| {
                let mut s = rinva_core::Scene::new(p.next_scene_id(), name);
                s.kind = kind;
                if kind.ui_element() {
                    s.ui = Some(Default::default());
                }
                s.lines[0].text = text;
                if kind != NodeKind::Dialogue {
                    s.lines.clear();
                }
                if kind == NodeKind::Choice {
                    s.choices.push(Choice {
                        text: "…".into(),
                        ..Choice::default()
                    });
                }
                p.scenes.push(s);
            });
            app.selected_scene = app.document.project.scenes.len() - 1;
            app.selected_item = 0;
            app.graph.selection.clear();
            app.studio.pane = if kind.interface() {
                Pane::Interface
            } else if kind == NodeKind::Music {
                Pane::Audio
            } else if kind.interactive() {
                Pane::Text
            } else {
                Pane::Logic
            };
            app.choices_tab = kind == NodeKind::Choice;
        }
        KIND => {
            let kind = KINDS[app
                .controls
                .selection(id, true)
                .unwrap_or(0)
                .min(KINDS.len() - 1)];
            app.document.change(|p| {
                p.scenes[i].kind = kind;
                if kind.ui_element() {
                    p.scenes[i].ui.get_or_insert_with(Default::default);
                }
            });
            if kind.interface() {
                app.studio.pane = Pane::Interface;
            }
        }
        RULE_VAR | RULE_OP | MUSIC_MODE => {
            return Ok(true);
        }
        OTHERWISE => {
            let n = app.controls.selection(id, true).unwrap_or(0);
            app.document.change(|p| {
                p.scenes[i].otherwise = if n == 0 {
                    None
                } else {
                    Some(p.scenes[n - 1].id.clone())
                }
            });
        }
        APPLY_RULE => {
            let n = app.controls.selection(RULE_VAR, true).unwrap_or(0);
            let variable = n
                .checked_sub(1)
                .and_then(|i| app.document.project.variables.keys().nth(i))
                .cloned();
            let value = number(app, RULE_VALUE, i64::MIN, i64::MAX)?;
            let op = app.controls.selection(RULE_OP, true).unwrap_or(0);
            let choices = app.choices_tab;
            if !choices
                && matches!(
                    app.scene().kind,
                    NodeKind::Condition | NodeKind::SetVariable
                )
                && variable.is_none()
            {
                return Err("Выберите переменную.".into());
            }
            app.document.change(|p| {
                let scene = &mut p.scenes[i];
                if !choices && scene.kind == NodeKind::SetVariable {
                    scene.assignment = variable.map(|variable| Assignment {
                        variable,
                        value,
                        operation: [Operation::Set, Operation::Add, Operation::Subtract][op.min(2)],
                    });
                } else {
                    let condition = variable.map(|variable| Condition {
                        variable,
                        value,
                        comparison: [
                            Comparison::Equal,
                            Comparison::NotEqual,
                            Comparison::Greater,
                            Comparison::AtLeast,
                            Comparison::Less,
                            Comparison::AtMost,
                        ][op.min(5)],
                    });
                    if choices {
                        if let Some(c) = scene.choices.get_mut(item) {
                            c.condition = condition;
                        }
                    } else {
                        scene.condition = condition;
                    }
                }
            });
        }
        VARIABLES => {
            app.studio.variable = app.controls.selection(id, true).unwrap_or(0);
        }
        VAR_ADD | VAR_UPDATE | VAR_DELETE => {
            let name = app.controls.read(VAR_NAME).trim().to_owned();
            let value = number(app, VAR_VALUE, i64::MIN, i64::MAX)?;
            if name.is_empty() || name.chars().count() > 64 {
                return Err("Имя переменной должно содержать от 1 до 64 символов.".into());
            }
            let old = app
                .document
                .project
                .variables
                .keys()
                .nth(app.studio.variable)
                .cloned();
            if id == VAR_ADD && app.document.project.variables.contains_key(&name) {
                return Err("Такое имя уже существует.".into());
            }
            if id == VAR_UPDATE
                && old.as_deref() != Some(&name)
                && app.document.project.variables.contains_key(&name)
            {
                return Err("Такое имя уже существует.".into());
            }
            if id == VAR_DELETE
                && old.as_ref().is_some_and(|name| {
                    app.document.project.scenes.iter().any(|s| {
                        s.condition.as_ref().is_some_and(|c| &c.variable == name)
                            || s.assignment.as_ref().is_some_and(|a| &a.variable == name)
                            || s.ui
                                .as_ref()
                                .is_some_and(|ui| ui.text.contains(&format!("{{{name}}}")))
                            || s.choices
                                .iter()
                                .any(|c| c.condition.as_ref().is_some_and(|c| &c.variable == name))
                    })
                })
            {
                return Err("Переменная используется в сценарии.".into());
            }
            app.document.change(|p| {
                if id != VAR_ADD {
                    if let Some(old) = &old {
                        p.variables.remove(old);
                        if id == VAR_UPDATE {
                            for s in &mut p.scenes {
                                if let Some(ui) = &mut s.ui {
                                    ui.text = ui
                                        .text
                                        .replace(&format!("{{{old}}}"), &format!("{{{name}}}"));
                                }
                                for c in s.condition.iter_mut().chain(
                                    s.choices.iter_mut().filter_map(|c| c.condition.as_mut()),
                                ) {
                                    if &c.variable == old {
                                        c.variable = name.clone();
                                    }
                                }
                                if let Some(a) = &mut s.assignment {
                                    if &a.variable == old {
                                        a.variable = name.clone();
                                    }
                                }
                            }
                        }
                    }
                }
                if id != VAR_DELETE {
                    p.variables.insert(name.clone(), value);
                }
            });
            app.studio.variable = app
                .document
                .project
                .variables
                .keys()
                .position(|v| v == &name)
                .unwrap_or(0);
        }
        MUSIC_FILE | EFFECT_FILE | VOICE_FILE => {
            if app.file.is_none() && !app.save(false)? {
                return Ok(true);
            }
            if let Some(source) = ui::audio_dialog(app.hwnd, app.t("Выберите аудиофайл"))?
            {
                let asset = storage::import_audio(&app.root(), &source)?;
                app.document.change(|p| {
                    if id == VOICE_FILE {
                        if let Some(l) = p.scenes[i].lines.get_mut(item) {
                            l.voice = Some(asset);
                        }
                    } else {
                        let cue = p.scenes[i].audio.get_or_insert_with(AudioCue::new);
                        if id == MUSIC_FILE {
                            cue.music = Some(asset);
                            cue.stop_music = false;
                        } else {
                            cue.effect = Some(asset);
                        }
                    }
                });
            }
        }
        EFFECT_CLEAR | VOICE_CLEAR => app.document.change(|p| {
            if id == VOICE_CLEAR {
                if let Some(l) = p.scenes[i].lines.get_mut(item) {
                    l.voice = None;
                }
            } else if let Some(c) = &mut p.scenes[i].audio {
                c.effect = None;
            }
        }),
        AUDIO_APPLY => {
            let mode = app.controls.selection(MUSIC_MODE, true).unwrap_or(0);
            let volume = number(app, MUSIC_VOLUME, 0, 100)? as u8;
            let fade = number(app, FADE, 0, 10_000)? as u32;
            if mode == 1
                && app
                    .scene()
                    .audio
                    .as_ref()
                    .and_then(|a| a.music.as_ref())
                    .is_none()
            {
                return Err("Выберите музыку для сцены.".into());
            }
            app.document.change(|p| {
                let cue = p.scenes[i].audio.get_or_insert_with(AudioCue::new);
                cue.volume = volume;
                cue.fade_ms = fade;
                cue.stop_music = mode == 2;
                if mode != 1 {
                    cue.music = None;
                }
            });
        }
        AUDIO_PREVIEW => {
            let cue = app.scene().audio.clone().unwrap_or_default();
            app.audio.play(audio::Request {
                root: app.root(),
                music: cue.music.map(|file| rinva_core::MusicState {
                    file,
                    volume: cue.volume,
                    fade_ms: cue.fade_ms,
                }),
                voice: app.scene().lines.get(item).and_then(|l| l.voice.clone()),
                effects: cue.effect.into_iter().collect(),
                volumes: app.reader.settings.volumes,
                fade_ms: cue.fade_ms,
            });
        }
        AUDIO_STOP => {}
        CHARACTERS => {
            app.studio.character = app.controls.selection(id, true).unwrap_or(0);
            app.studio.expression = 0;
        }
        EXPRESSIONS => {
            app.studio.expression = app.controls.selection(id, true).unwrap_or(0);
        }
        CHARACTER_ADD => {
            let name = format!(
                "{} {}",
                app.t("Персонаж"),
                app.document.project.characters.len() + 1
            );
            app.document.change(|p| {
                let id = (1..)
                    .map(|n| format!("character-{n}"))
                    .find(|id| !p.characters.iter().any(|c| &c.id == id))
                    .unwrap();
                p.characters.push(Character {
                    id,
                    name,
                    expressions: vec![],
                });
            });
            app.studio.character = app.document.project.characters.len() - 1;
            app.studio.expression = 0;
        }
        CHARACTER_DELETE => {
            if let Some(c) = app.document.project.characters.get(app.studio.character) {
                if app
                    .document
                    .project
                    .scenes
                    .iter()
                    .any(|s| s.sprites.iter().any(|s| s.character == c.id))
                {
                    return Err("Персонаж используется в сценах.".into());
                }
                let n = app.studio.character;
                app.document.change(|p| {
                    p.characters.remove(n);
                });
            }
        }
        EXPRESSION_ADD | EXPRESSION_IMAGE => {
            if app
                .document
                .project
                .characters
                .get(app.studio.character)
                .is_none()
            {
                return Ok(true);
            }
            if app.file.is_none() && !app.save(false)? {
                return Ok(true);
            }
            if let Some(source) = ui::file_dialog(
                app.hwnd,
                app.language,
                false,
                true,
                "",
                app.t("Выберите PNG персонажа"),
            )? {
                let _checked = paint::Bitmap::load(&source)?;
                let image = storage::import_asset(&app.root(), &source)?;
                let c = app.studio.character;
                let e = app.studio.expression;
                let name = app.t("Новая эмоция").to_owned();
                app.document.change(|p| {
                    let character = &mut p.characters[c];
                    if id == EXPRESSION_ADD {
                        let id = (1..)
                            .map(|n| format!("expression-{n}"))
                            .find(|id| !character.expressions.iter().any(|e| &e.id == id))
                            .unwrap();
                        character.expressions.push(Expression { id, name, image });
                    } else if let Some(e) = character.expressions.get_mut(e) {
                        e.image = image;
                    }
                });
                if id == EXPRESSION_ADD {
                    app.studio.expression =
                        app.document.project.characters[c].expressions.len() - 1;
                }
            }
        }
        SPRITES => {
            app.studio.sprite = app.controls.selection(id, false).unwrap_or(0);
            if let Some(s) = app.scene().sprites.get(app.studio.sprite) {
                if let Some(c) = app
                    .document
                    .project
                    .characters
                    .iter()
                    .position(|c| c.id == s.character)
                {
                    let e = app.document.project.characters[c]
                        .expressions
                        .iter()
                        .position(|e| e.id == s.expression)
                        .unwrap_or(0);
                    app.studio.character = c;
                    app.studio.expression = e;
                }
            }
        }
        SPRITE_ADD => {
            if let Some(c) = app.document.project.characters.get(app.studio.character) {
                if let Some(e) = c.expressions.get(app.studio.expression) {
                    let sprite = Sprite::new(c.id.clone(), e.id.clone());
                    app.document.change(|p| p.scenes[i].sprites.push(sprite));
                    app.studio.sprite = app.scene().sprites.len() - 1;
                }
            }
        }
        SPRITE_DELETE => {
            let n = app.studio.sprite;
            if n < app.scene().sprites.len() {
                app.document.change(|p| {
                    p.scenes[i].sprites.remove(n);
                });
            }
        }
        CHARACTER_APPLY => {
            let c = app.studio.character;
            let e = app.studio.expression;
            let n = app.studio.sprite;
            let name = app.controls.read(CHARACTER_NAME).trim().to_owned();
            let emotion = app.controls.read(EXPRESSION_NAME).trim().to_owned();
            if name.is_empty() {
                return Err("Введите имя персонажа.".into());
            }
            if app
                .document
                .project
                .characters
                .iter()
                .enumerate()
                .any(|(index, character)| index != c && character.name == name)
            {
                return Err("Такое имя уже существует.".into());
            }
            let transform = if n < app.scene().sprites.len() {
                Some((
                    number(app, SPRITE_X, -50, 150)? as i32,
                    number(app, SPRITE_Y, -50, 150)? as i32,
                    number(app, SPRITE_SCALE, 10, 300)? as u32,
                    number(app, SPRITE_LAYER, -1000, 1000)? as i32,
                ))
            } else {
                None
            };
            if app
                .document
                .project
                .characters
                .get(c)
                .and_then(|c| c.expressions.get(e))
                .is_some()
                && emotion.is_empty()
            {
                return Err("Введите имя эмоции.".into());
            }
            app.document.change(|p| {
                if let Some(character) = p.characters.get_mut(c) {
                    let old = std::mem::replace(&mut character.name, name.clone());
                    for s in &mut p.scenes {
                        for l in &mut s.lines {
                            if l.speaker == old {
                                l.speaker = name.clone();
                            }
                        }
                    }
                    if let Some(expression) = character.expressions.get_mut(e) {
                        expression.name = emotion;
                        if let Some((x, y, scale, layer)) = transform {
                            let s = &mut p.scenes[i].sprites[n];
                            s.character = character.id.clone();
                            s.expression = expression.id.clone();
                            s.x = x;
                            s.y = y;
                            s.scale = scale;
                            s.layer = layer;
                        }
                    }
                }
            });
        }
        NOTES_APPLY => {
            let group = app.controls.read(GROUP).trim().to_owned();
            let note = app.controls.read(NOTE);
            let selected = app.graph.selection.clone();
            app.document.change(|p| {
                for (n, s) in p.scenes.iter_mut().enumerate() {
                    if n == i || selected.contains(&s.id) {
                        s.group = group.clone();
                    }
                }
                p.scenes[i].note = note;
            });
        }
        SELECT_GROUP => {
            let group = app.scene().group.clone();
            app.graph.selection = app
                .document
                .project
                .scenes
                .iter()
                .filter(|s| s.group == group)
                .map(|s| s.id.clone())
                .collect();
        }
        LEFT_WIDTH => {
            app.left_width = [180, 224, 280][app.controls.selection(id, true).unwrap_or(1).min(2)];
            app.persist_settings()?;
        }
        RIGHT_WIDTH => {
            app.right_width = [280, 300, 380][app.controls.selection(id, true).unwrap_or(1).min(2)];
            app.persist_settings()?;
        }
        RESET_LAYOUT => {
            app.left_width = 224;
            app.right_width = 300;
            app.preview_share = 45;
            app.persist_settings()?;
        }
        PREVIEW_HEIGHT => {
            if let Ok(value) = app.controls.read(id).trim_end_matches('%').parse::<i32>() {
                app.preview_share = value.clamp(25, 65);
                app.persist_settings()?;
            }
        }
        MINIMAP => {
            app.graph.minimap = !app.graph.minimap;
        }
        _ => return Ok(true),
    }
    app.navigation.changed();
    app.image_key.clear();
    app.refresh();
    Ok(true)
}
