// ./src/app.rs

// uefi featureが有効な場合、標準のallocクレートをインポート
#[cfg(feature = "uefi")]
extern crate alloc;

// uefi と std で使用する String と format! を切り替える
#[cfg(feature = "uefi")]
use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};
#[cfg(not(feature = "uefi"))]
use std::{
    string::{String, ToString},
    vec::Vec,
};

use crate::model::{Model, ResultModel, Scroll, Segment, TypingModel, TypingStatus};

/// ユーザーがアップロード/オープンしたカスタム問題ファイル
pub struct CustomProblem {
    pub name: String,
    pub content: String,
    pub timestamp_ms: u64,
}

// セグメントの base テキストを返す（Anno は inner を連結）
fn seg_base_text_owned(seg: &Segment) -> String {
    match seg {
        Segment::Plain { text } => text.clone(),
        Segment::Annotated { base, .. } => base.clone(),
        Segment::Anno { inner, .. } => inner.iter().map(|s| seg_base_text_owned(s)).collect(),
    }
}

// セグメントの reading テキストを返す（Anno は inner を連結）
fn seg_reading_text_owned(seg: &Segment) -> String {
    match seg {
        Segment::Plain { text } => text.clone(),
        Segment::Annotated { reading, .. } => reading.clone(),
        Segment::Anno { inner, .. } => inner.iter().map(|s| seg_reading_text_owned(s)).collect(),
    }
}
use crate::parser;
use crate::typing;
use crate::ui; // typing_rendererの代わりにuiをインポート
use crate::renderer::gui_renderer;
use ab_glyph::FontRef;

// ビルドスクリプトによってOUT_DIRに生成されたファイルを取り込む
include!(concat!(env!("OUT_DIR"), "/problem_files.rs"));

/// アプリケーションの現在の状態（シーン）を定義するenum
#[derive(PartialEq, Clone, Copy)]
pub enum AppState {
    MainMenu,
    ProblemSelection,
    Typing,
    Result,
    Settings, // 設定画面の状態を追加
}

/// TUIの描画モードを定義するenum
#[derive(PartialEq, Clone, Copy, Debug)]
pub enum TuiDisplayMode {
    AsciiArt,
    SimpleText,
    Braille,
}

/// 利用可能なフォントを定義するenum
#[derive(PartialEq, Clone, Copy, Debug)]
pub enum FontChoice {
    YujiSyuku,
    NotoSerifJP,
}

/// ロードされたフォントデータを保持する構造体
pub struct Fonts<'a> {
    pub yuji_syuku: FontRef<'a>,
    pub noto_serif: FontRef<'a>,
}

#[cfg(target_arch = "wasm32")]
const MENU_ITEM_COUNT: usize = 2; // Quitなし

#[cfg(not(target_arch = "wasm32"))]
const MENU_ITEM_COUNT: usize = 3;

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

/// アプリケーション全体で共有される状態を保持する構造体
pub struct App<'a> {
    pub state: AppState,
    pub selected_main_menu_item: usize,
    pub selected_problem_item: usize,
    pub selected_settings_item: usize,
    pub custom_problems: Vec<CustomProblem>,
    pub typing_model: Option<TypingModel>,
    pub result_model: Option<ResultModel>,
    pub status_text: String,
    pub instructions_text: String,
    pub tui_display_mode: TuiDisplayMode,
    pub should_quit: bool,
    /// ファイルダイアログを開く要求フラグ（gui/wasm のみ）
    pub should_open_file_dialog: bool,
    // フォント管理用のフィールド
    pub fonts: Fonts<'a>,
    pub font_choice: FontChoice,
    pub fps: f64, // FPSを保持するフィールドを追加
    #[cfg(target_arch = "wasm32")] // wasmでのみ利用
    pub should_reset_ime: bool, // IMEリセット要求フラグ
}

