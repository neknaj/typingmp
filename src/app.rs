// ./src/app.rs

extern crate alloc;

mod problems;
mod scroll;
mod view;

use crate::app::scroll::{
    build_scroll_line_cache, cursor_position_from_status, line_origin_from_previous,
    line_origin_from_start, ScrollCacheState,
};
#[cfg(not(any(target_arch = "wasm32", feature = "uefi")))]
use crate::io::FontEntry;
use crate::io::{FontAssetId, ProblemRepository};
use crate::model::{
    CharIndex, LineIndex, ResultModel, Scroll, SegmentIndex, TypingModel, TypingStatus, WordIndex,
};
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

pub(crate) use scroll::ScrollCache;
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

/// どのスクリプト種別のフォントを選択しているか
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Script {
    Japanese,
    TraditionalChinese,
    SimplifiedChinese,
}

impl Script {
    pub fn label(self) -> &'static str {
        match self {
            Script::Japanese => "Japanese",
            Script::TraditionalChinese => "Traditional Chinese",
            Script::SimplifiedChinese => "Simplified Chinese",
        }
    }
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
                #[cfg(target_arch = "wasm32")]
                {
                    Self::Settings
                }
                #[cfg(not(target_arch = "wasm32"))]
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
    Japanese,
    TraditionalChinese,
    SimplifiedChinese,
}

impl SettingsItem {
    pub const fn index(self) -> usize {
        match self {
            Self::Japanese => 0,
            Self::TraditionalChinese => 1,
            Self::SimplifiedChinese => 2,
        }
    }

    pub const fn script(self) -> Script {
        match self {
            Self::Japanese => Script::Japanese,
            Self::TraditionalChinese => Script::TraditionalChinese,
            Self::SimplifiedChinese => Script::SimplifiedChinese,
        }
    }

    pub fn previous(self) -> Self {
        match self {
            Self::Japanese => Self::Japanese,
            Self::TraditionalChinese => Self::Japanese,
            Self::SimplifiedChinese => Self::TraditionalChinese,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Japanese => Self::TraditionalChinese,
            Self::TraditionalChinese => Self::SimplifiedChinese,
            Self::SimplifiedChinese => Self::SimplifiedChinese,
        }
    }
}

/// ロードされたフォントデータ（スクリプト別）
pub struct Fonts {
    pub japanese: FontVec,
    pub traditional_chinese: Option<FontVec>,
    pub simplified_chinese: Option<FontVec>,
}

impl Fonts {
    /// スクリプトに対応するフォントを返す（未設定時は japanese で代替）
    pub fn get_for_script(&self, script: Script) -> &FontVec {
        match script {
            Script::Japanese => &self.japanese,
            Script::TraditionalChinese => {
                self.traditional_chinese.as_ref().unwrap_or(&self.japanese)
            }
            Script::SimplifiedChinese => self.simplified_chinese.as_ref().unwrap_or(&self.japanese),
        }
    }

