// ./src/app.rs

extern crate alloc;

mod problems;
mod scroll;
mod view;

use crate::app::scroll::{
    build_scroll_line_cache, cursor_position_from_status, line_origin_from_previous,
    line_origin_from_start, typing_line_scroll_position, ScrollCacheState,
};
use crate::display::DisplaySettings;
pub use crate::font::{FontBundle, FontScale, FontScript as Script, FontTarget, Fonts};
use crate::io::{FontAssetId, FontEntry, ProblemRepository};
use crate::model::{
    CharIndex, LineIndex, ResultModel, Scroll, SegmentIndex, TypingModel, TypingStatus, WordIndex,
};
use crate::screen_keyboard::ScreenKeyboardUiCommand;
use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};

pub use crate::io::{CustomProblem, FontSource};
use crate::parser;
use crate::typing;
use crate::ui; // typing_rendererの代わりにuiをインポート
use ab_glyph::FontVec;

pub(crate) use scroll::{typing_line_scroll_offset, ScrollCache};
pub use view::AppSnapshot;

// ビルドスクリプトによってOUT_DIRに生成されたファイルを取り込む
include!(concat!(env!("OUT_DIR"), "/problem_files.rs"));

/// アプリケーションの現在の状態（シーン）を定義するenum
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum AppState {
    MainMenu,
    ProblemSelection,
    ProblemSource, // 問題ファイルのソースを閲覧するシーン
    Typing,
    Result,
    Settings,
    HowToUse, // 使い方説明シーン
}

/// TUIの描画モードを定義するenum
#[derive(PartialEq, Clone, Copy, Debug)]
pub enum TuiDisplayMode {
    AsciiArt,
    SimpleText,
    Braille,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainMenuItem {
    Start,
    HowToUse,
    Settings,
    Quit,
}

impl MainMenuItem {
    pub const fn index(self) -> usize {
        match self {
            Self::Start => 0,
            Self::HowToUse => 1,
            Self::Settings => 2,
            Self::Quit => 3,
        }
    }