impl<'a> App<'a> {
    /// Appの新しいインスタンスを生成する
    pub fn new(fonts: Fonts<'a>) -> Self {
        #[cfg(feature = "uefi")]
        uefi::println!("APP: START");
        Self {
            state: AppState::MainMenu,
            selected_main_menu_item: 0,
            selected_problem_item: 0,
            selected_settings_item: 0,
            custom_problems: Vec::new(),
            typing_model: None,
            result_model: None,
            status_text: String::new(),
            instructions_text: String::new(),
            tui_display_mode: TuiDisplayMode::Braille,
            should_quit: false,
            should_open_file_dialog: false,
            fonts,
            font_choice: FontChoice::YujiSyuku, // デフォルトフォント
            fps: 0.0, // FPSを初期化
            #[cfg(target_arch = "wasm32")]
            should_reset_ime: false, // 初期値はfalse
        }
    }

    /// 組み込み問題 + カスタム問題 + (gui/wasm では「Open File」エントリ1件) の合計数
    pub fn problem_count(&self) -> usize {
        let base = PROBLEM_FILES_NAMES.len() + self.custom_problems.len();
        #[cfg(any(feature = "gui", target_arch = "wasm32"))]
        { base + 1 }
        #[cfg(not(any(feature = "gui", target_arch = "wasm32")))]
        { base }
    }

    /// インデックスに対応する表示名を返す
    pub fn problem_name_at(&self, idx: usize) -> &str {
        let builtin_count = PROBLEM_FILES_NAMES.len();
        if idx < builtin_count {
            PROBLEM_FILES_NAMES[idx]
        } else if idx < builtin_count + self.custom_problems.len() {
            &self.custom_problems[idx - builtin_count].name
        } else {
            "[ Open File... ]"
        }
    }

    /// そのインデックスが「Open File」エントリかどうか
    pub fn is_open_file_entry(&self, idx: usize) -> bool {
        #[cfg(any(feature = "gui", target_arch = "wasm32"))]
        { idx == PROBLEM_FILES_NAMES.len() + self.custom_problems.len() }
        #[cfg(not(any(feature = "gui", target_arch = "wasm32")))]
        { let _ = idx; false }
    }

    /// カスタム問題を追加し、そのインデックスを選択状態にする
    pub fn add_custom_problem(&mut self, name: String, content: String, timestamp_ms: u64) {
        self.custom_problems.push(CustomProblem { name, content, timestamp_ms });
        // 追加された問題のインデックスを選択
        self.selected_problem_item = PROBLEM_FILES_NAMES.len() + self.custom_problems.len() - 1;
        if self.state != AppState::ProblemSelection {
            self.state = AppState::ProblemSelection;
            self.on_event(AppEvent::ChangeScene);
        }
    }