    /// メインフォント（日本語フォント）を返す
    pub fn primary(&self) -> &FontVec {
        &self.japanese
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontLoadRequest {
    pub script: Script,
    pub font_id: FontAssetId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontApplyError {
    InvalidFontData,
}

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
    /// ファイルダイアログを開く要求フラグ（gui/wasm のみ）
    pub(crate) should_open_file_dialog: bool,
    pub(crate) fonts: Fonts,
    /// Settings画面で選択中のスクリプト
    pub(crate) settings_script: Script,
    /// フォントピッカーを開いているか
    pub(crate) settings_picking_font: bool,
    /// フォントピッカー内の選択インデックス
    pub(crate) selected_font_item: usize,
    /// 発見されたフォント一覧（起動時にディスカバリー）
    #[cfg(not(any(target_arch = "wasm32", feature = "uefi")))]
    pub(crate) available_fonts: Vec<FontEntry>,
    requested_font_load: Option<FontLoadRequest>,
    pub(crate) fps: f64,
    pub(crate) source_scroll: usize, // ProblemSource でのスクロール行数
    pub(crate) how_to_use_scroll: usize, // HowToUse でのスクロール行数
    scroll_cache: Option<ScrollCache>,
    #[cfg(target_arch = "wasm32")]
    pub(crate) should_reset_ime: bool,
    #[cfg(target_arch = "wasm32")]
    pub(crate) should_save_custom_problems: bool, // localStorage への保存要求フラグ
}

impl App {
    /// Appの新しいインスタンスを生成する
    pub fn new(fonts: Fonts) -> Self {
        #[cfg(target_arch = "wasm32")]
        let custom_source_label = "W";
        #[cfg(not(target_arch = "wasm32"))]
        let custom_source_label = "F";

        #[cfg(any(feature = "gui", target_arch = "wasm32"))]
        let open_file_enabled = true;
        #[cfg(not(any(feature = "gui", target_arch = "wasm32")))]
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
            selected_settings_item: SettingsItem::Japanese,
            problem_repository,
            typing_model: None,
            result_model: None,
            status_text: String::new(),
            instructions_text: String::new(),
            tui_display_mode: TuiDisplayMode::Braille,
            should_quit: false,
            should_open_file_dialog: false,
            fonts,
            settings_script: Script::Japanese,
            settings_picking_font: false,
            selected_font_item: 0,
            #[cfg(not(any(target_arch = "wasm32", feature = "uefi")))]
            available_fonts: Vec::new(),
            requested_font_load: None,
            fps: 0.0,
            source_scroll: 0,
            how_to_use_scroll: 0,
            scroll_cache: None,
            #[cfg(target_arch = "wasm32")]
            should_reset_ime: false,
            #[cfg(target_arch = "wasm32")]
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

    #[cfg(target_arch = "wasm32")]
    pub fn take_ime_reset_request(&mut self) -> bool {
        let should_reset = self.should_reset_ime;
        self.should_reset_ime = false;
        should_reset
    }

    #[cfg(target_arch = "wasm32")]
    pub fn take_custom_problem_save_request(&mut self) -> bool {
        let should_save = self.should_save_custom_problems;
        self.should_save_custom_problems = false;
        should_save
    }

    /// 現在の設定言語に合わせたメインフォントへの参照を取得する
    pub fn get_current_font(&self) -> &FontVec {
        self.fonts.get_for_script(self.settings_script)
    }

    pub(crate) fn scroll_cache(&self) -> Option<&ScrollCache> {
        self.scroll_cache.as_ref()
    }

    pub fn take_font_load_request(&mut self) -> Option<FontLoadRequest> {
        self.requested_font_load.take()
    }

    pub fn apply_font_bytes(
        &mut self,
        script: Script,
        bytes: Vec<u8>,
    ) -> Result<(), FontApplyError> {
        let font = FontVec::try_from_vec(bytes).map_err(|_| FontApplyError::InvalidFontData)?;
        match script {
            Script::Japanese => self.fonts.japanese = font,
            Script::TraditionalChinese => self.fonts.traditional_chinese = Some(font),
            Script::SimplifiedChinese => self.fonts.simplified_chinese = Some(font),
        }
        self.scroll_cache = None;
        Ok(())
    }

    /// 新しいタイピングセッションを開始する
    fn start_typing_session(&mut self, problem_index: usize) {
        let Some(problem_text) = self.problem_repository.problem_content(problem_index) else {
            return;
        };
        let content = match parser::parse_problem(problem_text.as_ref()) {
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

        #[cfg(target_arch = "wasm32")]
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
        }

        if self.state != AppState::Typing {
            return;
        }
        // delta_timeが極端に大きい場合（デバッガで停止した場合など）にスクロールが飛びすぎるのを防ぐ
        let clamped_delta_time = delta_time.min(100.0);

        if let Some(model) = self.typing_model.as_mut() {
            let font = self.fonts.get_for_script(self.settings_script);
            let base_font_size_enum = crate::ui::FontSize::WindowHeight(ui::BASE_FONT_SIZE_RATIO);
            let base_pixel_font_size =
                crate::renderer::calculate_pixel_font_size(base_font_size_enum, width, height);
            let gap_width = width as f32;

            let line_idx = model.status.line.get();
            if let Some(current_line_content) = model.content.lines.get(line_idx) {
                let status = &model.status;

                let rebuild_cache = self.scroll_cache.as_ref().is_none_or(|cache| match cache {
                    ScrollCache::Ready(ready) => {
                        ready.width != width
                            || ready.height != height
                            || (ready.font_pixel_size - base_pixel_font_size).abs() > f32::EPSILON
                            || ready.current.line != status.line
                    }
                });

                let current_cache = if rebuild_cache {
                    build_scroll_line_cache(
                        current_line_content,
                        font,
                        base_pixel_font_size,
                        status.line,
                    )
                } else {
                    match &self.scroll_cache {
                        Some(ScrollCache::Ready(ready)) => ready.current.clone(),
                        _ => build_scroll_line_cache(
                            current_line_content,
                            font,
                            base_pixel_font_size,
                            status.line,
                        ),
                    }
                };

                let line_origin = match &self.scroll_cache {
                    Some(ScrollCache::Ready(previous_cache))
                        if (previous_cache.font_pixel_size - base_pixel_font_size).abs()
                            <= f32::EPSILON =>
                    {
                        line_origin_from_previous(
                            previous_cache,
                            line_idx,
                            &model.content.lines,
                            font,
                            base_pixel_font_size,
                            gap_width,
                        )
                    }
                    _ => line_origin_from_start(
                        line_idx,
                        &model.content.lines,
                        font,
                        base_pixel_font_size,
                        gap_width,
                    ),
                };

                let cursor_in_line = cursor_position_from_status(
                    &current_cache,
                    status.word,
                    status.segment,
                    status.char_,
                );
                let cursor_world = line_origin + cursor_in_line;

                let mut target_scroll = f64::from(cursor_world) - (f64::from(gap_width) * 0.5);
                if let Some(ScrollCache::Ready(previous_cache)) = &self.scroll_cache {
                    let previous_target = previous_cache.cursor_world as f64
                        - (previous_cache.gap_width as f64 * 0.5);
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

                self.scroll_cache = Some(ScrollCache::Ready(ScrollCacheState {
                    width,
                    height,
                    font_pixel_size: base_pixel_font_size,
                    gap_width,
                    line_origin,
                    cursor_in_line,
                    cursor_world,
                    current: current_cache,
                }));
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
                            #[cfg(not(target_arch = "wasm32"))]
                            {
                                self.should_quit = true;
                            }
                        }
                    },
                    _ => {}
                }
            }
            AppState::Settings => {
                self.status_text = "Select a font for each script.".to_string();
                if !self.settings_picking_font {
                    // スクリプト選択モード: Up/Down で 3 スクリプトをサイクル
                    match event {
                        AppEvent::Up => {
                            self.selected_settings_item = self.selected_settings_item.previous();
                            self.settings_script = self.selected_settings_item.script();
                        }
                        AppEvent::Down => {
                            self.selected_settings_item = self.selected_settings_item.next();
                            self.settings_script = self.selected_settings_item.script();
                        }
                        AppEvent::Enter => {
                            // 選択スクリプトのフォントピッカーを開く
                            self.settings_picking_font = true;
                            self.selected_font_item = 0;
                        }
                        AppEvent::Escape => {
                            self.state = AppState::MainMenu;
                            self.on_event(AppEvent::ChangeScene);
                        }
                        _ => {}
                    }
                } else {
                    // フォントピッカーモード: Up/Down で available_fonts を選択、Enter で適用
                    #[cfg(not(any(target_arch = "wasm32", feature = "uefi")))]
                    {
                        let font_count = self.available_fonts.len();
                        match event {
                            AppEvent::Up => {
                                if self.selected_font_item > 0 {
                                    self.selected_font_item -= 1;
                                }
                            }
                            AppEvent::Down => {
                                if font_count > 0 && self.selected_font_item < font_count - 1 {
                                    self.selected_font_item += 1;
                                }
                            }
                            AppEvent::Enter => {
                                if self.selected_font_item < font_count {
                                    let font_id = self.available_fonts[self.selected_font_item].id;
                                    let script = self.settings_script;
                                    self.requested_font_load =
                                        Some(FontLoadRequest { script, font_id });
                                }
                                self.settings_picking_font = false;
                            }
                            AppEvent::Escape => {
                                self.settings_picking_font = false;
                            }
                            _ => {}
                        }
                    }
                    // WASM / UEFI ではフォントピッカー操作なし
                    #[cfg(any(target_arch = "wasm32", feature = "uefi"))]
                    {
                        match event {
                            AppEvent::Escape | AppEvent::Enter => {
                                self.settings_picking_font = false;
                            }
                            _ => {}
                        }
                    }
                }
            }
            AppState::ProblemSelection => {
                self.status_text = "Select a problem to type.".to_string();
                match event {
                    AppEvent::Up => {
                        if self.selected_problem_item > 0 {
                            self.selected_problem_item -= 1;
                        }
                    }
                    AppEvent::Down => {
                        if self.problem_count() > 0
                            && self.selected_problem_item < self.problem_count() - 1
                        {
                            self.selected_problem_item += 1;
                        }
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
                            'v' | 'V' => {
                                if !self.is_open_file_entry(idx) {
                                    self.source_scroll = 0;
                                    self.state = AppState::ProblemSource;
                                    self.on_event(AppEvent::ChangeScene);
                                }
                            }
                            'x' | 'X' => {
                                if self.is_custom_problem(idx) {
                                    self.delete_custom_problem_at(idx);
                                }
                            }
                            'u' | 'U' => {
                                if self.is_custom_problem(idx) {
                                    self.move_custom_problem_up_at(idx);
                                }
                            }
                            'd' | 'D' => {
                                if self.is_custom_problem(idx) {
                                    self.move_custom_problem_down_at(idx);
                                }
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
                AppEvent::Up => {
                    if self.source_scroll > 0 {
                        self.source_scroll -= 1;
                    }
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
                    AppEvent::Up => {
                        if self.how_to_use_scroll > 0 {
                            self.how_to_use_scroll -= 1;
                        }
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
                        #[cfg(target_arch = "wasm32")]
                        let mut reset_ime = false;

                        if let Some(model) = self.typing_model.as_mut() {
                            #[cfg(target_arch = "wasm32")]
                            let old_position = (model.status.line, model.status.word);

                            finished = matches!(
                                typing::key_input(model, c, timestamp),
                                typing::TypingTransition::Finished
                            );

                            #[cfg(target_arch = "wasm32")]
                            {
                                reset_ime = (model.status.line, model.status.word) != old_position;
                            }
                        }

                        #[cfg(target_arch = "wasm32")]
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
                            if model.status.last_wrong_keydown.is_some() {
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
                            model.status.unconfirmed.pop();
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
        let japanese =
            FontVec::try_from_vec(include_bytes!("../fonts/YujiSyuku-Regular.ttf").to_vec())
                .expect("test font should parse");
        Fonts {
            japanese,
            traditional_chinese: None,
            simplified_chinese: None,
        }
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
}
