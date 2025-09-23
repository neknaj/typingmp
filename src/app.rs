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


#[cfg(feature = "uefi")]
use core_maths::CoreFloat; 

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
}

#[cfg(target_arch = "wasm32")]
const MENU_ITEM_COUNT: usize = 1;

#[cfg(not(target_arch = "wasm32"))]
const MENU_ITEM_COUNT: usize = 2;

/// アプリケーションで発生するイベントを定義するenum
pub enum AppEvent {
    Start,
    ChangeScene,
    /// 文字入力イベント (タイムスタンプも受け取るように変更)
    Char { c: char, timestamp: f64 },
    /// バックスペースイベント
    Backspace,
    /// 上キーイベント
    Up,
    /// 下キーイベント
    Down,
    /// エンターキーイベント
    Enter,
    /// エスケープキーイベント
    Escape,
    /// アプリケーション終了イベント
    Quit,
}

/// アプリケーション全体で共有される状態を保持する構造体
pub struct App {
    /// 現在のアプリケーションの状態
    pub state: AppState,
    /// メインメニューで選択されている項目
    pub selected_main_menu_item: usize,
    /// 問題選択画面で選択されている項目
    pub selected_problem_item: usize,
    /// 問題のリスト
    pub problem_list: &'static [&'static str],
    /// タイピング中の状態モデル
    pub typing_model: Option<TypingModel>,
    /// 結果画面の状態モデル
    pub result_model: Option<ResultModel>,
    /// 画面下部に表示されるステータスメッセージ
    pub status_text: String,
    /// 画面右下に表示される操作方法テキスト
    pub instructions_text: String,
    /// アプリケーションが終了すべきかどうかを示すフラグ
    pub should_quit: bool,
}

impl App {
    /// Appの新しいインスタンスを生成する
    pub fn new() -> Self {
        Self {
            state: AppState::MainMenu,
            selected_main_menu_item: 0,
            selected_problem_item: 0,
            problem_list: PROBLEM_FILES_NAMES,
            typing_model: None,
            result_model: None,
            status_text: String::new(),
            instructions_text: String::new(),
            should_quit: false,
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
    pub fn update(&mut self, width: usize, height: usize, font: &FontRef) {
        if self.state != AppState::Typing {
            return;
        }

        if let Some(model) = self.typing_model.as_mut() {
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

                // 4. Smoothly update the scroll value
                let now = model.scroll.scroll as f32;
                let d = target_scroll - now;
                model.scroll.scroll += (d * (d.powi(2)) / (1000000.0 + d.powi(2))) as f64;
            }
        }
    }

    /// アプリケーションイベントを処理する
    pub fn on_event(&mut self, event: AppEvent) {
        // AppStateが変更されたときにシーン固有の初期化を行う
        if let AppEvent::ChangeScene = event {
            match self.state {
                AppState::MainMenu => {
                    self.instructions_text = "Up/Down: Navigate | Enter: Select".to_string();
                }
                AppState::ProblemSelection => {
                    self.instructions_text = "Up/Down: Select | Enter: Start | ESC: Back".to_string();
                }
                AppState::Typing => {
                    self.instructions_text = "ESC: Back to Menu".to_string();
                }
                AppState::Result => {
                    self.instructions_text = "Enter/ESC: Back to Menu".to_string();
                }
            }
        }

        match self.state {
            AppState::MainMenu => {
                self.status_text = "Welcome to Neknaj Typing Multi-Platform".to_string();
                match event {
                    AppEvent::Up => {
                        if self.selected_main_menu_item > 0 {
                            self.selected_main_menu_item -= 1;
                        }
                    }
                    AppEvent::Down => {
                        if self.selected_main_menu_item < MENU_ITEM_COUNT - 1 {
                            self.selected_main_menu_item += 1;
                        }
                    }
                    AppEvent::Enter => match self.selected_main_menu_item {
                        0 => {
                            self.state = AppState::ProblemSelection;
                            self.on_event(AppEvent::ChangeScene);
                        }
                        1 => {
                            #[cfg(not(target_arch = "wasm32"))]
                            {
                                self.should_quit = true;
                            }
                        }
                        _ => {}
                    },
                    _ => {}
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
                        if self.selected_problem_item < self.problem_list.len() - 1 {
                            self.selected_problem_item += 1;
                        }
                    }
                    AppEvent::Enter => {
                        self.start_typing_session(self.selected_problem_item);
                    }
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
                        #[cfg(any(not(feature = "tui"), feature = "gui"))]
                        {
                            #[cfg(not(target_arch = "wasm32"))]
                            {
                                #[cfg(not(feature = "uefi"))]
                                println!("[APP] Received char: '{}'", c);
                                #[cfg(feature = "uefi")]
                                uefi::println!("[APP] Received char: '{}'", c);
                            }
                            #[cfg(target_arch = "wasm32")]
                            web_sys::console::log_1(&format!("[APP] Received char: '{}'", c).into());
                        }

                        if let Some(model) = self.typing_model.take() {
                            match typing::key_input(model, c, timestamp) {
                                Model::Typing(new_model) => {
                                    self.typing_model = Some(new_model);
                                }
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
                        metrics.speed * 60.0, // kpm is often chars per minute
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