    /// 現在選択されているフォントへの参照を取得する
    pub fn get_current_font(&self) -> &FontRef<'a> {
        match self.font_choice {
            FontChoice::YujiSyuku => &self.fonts.yuji_syuku,
            FontChoice::NotoSerifJP => &self.fonts.noto_serif,
        }
    }

    /// 新しいタイピングセッションを開始する
    fn start_typing_session(&mut self, problem_index: usize) {
        // 選択されたインデックスに基づいて問題文を読み込む
        let builtin_count = PROBLEM_FILES_NAMES.len();
        let problem_text_owned: String;
        let problem_text: &str = if problem_index < builtin_count {
            get_problem_content(problem_index)
        } else {
            problem_text_owned = self.custom_problems[problem_index - builtin_count].content.clone();
            &problem_text_owned
        };
        let content = parser::parse_problem(problem_text);
        let typing_correctness = typing::create_typing_correctness_model(&content);

        self.typing_model = Some(TypingModel {
            content,
            status: TypingStatus {
                line: 0,
                word: 0,
                segment: 0,
                char_: 0,
                unconfirmed: Vec::new(),
                last_wrong_keydown: None,
            },
            user_input: Vec::new(),
            typing_correctness,
            layout: Default::default(),
            scroll: Scroll {
                scroll: 0.0,
                max: 0.0,
            },
        });
        self.result_model = None;
        self.state = AppState::Typing;
        self.on_event(AppEvent::ChangeScene);

        #[cfg(target_arch = "wasm32")]
        {
            self.should_reset_ime = true; // タイピング開始時にIMEをリセット（フォーカスを当てる）
        }
    }

    /// 毎フレームの状態更新（スクロール計算など）
    pub fn update(&mut self, width: usize, height: usize, delta_time: f64) {
        // FPSを計算して保存
        if delta_time > 0.0 {
            let new_fps = 1000.0 / delta_time;
            self.fps = if self.fps == 0.0 { new_fps } else { self.fps * 0.9 + new_fps * 0.1 };
        }

        if self.state != AppState::Typing {
            return;
        }
        // delta_timeが極端に大きい場合（デバッガで停止した場合など）にスクロールが飛びすぎるのを防ぐ
        // 100ms (0.1秒) を上限とする
        let clamped_delta_time = delta_time.min(100.0);

        if let Some(model) = self.typing_model.as_mut() {

            // ブロック内で不変参照を取得することで借用ルール違反を回避
            let font = match self.font_choice {
                FontChoice::YujiSyuku => &self.fonts.yuji_syuku,
                FontChoice::NotoSerifJP => &self.fonts.noto_serif,
            };

            let base_font_size_enum = crate::ui::FontSize::WindowHeight(ui::BASE_FONT_SIZE_RATIO);
            let base_pixel_font_size = crate::renderer::calculate_pixel_font_size(base_font_size_enum, width, height);

            if let Some(current_line_content) = model.content.lines.get(model.status.line as usize) {
                // 1. Calculate the total width of the current line's BASE text for centering
                let total_width: f32 = current_line_content.words.iter().flat_map(|w| &w.segments).map(|seg| {
                    let text = seg_base_text_owned(seg);
                    gui_renderer::measure_text(font, &text, base_pixel_font_size).0 as f32
                }).sum();

                // セッション開始時の最初のフレームで、スクロールの初期値を設定する
                // これにより、テキストが画面の右側からスライドインする演出が生まれる
                // user_inputが空かつscrollが0.0の場合をセッション開始直後と判断
                if model.user_input.is_empty() && model.scroll.scroll == 0.0 {
                    // テキストブロックの左端が画面の右端に来るようにスクロール値を計算
                    model.scroll.scroll = (-(width as f32 / 2.0) - (total_width / 2.0)) as f64;
                }

                // 2. Calculate the width up to the cursor
                let mut cursor_x_offset = 0.0;
                // Add width of completed words (based on BASE text)
                for i in 0..model.status.word as usize {
                    if let Some(word) = current_line_content.words.get(i) {
                        for seg in &word.segments {
                            let text = seg_base_text_owned(seg);
                            cursor_x_offset += gui_renderer::measure_text(font, &text, base_pixel_font_size).0 as f32;
                        }
                    }
                }
                
                // Add width of completed segments in the current word
                if let Some(current_word) = current_line_content.words.get(model.status.word as usize) {
                    for i in 0..model.status.segment as usize {
                        if let Some(seg) = current_word.segments.get(i) {
                            let text = seg_base_text_owned(seg);
                            cursor_x_offset += gui_renderer::measure_text(font, &text, base_pixel_font_size).0 as f32;
                        }
                    }

                    // For the current segment, add the width of the typed READING text
                    if let Some(seg) = current_word.segments.get(model.status.segment as usize) {
                        let reading_text = seg_reading_text_owned(seg);
                        // Get the substring of the reading text that has been typed so far.
                        let typed_reading_part = reading_text.chars().take(model.status.char_ as usize).collect::<String>();
                        // Measure the actual pixel width of the typed part.
                        let typed_reading_width = gui_renderer::measure_text(font, &typed_reading_part, base_pixel_font_size).0 as f32;
                        // Add this width to the cursor offset.
                        cursor_x_offset += typed_reading_width;
                    }
                }

                // 3. Calculate target scroll position so the cursor is centered
                let target_scroll = cursor_x_offset - total_width / 2.0;

                // 4. Smoothly update the scroll value using delta_time for frame-rate independence
                let now = model.scroll.scroll;
                let diff = target_scroll as f64 - now;
                let scroll_speed_factor = 5.0; // Adjust this value for faster/slower scrolling
                model.scroll.scroll += diff * scroll_speed_factor * (clamped_delta_time / 1000.0);
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
                AppState::MainMenu => self.instructions_text = "Up/Down: Navigate | Enter: Select".to_string(),
                AppState::ProblemSelection => self.instructions_text = "Up/Down: Select | Enter: Start | ESC: Back".to_string(),
                AppState::Typing => self.instructions_text = "ESC: Back to Menu | Tab: Cycle Mode".to_string(),
                AppState::Result => self.instructions_text = "Enter/ESC: Back to Menu".to_string(),
                AppState::Settings => self.instructions_text = "Up/Down: Select | Enter: Apply | ESC: Back".to_string(),
            }
        }

        match self.state {
            AppState::MainMenu => {
                self.status_text = "Welcome to Neknaj Typing Multi-Platform".to_string();
                match event {
                    AppEvent::Up => if self.selected_main_menu_item > 0 { self.selected_main_menu_item -= 1; },
                    AppEvent::Down => if self.selected_main_menu_item < MENU_ITEM_COUNT - 1 { self.selected_main_menu_item += 1; },
                    AppEvent::Enter => match self.selected_main_menu_item {
                        0 => {
                            self.state = AppState::ProblemSelection;
                            self.on_event(AppEvent::ChangeScene);
                        }
                        1 => {
                            self.state = AppState::Settings;
                            self.on_event(AppEvent::ChangeScene);
                        }
                        2 => {
                            #[cfg(not(target_arch = "wasm32"))]
                            { self.should_quit = true; }
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
            AppState::Settings => {
                self.status_text = "Select a font.".to_string();
                match event {
                    AppEvent::Up => if self.selected_settings_item > 0 { self.selected_settings_item -= 1; },
                    AppEvent::Down => if self.selected_settings_item < 1 { self.selected_settings_item += 1; },
                    AppEvent::Enter => {
                        self.font_choice = match self.selected_settings_item {
                            0 => FontChoice::YujiSyuku,
                            _ => FontChoice::NotoSerifJP,
                        };
                        self.state = AppState::MainMenu;
                        self.on_event(AppEvent::ChangeScene);
                    }
                    AppEvent::Escape => {
                        self.state = AppState::MainMenu;
                        self.on_event(AppEvent::ChangeScene);
                    }
                    _ => {}
                }
            }
            AppState::ProblemSelection => {
                self.status_text = "Select a problem to type.".to_string();
                match event {
                    AppEvent::Up => if self.selected_problem_item > 0 { self.selected_problem_item -= 1; },
                    AppEvent::Down => {
                        if self.problem_count() > 0 && self.selected_problem_item < self.problem_count() - 1 {
                            self.selected_problem_item += 1;
                        }
                    },
                    AppEvent::Enter => {
                        let idx = self.selected_problem_item;
                        if self.is_open_file_entry(idx) {
                            self.should_open_file_dialog = true;
                        } else {
                            self.start_typing_session(idx);
                        }
                    },
                    AppEvent::Escape => {
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
                        if let Some(model) = self.typing_model.take() {
                            // key_input呼び出し前の状態を保存
                            let old_word = model.status.word;
                            let old_line = model.status.line;

                            match typing::key_input(model, c, timestamp) {
                                Model::Typing(new_model) => {
                                    #[cfg(target_arch = "wasm32")]
                                    {
                                        // 単語または行が完了したかをチェック
                                        if new_model.status.line != old_line || new_model.status.word != old_word {
                                            self.should_reset_ime = true;
                                        }
                                    }
                                    self.typing_model = Some(new_model)
                                },
                                Model::Result(result_model) => {
                                    self.result_model = Some(result_model);
                                    self.state = AppState::Result;
                                    self.on_event(AppEvent::ChangeScene);
                                }
                            }
                        }
                    }
                    AppEvent::Backspace => {
                        if let Some(model) = self.typing_model.as_mut() {
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