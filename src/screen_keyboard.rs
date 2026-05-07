// ./src/screen_keyboard.rs

pub const FLICK_THRESHOLD_PX: f32 = 20.0;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ScreenKeyboardLayoutKind {
    #[default]
    KanaFlick,
    Qwerty,
}

impl ScreenKeyboardLayoutKind {
    pub const fn bridge_label(self) -> &'static str {
        match self {
            Self::KanaFlick => "kana-flick",
            Self::Qwerty => "qwerty",
        }
    }

    pub const fn display_label(self) -> &'static str {
        match self {
            Self::KanaFlick => "かな",
            Self::Qwerty => "ABC",
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::KanaFlick => Self::Qwerty,
            Self::Qwerty => Self::KanaFlick,
        }
    }

    pub fn from_bridge_label(value: &str) -> Option<Self> {
        match value {
            "kana-flick" => Some(Self::KanaFlick),
            "qwerty" => Some(Self::Qwerty),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlickDirection {
    Center,
    Up,
    Right,
    Down,
    Left,
}

impl FlickDirection {
    pub fn from_delta(dx: f32, dy: f32) -> Self {
        let threshold_sq = FLICK_THRESHOLD_PX * FLICK_THRESHOLD_PX;
        if dx * dx + dy * dy < threshold_sq {
            return Self::Center;
        }

        if dy * dy > dx * dx {
            if dy < 0.0 {
                Self::Up
            } else {
                Self::Down
            }
        } else if dx < 0.0 {
            Self::Left
        } else {
            Self::Right
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenKeyboardUiCommand {
    Backspace,
    Enter,
    Escape,
    Up,
    Down,
    CycleTuiMode,
}

impl ScreenKeyboardUiCommand {
    pub const fn bridge_label(self) -> &'static str {
        match self {
            Self::Backspace => "Backspace",
            Self::Enter => "Enter",
            Self::Escape => "Escape",
            Self::Up => "Up",
            Self::Down => "Down",
            Self::CycleTuiMode => "CycleTuiMode",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenKeyboardAction {
    Text(&'static str),
    UiCommand(ScreenKeyboardUiCommand),
    TransformLastText,
    SwitchLayout,
    SwitchInputSource,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenKeyboardKeyClass {
    Text,
    Special,
    Backspace,
    Enter,
    Modifier,
    Spacer,
}

impl ScreenKeyboardKeyClass {
    pub const fn bridge_label(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Special => "special",
            Self::Backspace => "backspace",
            Self::Enter => "enter",
            Self::Modifier => "modifier",
            Self::Spacer => "spacer",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenKeyboardKeyWidth {
    Normal,
    Wide,
    Space,
}

impl ScreenKeyboardKeyWidth {
    pub const fn bridge_label(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Wide => "wide",
            Self::Space => "space",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlickKeyMap {
    pub center: &'static str,
    pub up: Option<&'static str>,
    pub right: Option<&'static str>,
    pub down: Option<&'static str>,
    pub left: Option<&'static str>,
}

impl FlickKeyMap {
    pub const fn new(
        center: &'static str,
        up: Option<&'static str>,
        right: Option<&'static str>,
        down: Option<&'static str>,
        left: Option<&'static str>,
    ) -> Self {
        Self {
            center,
            up,
            right,
            down,
            left,
        }
    }

    pub const fn text_for(self, direction: FlickDirection) -> Option<&'static str> {
        match direction {
            FlickDirection::Center => Some(self.center),
            FlickDirection::Up => self.up,
            FlickDirection::Right => self.right,
            FlickDirection::Down => self.down,
            FlickDirection::Left => self.left,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenKeyboardKeyRole {
    Flick(FlickKeyMap),
    Action(ScreenKeyboardAction),
    Spacer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenKeyboardKey {
    pub label: &'static str,
    pub role: ScreenKeyboardKeyRole,
    pub class: ScreenKeyboardKeyClass,
    pub width: ScreenKeyboardKeyWidth,
}

impl ScreenKeyboardKey {
    pub const fn flick(
        label: &'static str,
        center: &'static str,
        up: Option<&'static str>,
        right: Option<&'static str>,
        down: Option<&'static str>,
        left: Option<&'static str>,
    ) -> Self {
        Self {
            label,
            role: ScreenKeyboardKeyRole::Flick(FlickKeyMap::new(center, up, right, down, left)),
            class: ScreenKeyboardKeyClass::Text,
            width: ScreenKeyboardKeyWidth::Normal,
        }
    }

    pub const fn action(
        label: &'static str,
        action: ScreenKeyboardAction,
        class: ScreenKeyboardKeyClass,
        width: ScreenKeyboardKeyWidth,
    ) -> Self {
        Self {
            label,
            role: ScreenKeyboardKeyRole::Action(action),
            class,
            width,
        }
    }

    pub const fn text(label: &'static str, text: &'static str) -> Self {
        Self::action(
            label,
            ScreenKeyboardAction::Text(text),
            ScreenKeyboardKeyClass::Text,
            ScreenKeyboardKeyWidth::Normal,
        )
    }

    pub const fn spacer() -> Self {
        Self {
            label: "",
            role: ScreenKeyboardKeyRole::Spacer,
            class: ScreenKeyboardKeyClass::Spacer,
            width: ScreenKeyboardKeyWidth::Normal,
        }
    }

    pub fn resolve(self, dx: f32, dy: f32) -> ScreenKeyboardAction {
        match self.role {
            ScreenKeyboardKeyRole::Flick(map) => map
                .text_for(FlickDirection::from_delta(dx, dy))
                .map_or(ScreenKeyboardAction::None, ScreenKeyboardAction::Text),
            ScreenKeyboardKeyRole::Action(action) => action,
            ScreenKeyboardKeyRole::Spacer => ScreenKeyboardAction::None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenKeyboardRow {
    pub keys: &'static [ScreenKeyboardKey],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenKeyboardLayout {
    pub kind: ScreenKeyboardLayoutKind,
    pub label: &'static str,
    pub rows: &'static [ScreenKeyboardRow],
}

const KANA_ROW_0: [ScreenKeyboardKey; 5] = [
    ScreenKeyboardKey::action(
        "Esc",
        ScreenKeyboardAction::UiCommand(ScreenKeyboardUiCommand::Escape),
        ScreenKeyboardKeyClass::Special,
        ScreenKeyboardKeyWidth::Normal,
    ),
    ScreenKeyboardKey::flick("あ", "あ", Some("う"), Some("え"), Some("お"), Some("い")),
    ScreenKeyboardKey::flick("か", "か", Some("く"), Some("け"), Some("こ"), Some("き")),
    ScreenKeyboardKey::flick("さ", "さ", Some("す"), Some("せ"), Some("そ"), Some("し")),
    ScreenKeyboardKey::action(
        "⌫",
        ScreenKeyboardAction::UiCommand(ScreenKeyboardUiCommand::Backspace),
        ScreenKeyboardKeyClass::Backspace,
        ScreenKeyboardKeyWidth::Normal,
    ),
];

const KANA_ROW_1: [ScreenKeyboardKey; 5] = [
    ScreenKeyboardKey::action(
        "▲",
        ScreenKeyboardAction::UiCommand(ScreenKeyboardUiCommand::Up),
        ScreenKeyboardKeyClass::Special,
        ScreenKeyboardKeyWidth::Normal,
    ),
    ScreenKeyboardKey::flick("た", "た", Some("つ"), Some("て"), Some("と"), Some("ち")),
    ScreenKeyboardKey::flick("な", "な", Some("ぬ"), Some("ね"), Some("の"), Some("に")),
    ScreenKeyboardKey::flick("は", "は", Some("ふ"), Some("へ"), Some("ほ"), Some("ひ")),
    ScreenKeyboardKey::action(
        "▼",
        ScreenKeyboardAction::UiCommand(ScreenKeyboardUiCommand::Down),
        ScreenKeyboardKeyClass::Special,
        ScreenKeyboardKeyWidth::Normal,
    ),
];

const KANA_ROW_2: [ScreenKeyboardKey; 5] = [
    ScreenKeyboardKey::action(
        "ABC",
        ScreenKeyboardAction::SwitchLayout,
        ScreenKeyboardKeyClass::Special,
        ScreenKeyboardKeyWidth::Normal,
    ),
    ScreenKeyboardKey::flick("ま", "ま", Some("む"), Some("め"), Some("も"), Some("み")),
    ScreenKeyboardKey::flick("や", "や", Some("ゆ"), Some("ゅ"), Some("よ"), Some("ゃ")),
    ScreenKeyboardKey::flick("ら", "ら", Some("る"), Some("れ"), Some("ろ"), Some("り")),
    ScreenKeyboardKey::action(
        "ー",
        ScreenKeyboardAction::Text("ー"),
        ScreenKeyboardKeyClass::Special,
        ScreenKeyboardKeyWidth::Normal,
    ),
];

const KANA_ROW_3: [ScreenKeyboardKey; 5] = [
    ScreenKeyboardKey::spacer(),
    ScreenKeyboardKey::action(
        "大⇔小",
        ScreenKeyboardAction::TransformLastText,
        ScreenKeyboardKeyClass::Modifier,
        ScreenKeyboardKeyWidth::Normal,
    ),
    ScreenKeyboardKey::flick("わ", "わ", Some("ん"), None, None, Some("を")),
    ScreenKeyboardKey::flick("。", "。", Some("？"), Some("！"), Some("…"), Some("、")),
    ScreenKeyboardKey::action(
        "↩",
        ScreenKeyboardAction::UiCommand(ScreenKeyboardUiCommand::Enter),
        ScreenKeyboardKeyClass::Enter,
        ScreenKeyboardKeyWidth::Normal,
    ),
];

const KANA_ROWS: [ScreenKeyboardRow; 4] = [
    ScreenKeyboardRow { keys: &KANA_ROW_0 },
    ScreenKeyboardRow { keys: &KANA_ROW_1 },
    ScreenKeyboardRow { keys: &KANA_ROW_2 },
    ScreenKeyboardRow { keys: &KANA_ROW_3 },
];

const KANA_LAYOUT: ScreenKeyboardLayout = ScreenKeyboardLayout {
    kind: ScreenKeyboardLayoutKind::KanaFlick,
    label: "かな",
    rows: &KANA_ROWS,
};

const QWERTY_ROW_0: [ScreenKeyboardKey; 12] = [
    ScreenKeyboardKey::action(
        "Esc",
        ScreenKeyboardAction::UiCommand(ScreenKeyboardUiCommand::Escape),
        ScreenKeyboardKeyClass::Special,
        ScreenKeyboardKeyWidth::Normal,
    ),
    ScreenKeyboardKey::text("q", "q"),
    ScreenKeyboardKey::text("w", "w"),
    ScreenKeyboardKey::text("e", "e"),
    ScreenKeyboardKey::text("r", "r"),
    ScreenKeyboardKey::text("t", "t"),
    ScreenKeyboardKey::text("y", "y"),
    ScreenKeyboardKey::text("u", "u"),
    ScreenKeyboardKey::text("i", "i"),
    ScreenKeyboardKey::text("o", "o"),
    ScreenKeyboardKey::text("p", "p"),
    ScreenKeyboardKey::action(
        "⌫",
        ScreenKeyboardAction::UiCommand(ScreenKeyboardUiCommand::Backspace),
        ScreenKeyboardKeyClass::Backspace,
        ScreenKeyboardKeyWidth::Normal,
    ),
];

const QWERTY_ROW_1: [ScreenKeyboardKey; 9] = [
    ScreenKeyboardKey::text("a", "a"),
    ScreenKeyboardKey::text("s", "s"),
    ScreenKeyboardKey::text("d", "d"),
    ScreenKeyboardKey::text("f", "f"),
    ScreenKeyboardKey::text("g", "g"),
    ScreenKeyboardKey::text("h", "h"),
    ScreenKeyboardKey::text("j", "j"),
    ScreenKeyboardKey::text("k", "k"),
    ScreenKeyboardKey::text("l", "l"),
];

const QWERTY_ROW_2: [ScreenKeyboardKey; 11] = [
    ScreenKeyboardKey::action(
        "かな",
        ScreenKeyboardAction::SwitchLayout,
        ScreenKeyboardKeyClass::Special,
        ScreenKeyboardKeyWidth::Wide,
    ),
    ScreenKeyboardKey::text("z", "z"),
    ScreenKeyboardKey::text("x", "x"),
    ScreenKeyboardKey::text("c", "c"),
    ScreenKeyboardKey::text("v", "v"),
    ScreenKeyboardKey::text("b", "b"),
    ScreenKeyboardKey::text("n", "n"),
    ScreenKeyboardKey::text("m", "m"),
    ScreenKeyboardKey::text(",", ","),
    ScreenKeyboardKey::text(".", "."),
    ScreenKeyboardKey::text("?", "?"),
];

const QWERTY_ROW_3: [ScreenKeyboardKey; 2] = [
    ScreenKeyboardKey::action(
        "Space",
        ScreenKeyboardAction::Text(" "),
        ScreenKeyboardKeyClass::Text,
        ScreenKeyboardKeyWidth::Space,
    ),
    ScreenKeyboardKey::action(
        "↩",
        ScreenKeyboardAction::UiCommand(ScreenKeyboardUiCommand::Enter),
        ScreenKeyboardKeyClass::Enter,
        ScreenKeyboardKeyWidth::Wide,
    ),
];

const QWERTY_ROWS: [ScreenKeyboardRow; 4] = [
    ScreenKeyboardRow {
        keys: &QWERTY_ROW_0,
    },
    ScreenKeyboardRow {
        keys: &QWERTY_ROW_1,
    },
    ScreenKeyboardRow {
        keys: &QWERTY_ROW_2,
    },
    ScreenKeyboardRow {
        keys: &QWERTY_ROW_3,
    },
];

const QWERTY_LAYOUT: ScreenKeyboardLayout = ScreenKeyboardLayout {
    kind: ScreenKeyboardLayoutKind::Qwerty,
    label: "ABC",
    rows: &QWERTY_ROWS,
};

pub const fn layout(kind: ScreenKeyboardLayoutKind) -> &'static ScreenKeyboardLayout {
    match kind {
        ScreenKeyboardLayoutKind::KanaFlick => &KANA_LAYOUT,
        ScreenKeyboardLayoutKind::Qwerty => &QWERTY_LAYOUT,
    }
}

pub fn resolve_key(
    kind: ScreenKeyboardLayoutKind,
    row_index: usize,
    key_index: usize,
    dx: f32,
    dy: f32,
) -> ScreenKeyboardAction {
    layout(kind)
        .rows
        .get(row_index)
        .and_then(|row| row.keys.get(key_index))
        .map_or(ScreenKeyboardAction::None, |key| key.resolve(dx, dy))
}

pub fn modified_kana(c: char) -> Option<char> {
    match c {
        'か' => Some('が'),
        'が' => Some('か'),
        'き' => Some('ぎ'),
        'ぎ' => Some('き'),
        'く' => Some('ぐ'),
        'ぐ' => Some('く'),
        'け' => Some('げ'),
        'げ' => Some('け'),
        'こ' => Some('ご'),
        'ご' => Some('こ'),
        'さ' => Some('ざ'),
        'ざ' => Some('さ'),
        'し' => Some('じ'),
        'じ' => Some('し'),
        'す' => Some('ず'),
        'ず' => Some('す'),
        'せ' => Some('ぜ'),
        'ぜ' => Some('せ'),
        'そ' => Some('ぞ'),
        'ぞ' => Some('そ'),
        'た' => Some('だ'),
        'だ' => Some('た'),
        'ち' => Some('ぢ'),
        'ぢ' => Some('ち'),
        'つ' => Some('っ'),
        'っ' => Some('づ'),
        'づ' => Some('つ'),
        'て' => Some('で'),
        'で' => Some('て'),
        'と' => Some('ど'),
        'ど' => Some('と'),
        'は' => Some('ば'),
        'ば' => Some('ぱ'),
        'ぱ' => Some('は'),
        'ひ' => Some('び'),
        'び' => Some('ぴ'),
        'ぴ' => Some('ひ'),
        'ふ' => Some('ぶ'),
        'ぶ' => Some('ぷ'),
        'ぷ' => Some('ふ'),
        'へ' => Some('べ'),
        'べ' => Some('ぺ'),
        'ぺ' => Some('へ'),
        'ほ' => Some('ぼ'),
        'ぼ' => Some('ぽ'),
        'ぽ' => Some('ほ'),
        'う' => Some('ゔ'),
        'ゔ' => Some('う'),
        'あ' => Some('ぁ'),
        'ぁ' => Some('あ'),
        'い' => Some('ぃ'),
        'ぃ' => Some('い'),
        'え' => Some('ぇ'),
        'ぇ' => Some('え'),
        'お' => Some('ぉ'),
        'ぉ' => Some('お'),
        'や' => Some('ゃ'),
        'ゃ' => Some('や'),
        'ゆ' => Some('ゅ'),
        'ゅ' => Some('ゆ'),
        'よ' => Some('ょ'),
        'ょ' => Some('よ'),
        'わ' => Some('ゎ'),
        'ゎ' => Some('わ'),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScreenKeyboardInputState {
    last_text: Option<char>,
    last_accepted: bool,
}

impl ScreenKeyboardInputState {
    pub const fn new() -> Self {
        Self {
            last_text: None,
            last_accepted: false,
        }
    }

    pub fn record_text(&mut self, text: &str, accepted: bool) {
        let mut chars = text.chars();
        match (chars.next(), chars.next()) {
            (Some(c), None) => {
                self.last_text = Some(c);
                self.last_accepted = accepted;
            }
            _ => self.clear(),
        }
    }

    pub fn clear(&mut self) {
        self.last_text = None;
        self.last_accepted = false;
    }

    pub fn pending_modified_char(self) -> Option<char> {
        match (self.last_text, self.last_accepted) {
            (Some(c), false) => modified_kana(c),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flick_direction_uses_distance_threshold_and_dominant_axis() {
        assert_eq!(FlickDirection::from_delta(0.0, 0.0), FlickDirection::Center);
        assert_eq!(FlickDirection::from_delta(0.0, -20.0), FlickDirection::Up);
        assert_eq!(FlickDirection::from_delta(0.0, 25.0), FlickDirection::Down);
        assert_eq!(FlickDirection::from_delta(24.0, 3.0), FlickDirection::Right);
        assert_eq!(FlickDirection::from_delta(-24.0, 3.0), FlickDirection::Left);
    }

    #[test]
    fn kana_flick_layout_resolves_shared_actions() {
        assert_eq!(
            resolve_key(ScreenKeyboardLayoutKind::KanaFlick, 0, 1, 0.0, 0.0),
            ScreenKeyboardAction::Text("あ")
        );
        assert_eq!(
            resolve_key(ScreenKeyboardLayoutKind::KanaFlick, 0, 1, 0.0, -25.0),
            ScreenKeyboardAction::Text("う")
        );
        assert_eq!(
            resolve_key(ScreenKeyboardLayoutKind::KanaFlick, 2, 0, 0.0, 0.0),
            ScreenKeyboardAction::SwitchLayout
        );
        assert_eq!(
            resolve_key(ScreenKeyboardLayoutKind::KanaFlick, 3, 1, 0.0, 0.0),
            ScreenKeyboardAction::TransformLastText
        );
    }

    #[test]
    fn qwerty_layout_outputs_text_and_can_switch_back() {
        assert_eq!(
            resolve_key(ScreenKeyboardLayoutKind::Qwerty, 0, 1, 100.0, 0.0),
            ScreenKeyboardAction::Text("q")
        );
        assert_eq!(
            resolve_key(ScreenKeyboardLayoutKind::Qwerty, 2, 0, 0.0, 0.0),
            ScreenKeyboardAction::SwitchLayout
        );
        assert_eq!(
            resolve_key(ScreenKeyboardLayoutKind::Qwerty, 3, 0, 0.0, 0.0),
            ScreenKeyboardAction::Text(" ")
        );
    }

    #[test]
    fn layout_kind_switches_exhaustively() {
        assert_eq!(
            ScreenKeyboardLayoutKind::KanaFlick.next(),
            ScreenKeyboardLayoutKind::Qwerty
        );
        assert_eq!(
            ScreenKeyboardLayoutKind::Qwerty.next(),
            ScreenKeyboardLayoutKind::KanaFlick
        );
    }

    #[test]
    fn modifier_tracks_pending_single_character_input() {
        let mut state = ScreenKeyboardInputState::new();
        state.record_text("か", false);
        assert_eq!(state.pending_modified_char(), Some('が'));

        state.record_text("が", true);
        assert_eq!(state.pending_modified_char(), None);

        state.record_text("Space", false);
        assert_eq!(state.pending_modified_char(), None);
    }
}
