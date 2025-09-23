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

use crate::model::{Model, ResultModel, Scroll, TypingModel, TypingStatus};
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
const MENU_ITEM_COUNT: usize = 1;

#[cfg(not(target_arch = "wasm32"))]
const MENU_ITEM_COUNT: usize = 3; // "Settings" を追加

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
    pub problem_list: &'static [&'static str],
    pub typing_model: Option<TypingModel>,
    pub result_model: Option<ResultModel>,
    pub status_text: String,
    pub instructions_text: String,
    pub tui_display_mode: TuiDisplayMode,
    pub should_quit: bool,
    // フォント管理用のフィールド
    pub fonts: Fonts<'a>,
    pub font_choice: FontChoice,
    pub fps: f64, // FPSを保持するフィールドを追加
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
            problem_list: PROBLEM_FILES_NAMES,
            typing_model: None,
            result_model: None,
            status_text: String::new(),
            instructions_text: String::new(),
            tui_display_mode: TuiDisplayMode::Braille,
            should_quit: false,
            fonts,
            font_choice: FontChoice::YujiSyuku, // デフォルトフォント
            fps: 0.0, // FPSを初期化
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
        let problem_text = get_problem_content(problem_index);
        let content = parser::parse_problem(problem_text);
        let typing_correctness = typing::create_typing_correctness_model(&content);

        self.typing_model = Some(TypingModel {
            content,
            status: TypingStatus {
                line: 0,
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
    }

    /// 毎フレームの状態更新（スクロール計算など）
    pub fn update(&mut self, width: usize, height: usize, delta_time: f64) {
        // FPSを計算して保存
        if delta_time > 0.0 {
            self.fps = 1000.0 / delta_time;
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
                let total_width = current_line_content.segments.iter().map(|seg| {
                    let text = match seg {
                        crate::model::Segment::Plain { text } => text.as_str(),
                        crate::model::Segment::Annotated { base, .. } => base.as_str(),
                    };
                    gui_renderer::measure_text(font, text, base_pixel_font_size).0 as f32
                }).sum::<f32>();

                // セッション開始時の最初のフレームで、スクロールの初期値を設定する
                // これにより、テキストが画面の右側からスライドインする演出が生まれる
                // user_inputが空かつscrollが0.0の場合をセッション開始直後と判断
                if model.user_input.is_empty() && model.scroll.scroll == 0.0 {
                    // テキストブロックの左端が画面の右端に来るようにスクロール値を計算
                    model.scroll.scroll = (-(width as f32 / 2.0) - (total_width / 2.0)) as f64;
                }

                // 2. Calculate the width up to the cursor
                let mut cursor_x_offset = 0.0;
                // Add width of completed segments (based on BASE text)
                for i in 0..model.status.segment as usize {
                    if let Some(seg) = current_line_content.segments.get(i) {
                        let text = match seg {
                            crate::model::Segment::Plain { text } => text.as_str(),
                            crate::model::Segment::Annotated { base, .. } => base.as_str(),
                        };
                        cursor_x_offset += gui_renderer::measure_text(font, text, base_pixel_font_size).0 as f32;
                    }
                }

                // For the current segment, add the width of the typed READING text
                if let Some(seg) = current_line_content.segments.get(model.status.segment as usize) {
                    let reading_text = match seg {
                        crate::model::Segment::Plain { text } => text,
                        crate::model::Segment::Annotated { reading, .. } => reading,
                    };
                    // Get the substring of the reading text that has been typed so far.
                    let typed_reading_part = reading_text.chars().take(model.status.char_ as usize).collect::<String>();
                    // Measure the actual pixel width of the typed part.
                    let typed_reading_width = gui_renderer::measure_text(font, &typed_reading_part, base_pixel_font_size).0 as f32;
                    // Add this width to the cursor offset.
                    cursor_x_offset += typed_reading_width;
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
                    AppEvent::Down => if self.selected_problem_item < self.problem_list.len() - 1 { self.selected_problem_item += 1; },
                    AppEvent::Enter => self.start_typing_session(self.selected_problem_item),
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
                            match typing::key_input(model, c, timestamp) {
                                Model::Typing(new_model) => self.typing_model = Some(new_model),
                                Model::Result(result_model) => {
                                    self.result_model = Some(result_model);
                                    self.state = AppState::Result;
                                    self.on_event(AppEvent::ChangeScene);
                                }
                            }
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