    pub fn previous(self) -> Self {
        match self {
            Self::Start => Self::Start,
            Self::HowToUse => Self::Start,
            Self::Settings => Self::HowToUse,
            Self::Quit => Self::Settings,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Start => Self::HowToUse,
            Self::HowToUse => Self::Settings,
            Self::Settings => {
                #[cfg(feature = "wasm")]
                {
                    Self::Settings
                }
                #[cfg(not(feature = "wasm"))]
                {
                    Self::Quit
                }
            }
            Self::Quit => Self::Quit,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsItem {
    FontFamily(FontTarget),
    FontScale(FontTarget),
    AspectRatio,
    DisplayScale,
    ImeInput,
}

const SETTINGS_ITEMS: &[SettingsItem] = &[
    SettingsItem::FontFamily(FontTarget::Ui),
    SettingsItem::FontScale(FontTarget::Ui),
    SettingsItem::FontFamily(FontTarget::Script(Script::Japanese)),
    SettingsItem::FontScale(FontTarget::Script(Script::Japanese)),
    SettingsItem::FontFamily(FontTarget::Ruby(Script::Japanese)),
    SettingsItem::FontScale(FontTarget::Ruby(Script::Japanese)),
    SettingsItem::FontFamily(FontTarget::Unconfirmed(Script::Japanese)),
    SettingsItem::FontScale(FontTarget::Unconfirmed(Script::Japanese)),
    SettingsItem::FontFamily(FontTarget::Script(Script::ChineseSimplified)),
    SettingsItem::FontScale(FontTarget::Script(Script::ChineseSimplified)),
    SettingsItem::FontFamily(FontTarget::Ruby(Script::ChineseSimplified)),
    SettingsItem::FontScale(FontTarget::Ruby(Script::ChineseSimplified)),
    SettingsItem::FontFamily(FontTarget::Unconfirmed(Script::ChineseSimplified)),
    SettingsItem::FontScale(FontTarget::Unconfirmed(Script::ChineseSimplified)),
    SettingsItem::FontFamily(FontTarget::Script(Script::TraditionalChinese)),
    SettingsItem::FontScale(FontTarget::Script(Script::TraditionalChinese)),
    SettingsItem::FontFamily(FontTarget::Ruby(Script::TraditionalChinese)),
    SettingsItem::FontScale(FontTarget::Ruby(Script::TraditionalChinese)),
    SettingsItem::FontFamily(FontTarget::Unconfirmed(Script::TraditionalChinese)),
    SettingsItem::FontScale(FontTarget::Unconfirmed(Script::TraditionalChinese)),
    SettingsItem::FontFamily(FontTarget::Script(Script::English)),
    SettingsItem::FontScale(FontTarget::Script(Script::English)),
    SettingsItem::AspectRatio,
    SettingsItem::DisplayScale,
    SettingsItem::ImeInput,
];

impl SettingsItem {
    pub const fn all() -> &'static [Self] {
        SETTINGS_ITEMS
    }

    pub fn index(self) -> usize {
        SETTINGS_ITEMS
            .iter()
            .position(|item| *item == self)
            .unwrap_or(0)
    }

    pub const fn font_target(self) -> Option<FontTarget> {
        match self {
            Self::FontFamily(target) => Some(target),
            Self::FontScale(_) | Self::AspectRatio | Self::DisplayScale | Self::ImeInput => None,
        }
    }

    pub const fn font_scale_target(self) -> Option<FontTarget> {
        match self {
            Self::FontScale(target) => Some(target),
            Self::FontFamily(_) | Self::AspectRatio | Self::DisplayScale | Self::ImeInput => None,
        }
    }

    pub fn previous(self) -> Self {
        let index = self.index();
        if index == 0 {
            self
        } else {
            SETTINGS_ITEMS[index - 1]
        }
    }

    pub fn next(self) -> Self {
        let index = self.index();
        SETTINGS_ITEMS.get(index + 1).copied().unwrap_or(self)
    }
}

/// アプリケーションで発生するイベントを定義するenum
pub enum AppEvent {
    Start,
    ChangeScene,
    Char { c: char, timestamp: f64 },
    Backspace,
    Up,
    Down,
    Enter,
    Escape,
    CycleTuiMode,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiCommand {
    Backspace,
    Enter,
    Escape,
    Up,
    Down,
    CycleTuiMode,
}

impl UiCommand {
    pub fn from_bridge_label(value: &str) -> Option<Self> {
        match value {
            "Backspace" => Some(Self::Backspace),
            "Enter" => Some(Self::Enter),
            "Escape" => Some(Self::Escape),
            "Up" => Some(Self::Up),
            "Down" => Some(Self::Down),
            "CycleTuiMode" => Some(Self::CycleTuiMode),
            _ => None,
        }
    }

    pub fn from_web_key(value: &str) -> Option<Self> {
        match value {
            "ArrowUp" => Some(Self::Up),
            "ArrowDown" => Some(Self::Down),
            "Backspace" => Some(Self::Backspace),
            "Enter" => Some(Self::Enter),
            "Escape" => Some(Self::Escape),
            "Tab" => Some(Self::CycleTuiMode),
            _ => None,
        }
    }

    pub const fn app_event(self) -> AppEvent {
        match self {
            Self::Backspace => AppEvent::Backspace,
            Self::Enter => AppEvent::Enter,
            Self::Escape => AppEvent::Escape,
            Self::Up => AppEvent::Up,
            Self::Down => AppEvent::Down,
            Self::CycleTuiMode => AppEvent::CycleTuiMode,
        }
    }
}

impl From<ScreenKeyboardUiCommand> for UiCommand {
    fn from(value: ScreenKeyboardUiCommand) -> Self {
        match value {
            ScreenKeyboardUiCommand::Backspace => Self::Backspace,
            ScreenKeyboardUiCommand::Enter => Self::Enter,
            ScreenKeyboardUiCommand::Escape => Self::Escape,
            ScreenKeyboardUiCommand::Up => Self::Up,
            ScreenKeyboardUiCommand::Down => Self::Down,
            ScreenKeyboardUiCommand::CycleTuiMode => Self::CycleTuiMode,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontLoadRequest {
    pub target: FontTarget,
    pub font_id: FontAssetId,
    pub font_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontApplyError {
    InvalidFontData,
}

const FPS_DISPLAY_UPDATE_INTERVAL_MS: f64 = 250.0;

/// アプリケーション全体で共有される状態を保持する構造体
pub struct App {
    pub(crate) state: AppState,
    pub(crate) selected_main_menu_item: MainMenuItem,
    pub(crate) selected_problem_item: usize,
    pub(crate) selected_settings_item: SettingsItem,
    problem_repository: ProblemRepository,
    pub(crate) typing_model: Option<TypingModel>,
    pub(crate) result_model: Option<ResultModel>,
    pub(crate) status_text: String,
    pub(crate) instructions_text: String,
    pub(crate) tui_display_mode: TuiDisplayMode,
    pub(crate) should_quit: bool,
    /// ファイルダイアログを開く要求フラグ（gui-file/wasm のみ）
    pub(crate) should_open_file_dialog: bool,
    pub(crate) fonts: Fonts,
    pub(crate) display_settings: DisplaySettings,
    pub(crate) accept_ime_input: bool,
    /// フォントピッカーを開いているか
    pub(crate) settings_picking_font: bool,
    /// フォントピッカー内の選択インデックス
    pub(crate) selected_font_item: usize,
    /// 発見されたフォント一覧（起動時にディスカバリー）
    pub(crate) available_fonts: Vec<FontEntry>,
    requested_font_load: Option<FontLoadRequest>,
    pub(crate) fps: f64,
    pub(crate) displayed_fps: f64,
    fps_display_elapsed_ms: f64,
    pub(crate) source_scroll: usize, // ProblemSource でのスクロール行数
    pub(crate) how_to_use_scroll: usize, // HowToUse でのスクロール行数
    scroll_cache: Option<ScrollCache>,
    #[cfg(feature = "wasm")]
    pub(crate) should_reset_ime: bool,
    #[cfg(feature = "wasm")]
    pub(crate) should_save_custom_problems: bool, // localStorage への保存要求フラグ
}

impl App {
    /// Appの新しいインスタンスを生成する
    pub fn new(fonts: Fonts) -> Self {
        #[cfg(feature = "wasm")]
        let custom_source_label = "W";
        #[cfg(not(feature = "wasm"))]
        let custom_source_label = "F";

        #[cfg(any(feature = "gui-file", feature = "wasm"))]
        let open_file_enabled = true;
        #[cfg(not(any(feature = "gui-file", feature = "wasm")))]
        let open_file_enabled = false;

        let problem_repository = ProblemRepository::new(
            PROBLEM_FILES_NAMES,
            get_problem_content,
            custom_source_label,
            open_file_enabled,
        );

        Self {
            state: AppState::MainMenu,
            selected_main_menu_item: MainMenuItem::Start,
            selected_problem_item: 0,
            selected_settings_item: SettingsItem::FontFamily(FontTarget::Ui),
            problem_repository,
            typing_model: None,
            result_model: None,
            status_text: String::new(),
            instructions_text: String::new(),
            tui_display_mode: TuiDisplayMode::Braille,
            should_quit: false,
            should_open_file_dialog: false,
            fonts,
            display_settings: DisplaySettings::default(),
            accept_ime_input: false,
            settings_picking_font: false,
            selected_font_item: 0,
            available_fonts: Vec::new(),
            requested_font_load: None,
            fps: 0.0,
            displayed_fps: 0.0,
            fps_display_elapsed_ms: 0.0,
            source_scroll: 0,
            how_to_use_scroll: 0,
            scroll_cache: None,
            #[cfg(feature = "wasm")]
            should_reset_ime: false,
            #[cfg(feature = "wasm")]
            should_save_custom_problems: false,
        }
    }

    pub fn typing_model(&self) -> Option<&TypingModel> {
        self.typing_model.as_ref()
    }

    pub fn result_model(&self) -> Option<&ResultModel> {
        self.result_model.as_ref()
    }

    pub fn is_typing_active(&self) -> bool {
        self.state == AppState::Typing
    }

    pub fn has_pending_input_correction(&self) -> bool {
        self.typing_model.as_ref().is_some_and(|model| {
            model.status.last_wrong_keydown.is_some() || !model.status.unconfirmed.is_empty()
        })
    }

    pub fn report_visible_error(&mut self, message: impl Into<String>) {
        self.status_text = format!("Error: {}", message.into());
    }

    pub fn take_file_open_request(&mut self) -> bool {
        let should_open = self.should_open_file_dialog;
        self.should_open_file_dialog = false;
        should_open
    }

    #[cfg(feature = "wasm")]
    pub fn take_ime_reset_request(&mut self) -> bool {
        let should_reset = self.should_reset_ime;
        self.should_reset_ime = false;
        should_reset
    }

    #[cfg(feature = "wasm")]
    pub fn take_custom_problem_save_request(&mut self) -> bool {
        let should_save = self.should_save_custom_problems;
        self.should_save_custom_problems = false;
        should_save
    }

    pub fn fonts(&self) -> &Fonts {
        &self.fonts
    }

    pub fn display_settings(&self) -> DisplaySettings {
        self.display_settings
    }

    pub fn accepts_ime_input(&self) -> bool {
        self.accept_ime_input
    }

    pub(crate) fn scroll_cache(&self) -> Option<&ScrollCache> {
        self.scroll_cache.as_ref()
    }

    pub fn take_font_load_request(&mut self) -> Option<FontLoadRequest> {
        self.requested_font_load.take()
    }

    pub fn apply_font_bytes(
        &mut self,
        target: FontTarget,
        font_name: String,
        bytes: Vec<u8>,
    ) -> Result<(), FontApplyError> {
        let font = FontVec::try_from_vec(bytes).map_err(|_| FontApplyError::InvalidFontData)?;
        self.fonts.set_for_target(target, font_name, font);
        self.scroll_cache = None;
        Ok(())
    }

    fn advance_selected_display_setting(&mut self) {
        match self.selected_settings_item {
            SettingsItem::FontScale(target) => {
                let scale = self.fonts.scale_for_target(target).next();
                self.fonts.set_scale_for_target(target, scale);
                self.scroll_cache = None;
            }
            SettingsItem::AspectRatio => {
                self.display_settings.aspect_ratio = self.display_settings.aspect_ratio.next();
                self.scroll_cache = None;
            }
            SettingsItem::DisplayScale => {
                self.display_settings.scale = self.display_settings.scale.next();
                self.scroll_cache = None;
            }
            SettingsItem::ImeInput => {
                self.accept_ime_input = !self.accept_ime_input;
            }
            SettingsItem::FontFamily(_) => {}
        }
    }

    fn handle_display_setting_shortcut(&mut self, c: char) -> bool {
        let next = matches!(c, 'd' | 'D' | '+' | '=');
        let previous = matches!(c, 'a' | 'A' | '-' | '_');
        if !next && !previous {
            return false;
        }

        match self.selected_settings_item {
            SettingsItem::FontScale(target) => {
                let scale = self.fonts.scale_for_target(target);
                let scale = if next { scale.next() } else { scale.previous() };
                self.fonts.set_scale_for_target(target, scale);
                self.scroll_cache = None;
            }
            SettingsItem::AspectRatio => {
                self.display_settings.aspect_ratio = if next {
                    self.display_settings.aspect_ratio.next()
                } else {
                    self.display_settings.aspect_ratio.previous()
                };
                self.scroll_cache = None;
            }
            SettingsItem::DisplayScale => {
                self.display_settings.scale = if next {
                    self.display_settings.scale.next()
                } else {
                    self.display_settings.scale.previous()
                };
                self.scroll_cache = None;
            }
            SettingsItem::ImeInput => {
                self.accept_ime_input = !self.accept_ime_input;
            }
            SettingsItem::FontFamily(_) => return false,
        }

        true
    }

    /// 新しいタイピングセッションを開始する
    fn start_typing_session(&mut self, problem_index: usize) {
        let Some(problem_text) = self.problem_repository.problem_content_ref(problem_index) else {
            return;
        };
        let content = match parser::parse_problem(problem_text) {
            Ok(content) => content,
            Err(diagnostics) => {
                self.status_text = format!("Problem parse error: {diagnostics}");
                self.instructions_text = self.problem_selection_instructions();
                return;
            }
        };
        let typing_correctness = typing::create_typing_correctness_model(&content);

        self.typing_model = Some(TypingModel {
            content,
            status: TypingStatus {
                line: LineIndex::ZERO,
                word: WordIndex::ZERO,
                segment: SegmentIndex::ZERO,
                char_: CharIndex::ZERO,
                unconfirmed: Vec::new(),
                last_wrong_keydown: None,
            },
            user_input: Vec::new(),
            total_type_count: 0,
            total_miss_count: 0,
            first_input_time: None,
            last_input_time: None,
            typing_correctness,
            layout: Default::default(),
            scroll: Scroll {
                scroll: 0.0,
                max: 0.0,
            },
        });
        self.result_model = None;
        self.scroll_cache = None;
        self.state = AppState::Typing;
        self.on_event(AppEvent::ChangeScene);

        #[cfg(feature = "wasm")]
        {
            self.should_reset_ime = true; // タイピング開始時にIMEをリセット（フォーカスを当てる）
        }
    }

    /// 毎フレームの状態更新（スクロール計算など）
    /// 毎フレームの状態更新（スクロール計算など）
    pub fn update(&mut self, width: usize, height: usize, delta_time: f64) {
        // FPSを計算して保存
        if delta_time > 0.0 {
            let new_fps = 1000.0 / delta_time;
            self.fps = if self.fps == 0.0 {
                new_fps
            } else {
                self.fps * 0.9 + new_fps * 0.1
            };
            self.fps_display_elapsed_ms += delta_time;
            if self.displayed_fps == 0.0
                || self.fps_display_elapsed_ms >= FPS_DISPLAY_UPDATE_INTERVAL_MS
            {
                self.displayed_fps = self.fps;
                self.fps_display_elapsed_ms = 0.0;
            }
        }

        if self.state != AppState::Typing {
            return;
        }
        // delta_timeが極端に大きい場合（デバッガで停止した場合など）にスクロールが飛びすぎるのを防ぐ
        let clamped_delta_time = delta_time.min(100.0);

        if let Some(model) = self.typing_model.as_mut() {
            let base_font_size_enum = crate::ui::FontSize::WindowHeight(ui::BASE_FONT_SIZE_RATIO);
            let base_pixel_font_size =
                crate::renderer::calculate_pixel_font_size(base_font_size_enum, width, height)
                    * self.display_settings.scale.multiplier();
            let gap_width = width as f32;

            let line_idx = model.status.line.get();
            if let Some(current_line_content) = model.content.lines.get(line_idx) {
                let status = &model.status;
                let font_generation = self.fonts.generation();

                let rebuild_cache = self.scroll_cache.as_ref().is_none_or(|cache| match cache {
                    ScrollCache::Ready(ready) => {
                        ready.font_generation != font_generation
                            || (ready.font_pixel_size - base_pixel_font_size).abs() > f32::EPSILON
                            || ready.current.line != status.line
                    }
                });

                let rebuilt_current_cache = if rebuild_cache {
                    Some(build_scroll_line_cache(
                        current_line_content,
                        &self.fonts,
                        base_pixel_font_size,
                        status.line,
                    ))
                } else {
                    None
                };
                let current_cache = rebuilt_current_cache
                    .as_ref()
                    .or(match &self.scroll_cache {
                        Some(ScrollCache::Ready(ready)) => Some(&ready.current),
                        _ => None,
                    })
                    .expect("scroll cache should exist or be rebuilt");

                let line_origin = match &self.scroll_cache {
                    Some(ScrollCache::Ready(previous_cache))
                        if previous_cache.font_generation == font_generation
                            && (previous_cache.font_pixel_size - base_pixel_font_size).abs()
                                <= f32::EPSILON =>
                    {
                        line_origin_from_previous(
                            previous_cache,
                            line_idx,
                            &model.content.lines,
                            &self.fonts,
                            base_pixel_font_size,
                            gap_width,
                        )
                    }
                    _ => line_origin_from_start(
                        line_idx,
                        &model.content.lines,
                        &self.fonts,
                        base_pixel_font_size,
                        gap_width,
                    ),
                };

                let cursor_in_line = cursor_position_from_status(
                    current_cache,
                    status.word,
                    status.segment,
                    status.char_,
                );
                let cursor_world = line_origin + cursor_in_line;
                let current_total_width = current_cache.total_width;

                let scroll_position = typing_line_scroll_position(
                    line_origin,
                    current_total_width,
                    cursor_in_line,
                    width,
                );
                let mut target_scroll = scroll_position.target;
                if let Some(ScrollCache::Ready(previous_cache)) = &self.scroll_cache {
                    let previous_position = typing_line_scroll_position(
                        previous_cache.line_origin,
                        previous_cache.current.total_width,
                        previous_cache.cursor_in_line,
                        previous_cache.width,
                    );
                    let previous_target = previous_position.target;
                    if (cursor_world >= previous_cache.cursor_world
                        && target_scroll < previous_target)
                        || (cursor_world < previous_cache.cursor_world
                            && target_scroll > previous_target)
                    {
                        target_scroll = previous_target;
                    }
                }

                if model.user_input.is_empty() && model.scroll.scroll == 0.0 {
                    model.scroll.scroll = target_scroll;
                }

                let now = model.scroll.scroll;
                let diff = target_scroll - now;
                model.scroll.scroll += diff * 7.5 * (clamped_delta_time / 1000.0);
                model.scroll.max = scroll_position.max;

                if let Some(current) = rebuilt_current_cache {
                    self.scroll_cache = Some(ScrollCache::Ready(ScrollCacheState {
                        width,
                        height,
                        font_pixel_size: base_pixel_font_size,
                        font_generation,
                        line_origin,
                        cursor_in_line,
                        cursor_world,
                        current,
                    }));
                } else if let Some(ScrollCache::Ready(cache)) = self.scroll_cache.as_mut() {
                    cache.width = width;
                    cache.height = height;
                    cache.font_pixel_size = base_pixel_font_size;
                    cache.font_generation = font_generation;
                    cache.line_origin = line_origin;
                    cache.cursor_in_line = cursor_in_line;
                    cache.cursor_world = cursor_world;
                }
            }
        }
    }

    /// アプリケーションイベントを処理する
    pub fn on_event(&mut self, event: AppEvent) {
        // --- グローバルイベントの処理 ---
        if let AppEvent::CycleTuiMode = event {
            self.tui_display_mode = match self.tui_display_mode {
                TuiDisplayMode::Braille => TuiDisplayMode::AsciiArt,
                TuiDisplayMode::AsciiArt => TuiDisplayMode::SimpleText,
                TuiDisplayMode::SimpleText => TuiDisplayMode::Braille,
            };
            self.status_text = format!("Display Mode: {:?}", self.tui_display_mode);
            return;
        }

        // --- シーンごとのイベント処理 ---
        if let AppEvent::ChangeScene = event {
            match self.state {
                AppState::MainMenu => {
                    self.instructions_text = "Up/Down: Navigate | Enter: Select".to_string()
                }
                AppState::ProblemSelection => {
                    self.instructions_text = self.problem_selection_instructions()
                }
                AppState::ProblemSource => {
                    self.instructions_text = "Up/Down: Scroll | Enter/ESC: Back".to_string()
                }
                AppState::Typing => {
                    self.instructions_text = "ESC: Back to Menu | Tab: Cycle Mode".to_string()
                }
                AppState::Result => self.instructions_text = "Enter/ESC: Back to Menu".to_string(),
                AppState::Settings => {
                    self.instructions_text =
                        "Up/Down: Select | Enter: Apply | ESC: Back".to_string()
                }
                AppState::HowToUse => {
                    self.instructions_text = "Up/Down: Scroll | Enter/ESC: Back".to_string()
                }
            }
        }

        match self.state {
            AppState::MainMenu => {
                self.status_text = "Welcome to Neknaj Typing Multi-Platform".to_string();
                match event {
                    AppEvent::Up => {
                        self.selected_main_menu_item = self.selected_main_menu_item.previous();
                    }
                    AppEvent::Down => {
                        self.selected_main_menu_item = self.selected_main_menu_item.next();
                    }
                    AppEvent::Enter => match self.selected_main_menu_item {
                        MainMenuItem::Start => {
                            self.state = AppState::ProblemSelection;
                            self.on_event(AppEvent::ChangeScene);
                        }
                        MainMenuItem::HowToUse => {
                            self.how_to_use_scroll = 0;
                            self.state = AppState::HowToUse;
                            self.on_event(AppEvent::ChangeScene);
                        }
                        MainMenuItem::Settings => {
                            self.state = AppState::Settings;
                            self.on_event(AppEvent::ChangeScene);
                        }
                        MainMenuItem::Quit => {
                            #[cfg(not(feature = "wasm"))]
                            {
                                self.should_quit = true;
                            }
                        }
                    },
                    _ => {}
                }
            }
            AppState::Settings => {
                self.status_text =
                    "Assign fonts, font scales, and keyboard input handling.".to_string();
                if !self.settings_picking_font {
                    // 設定項目選択モード: Up/Down で項目を移動
                    match event {
                        AppEvent::Up => {
                            self.selected_settings_item = self.selected_settings_item.previous();
                        }
                        AppEvent::Down => {
                            self.selected_settings_item = self.selected_settings_item.next();
                        }
                        AppEvent::Enter => {
                            if self.selected_settings_item.font_target().is_some() {
                                self.settings_picking_font = true;
                                self.selected_font_item = 0;
                            } else {
                                self.advance_selected_display_setting();
                            }
                        }
                        AppEvent::Char { c, .. } => {
                            self.handle_display_setting_shortcut(c);
                        }
                        AppEvent::Escape => {
                            self.state = AppState::MainMenu;
                            self.on_event(AppEvent::ChangeScene);
                        }
                        _ => {}
                    }
                } else {
                    // フォントピッカーモード: Up/Down で available_fonts を選択、Enter で適用
                    let font_count = self.available_fonts.len();
                    match event {
                        AppEvent::Up if self.selected_font_item > 0 => {
                            self.selected_font_item -= 1;
                        }
                        AppEvent::Down
                            if font_count > 0 && self.selected_font_item < font_count - 1 =>
                        {
                            self.selected_font_item += 1;
                        }
                        AppEvent::Enter => {
                            if self.selected_font_item < font_count {
                                if let Some(target) = self.selected_settings_item.font_target() {
                                    let selected_font =
                                        &self.available_fonts[self.selected_font_item];
                                    self.requested_font_load = Some(FontLoadRequest {
                                        target,
                                        font_id: selected_font.id,
                                        font_name: selected_font.name.clone(),
                                    });
                                }
                            }
                            self.settings_picking_font = false;
                        }
                        AppEvent::Escape => {
                            self.settings_picking_font = false;
                        }
                        _ => {}
                    }
                }
            }
            AppState::ProblemSelection => {
                self.status_text = "Select a problem to type.".to_string();
                match event {
                    AppEvent::Up if self.selected_problem_item > 0 => {
                        self.selected_problem_item -= 1;
                    }
                    AppEvent::Down
                        if self.problem_count() > 0
                            && self.selected_problem_item < self.problem_count() - 1 =>
                    {
                        self.selected_problem_item += 1;
                    }
                    AppEvent::Enter => {
                        let idx = self.selected_problem_item;
                        if self.is_open_file_entry(idx) {
                            self.should_open_file_dialog = true;
                        } else {
                            self.start_typing_session(idx);
                        }
                    }
                    AppEvent::Escape => {
                        self.state = AppState::MainMenu;
                        self.on_event(AppEvent::ChangeScene);
                    }
                    AppEvent::Char { c, .. } => {
                        let idx = self.selected_problem_item;
                        match c {
                            'v' | 'V' if !self.is_open_file_entry(idx) => {
                                self.source_scroll = 0;
                                self.state = AppState::ProblemSource;
                                self.on_event(AppEvent::ChangeScene);
                            }
                            'x' | 'X' if self.is_custom_problem(idx) => {
                                self.delete_custom_problem_at(idx);
                            }
                            'u' | 'U' if self.is_custom_problem(idx) => {
                                self.move_custom_problem_up_at(idx);
                            }
                            'd' | 'D' if self.is_custom_problem(idx) => {
                                self.move_custom_problem_down_at(idx);
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
                if self.state == AppState::ProblemSelection {
                    self.instructions_text = self.problem_selection_instructions();
                }
            }
            AppState::ProblemSource => match event {
                AppEvent::Up if self.source_scroll > 0 => {
                    self.source_scroll -= 1;
                }
                AppEvent::Down => {
                    let total = self
                        .get_problem_source(self.selected_problem_item)
                        .map(|s| s.lines().count())
                        .unwrap_or(0);
                    if self.source_scroll + 1 < total {
                        self.source_scroll += 1;
                    }
                }
                AppEvent::Enter | AppEvent::Escape => {
                    self.state = AppState::ProblemSelection;
                    self.on_event(AppEvent::ChangeScene);
                }
                _ => {}
            },
            AppState::HowToUse => {
                self.status_text = "How to Use".to_string();
                match event {
                    AppEvent::Up if self.how_to_use_scroll > 0 => {
                        self.how_to_use_scroll -= 1;
                    }
                    AppEvent::Down => {
                        let total = crate::ui::HOW_TO_USE_CONTENT.len();
                        if self.how_to_use_scroll + 1 < total {
                            self.how_to_use_scroll += 1;
                        }
                    }
                    AppEvent::Enter | AppEvent::Escape => {
                        self.state = AppState::MainMenu;
                        self.on_event(AppEvent::ChangeScene);
                    }
                    _ => {}
                }
            }
            AppState::Typing => {
                self.status_text = "Start typing!".to_string();
                match event {
                    AppEvent::Char { c, timestamp } => {
                        let mut finished = false;
                        #[cfg(feature = "wasm")]
                        let mut reset_ime = false;

                        if let Some(model) = self.typing_model.as_mut() {
                            #[cfg(feature = "wasm")]
                            let old_position = (model.status.line, model.status.word);

                            finished = matches!(
                                typing::key_input(model, c, timestamp),
                                typing::TypingTransition::Finished
                            );

                            #[cfg(feature = "wasm")]
                            {
                                reset_ime = (model.status.line, model.status.word) != old_position;
                            }
                        }

                        #[cfg(feature = "wasm")]
                        {
                            if reset_ime {
                                self.should_reset_ime = true;
                            }
                        }

                        if finished {
                            if let Some(typing_model) = self.typing_model.take() {
                                self.result_model = Some(ResultModel { typing_model });
                                self.state = AppState::Result;
                                self.on_event(AppEvent::ChangeScene);
                            }
                        }
                    }
                    AppEvent::Backspace => {
                        if let Some(model) = self.typing_model.as_mut() {
                            let had_wrong_keydown = model.status.last_wrong_keydown.is_some();
                            if had_wrong_keydown {
                                let line = model.status.line.get();
                                let word = model.status.word.get();
                                let seg = model.status.segment.get();
                                let char_i = model.status.char_.get();
                                if let Some(c) = model
                                    .typing_correctness
                                    .lines
                                    .get_mut(line)
                                    .and_then(|l| l.words.get_mut(word))
                                    .and_then(|w| w.segments.get_mut(seg))
                                    .and_then(|s| s.chars.get_mut(char_i))
                                {
                                    if *c == crate::model::TypingCorrectnessChar::Incorrect {
                                        *c = crate::model::TypingCorrectnessChar::Pending;
                                    }
                                }
                            }
                            if !had_wrong_keydown {
                                model.status.unconfirmed.pop();
                            }
                            model.status.last_wrong_keydown = None;
                        }
                    }
                    AppEvent::Escape => {
                        self.state = AppState::MainMenu;
                        self.typing_model = None;
                        self.result_model = None;
                        self.on_event(AppEvent::ChangeScene);
                    }
                    _ => {}
                }
            }
            AppState::Result => {
                if let Some(result) = &self.result_model {
                    let metrics = typing::calculate_total_metrics(&result.typing_model);
                    self.status_text = format!(
                        "Complete! Speed: {:.2} kpm, Accuracy: {:.2}%",
                        metrics.speed * 60.0,
                        metrics.accuracy * 100.0
                    );
                }
                match event {
                    AppEvent::Enter | AppEvent::Escape => {
                        self.state = AppState::MainMenu;
                        self.typing_model = None;
                        self.result_model = None;
                        self.on_event(AppEvent::ChangeScene);
                    }
                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_fonts() -> Fonts {
        fn font() -> FontVec {
            FontVec::try_from_vec(include_bytes!("../fonts/YujiSyuku-Regular.ttf").to_vec())
                .expect("test font should parse")
        }

        Fonts::new(FontBundle {
            ui: font(),
            japanese: font(),
            japanese_ruby: font(),
            japanese_unconfirmed: font(),
            chinese_simplified: font(),
            chinese_simplified_ruby: font(),
            chinese_simplified_unconfirmed: font(),
            traditional_chinese: font(),
            traditional_chinese_ruby: font(),
            traditional_chinese_unconfirmed: font(),
            english: font(),
        })
    }

    #[test]
    fn enter_on_start_menu_opens_problem_selection() {
        let mut app = App::new(test_fonts());

        app.on_event(AppEvent::Enter);

        assert_eq!(app.state, AppState::ProblemSelection);
        assert!(app.typing_model.is_none());
        assert_eq!(app.instructions_text, app.problem_selection_instructions());
    }

    #[test]
    fn enter_on_builtin_problem_starts_typing_session() {
        let mut app = App::new(test_fonts());

        app.on_event(AppEvent::Enter);
        app.on_event(AppEvent::Enter);

        assert_eq!(app.state, AppState::Typing);
        assert!(app.typing_model.is_some());
        assert!(app.result_model.is_none());
        assert_eq!(app.instructions_text, "ESC: Back to Menu | Tab: Cycle Mode");
    }

    #[test]
    fn invalid_problem_selection_does_not_start_typing_session() {
        let mut app = App::new(test_fonts());

        app.on_event(AppEvent::Enter);
        app.selected_problem_item = app.problem_count();
        app.on_event(AppEvent::Enter);

        assert_eq!(app.state, AppState::ProblemSelection);
        assert!(app.typing_model.is_none());
        assert!(app.result_model.is_none());
    }

    #[test]
    fn malformed_custom_problem_reports_parse_error() {
        let mut app = App::new(test_fonts());

        app.add_custom_problem(
            "Broken".to_string(),
            "#title Broken\n[未完了/みかんりょう".to_string(),
            0,
        );
        app.on_event(AppEvent::Enter);

        assert_eq!(app.state, AppState::ProblemSelection);
        assert!(app.typing_model.is_none());
        assert!(app.status_text.contains("Problem parse error"));
        assert!(app.status_text.contains("missing closing"));
    }

    #[test]
    fn builtin_problem_files_parse_successfully() {
        for (index, name) in PROBLEM_FILES_NAMES.iter().enumerate() {
            parser::parse_problem(get_problem_content(index))
                .unwrap_or_else(|diagnostics| panic!("{name} should parse: {diagnostics}"));
        }
    }

    #[test]
    fn ime_input_setting_defaults_disabled_and_toggles() {
        let mut app = App::new(test_fonts());
        app.state = AppState::Settings;

        assert!(!app.accepts_ime_input());

        app.selected_settings_item = SettingsItem::ImeInput;
        app.on_event(AppEvent::Enter);
        assert!(app.accepts_ime_input());

        app.on_event(AppEvent::Char {
            c: 'a',
            timestamp: 1.0,
        });
        assert!(!app.accepts_ime_input());
    }

    fn assert_settings_font_picker_requests_target(target: FontTarget) {
        let mut app = App::new(test_fonts());
        app.state = AppState::Settings;
        app.available_fonts = vec![FontEntry {
            id: FontAssetId(42),
            name: "Kalam-Regular".to_string(),
            source: FontSource::Bundled,
        }];
        app.selected_settings_item = SettingsItem::FontFamily(target);

        app.on_event(AppEvent::Enter);
        assert!(app.settings_picking_font);
        app.on_event(AppEvent::Enter);

        assert!(!app.settings_picking_font);
        assert_eq!(
            app.take_font_load_request(),
            Some(FontLoadRequest {
                target,
                font_id: FontAssetId(42),
                font_name: "Kalam-Regular".to_string(),
            })
        );
    }

    #[test]
    fn settings_font_picker_requests_script_ruby_and_unconfirmed_font_targets() {
        for target in [
            FontTarget::Unconfirmed(Script::Japanese),
            FontTarget::Script(Script::ChineseSimplified),
            FontTarget::Ruby(Script::ChineseSimplified),
            FontTarget::Unconfirmed(Script::ChineseSimplified),
            FontTarget::Script(Script::TraditionalChinese),
            FontTarget::Ruby(Script::TraditionalChinese),
            FontTarget::Unconfirmed(Script::TraditionalChinese),
        ] {
            assert_settings_font_picker_requests_target(target);
        }
    }

    fn assert_applying_font_replaces_only_target(target: FontTarget, related: &[FontTarget]) {
        let mut app = App::new(test_fonts());
        let related_before = related
            .iter()
            .map(|target| (*target, app.fonts().name_for_target(*target).to_string()))
            .collect::<Vec<_>>();

        app.apply_font_bytes(
            target,
            "Kalam-Regular".to_string(),
            include_bytes!("../fonts/Kalam-Regular.ttf").to_vec(),
        )
        .expect("Kalam font should apply to selected font slot");

        assert_eq!(
            app.fonts().name_for_target(target),
            "Kalam-Regular",
            "{target:?} should receive selected font"
        );
        for (related_target, before) in related_before {
            if related_target != target {
                assert_eq!(
                    app.fonts().name_for_target(related_target),
                    before,
                    "{target:?} should not replace {related_target:?}"
                );
            }
        }
    }

    #[test]
    fn applying_font_to_chinese_targets_uses_independent_slots() {
        for script in [Script::ChineseSimplified, Script::TraditionalChinese] {
            let related = [
                FontTarget::Script(script),
                FontTarget::Ruby(script),
                FontTarget::Unconfirmed(script),
            ];
            for target in related {
                assert_applying_font_replaces_only_target(target, &related);
            }
        }
    }

    #[test]
    fn applying_unconfirmed_font_does_not_replace_base_font() {
        assert_applying_font_replaces_only_target(
            FontTarget::Unconfirmed(Script::Japanese),
            &[
                FontTarget::Script(Script::Japanese),
                FontTarget::Ruby(Script::Japanese),
                FontTarget::Unconfirmed(Script::Japanese),
            ],
        );
    }

    #[test]
    fn backspace_after_wrong_key_preserves_unconfirmed_prefix() {
        let mut app = App::new(test_fonts());
        app.add_custom_problem(
            "Backspace".to_string(),
            "#title Test\n[\u{8272}/\u{3057}]\u{3042}".to_string(),
            0,
        );
        app.on_event(AppEvent::Enter);

        app.on_event(AppEvent::Char {
            c: 's',
            timestamp: 1.0,
        });
        app.on_event(AppEvent::Char {
            c: 'x',
            timestamp: 2.0,
        });
        app.on_event(AppEvent::Backspace);

        let model = app
            .typing_model
            .as_ref()
            .expect("typing model should remain active");
        assert_eq!(model.status.unconfirmed, ['s']);
        assert_eq!(model.status.last_wrong_keydown, None);
        assert_eq!(
            model.typing_correctness.lines[0].words[0].segments[0].chars[0],
            crate::model::TypingCorrectnessChar::Pending
        );

        app.on_event(AppEvent::Char {
            c: 'i',
            timestamp: 3.0,
        });

        let model = app
            .typing_model
            .as_ref()
            .expect("typing model should remain active after completing first segment");
        assert!(model.status.unconfirmed.is_empty());
        assert_eq!(model.status.word, WordIndex::new(1));
        assert_eq!(
            model.typing_correctness.lines[0].words[0].segments[0].chars[0],
            crate::model::TypingCorrectnessChar::Correct
        );
    }

    #[test]
    fn snapshot_exposes_immutable_view_state() {
        let mut app = App::new(test_fonts());

        app.on_event(AppEvent::Start);
        app.on_event(AppEvent::Down);

        let snapshot = app.snapshot();
        assert_eq!(snapshot.state, AppState::MainMenu);
        assert_eq!(snapshot.selected_main_menu_item, MainMenuItem::HowToUse);
        assert_eq!(
            snapshot.status_text,
            "Welcome to Neknaj Typing Multi-Platform"
        );
        assert!(!snapshot.should_quit);
        assert!(!snapshot.should_open_file_dialog);
    }

    #[test]
    fn snapshot_fps_is_throttled_for_stable_render_lists() {
        let mut app = App::new(test_fonts());
        let fps_text = |app: &App| {
            crate::ui::build_ui(app, app.fonts(), 800, 500)
                .into_iter()
                .find_map(|item| match item {
                    crate::ui::Renderable::Text { text, .. } if text.starts_with("FPS:") => {
                        Some(text)
                    }
                    _ => None,
                })
                .expect("FPS renderable should exist")
        };

        app.update(800, 500, 16.0);
        let first_displayed_fps = app.snapshot().fps;
        let first_fps_text = fps_text(&app);
        assert!(first_displayed_fps > 0.0);

        app.update(800, 500, 10.0);
        assert_eq!(app.snapshot().fps, first_displayed_fps);
        assert_eq!(fps_text(&app), first_fps_text);
        assert!(app.fps > first_displayed_fps);

        for _ in 0..25 {
            app.update(800, 500, 10.0);
        }
        assert!(
            app.snapshot().fps > first_displayed_fps,
            "displayed FPS should update after the throttle interval"
        );
        assert_ne!(fps_text(&app), first_fps_text);
    }

    #[test]
    #[cfg(not(any(feature = "gui-file", feature = "wasm")))]
    fn app_does_not_advertise_open_file_without_backend_support() {
        let app = App::new(test_fonts());

        assert!((0..app.problem_count()).all(|index| !app.is_open_file_entry(index)));
    }
}
