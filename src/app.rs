// ./src/app.rs

// uefi featureが有効な場合、標準のallocクレートをインポート
#[cfg(feature = "uefi")]
extern crate alloc;

// uefi と std で使用する String と format! を切り替える
#[cfg(feature = "uefi")]
use alloc::{
    format,
    string::{String, ToString},
};
#[cfg(not(feature = "uefi"))]
use std::string::{String, ToString};

use crate::model::{Model, ResultModel, Scroll, TypingModel, TypingStatus};
use crate::parser;
use crate::typing;

/// アプリケーションの現在の状態（シーン）を定義するenum
#[derive(PartialEq)]
pub enum AppState {
    Menu,
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
    /// 文字入力イベント
    Char(char),
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
    /// メニューで選択されている項目
    pub selected_menu_item: usize,
    /// ユーザーが入力したテキスト
    pub input_text: String, // This can be removed or repurposed
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
            state: AppState::Menu,
            selected_menu_item: 0,
            input_text: String::new(),
            typing_model: None,
            result_model: None,
            status_text: String::new(),
            instructions_text: String::new(),
            should_quit: false,
        }
    }

    /// 新しいタイピングセッションを開始する
    fn start_typing_session(&mut self) {
        // examples/sample.txtから問題文を読み込む
        let problem_text = include_str!("../examples/MIT.ntq");
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

    /// アプリケーションイベントを処理する
    pub fn on_event(&mut self, event: AppEvent) {
        // AppStateが変更されたときにシーン固有の初期化を行う
        if let AppEvent::ChangeScene = event {
            match self.state {
                AppState::Menu => {
                    self.instructions_text = "Up/Down: Navigate | Enter: Select".to_string();
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
            AppState::Menu => {
                self.status_text = "Welcome to Neknaj Typing Multi-Platform".to_string();
                match event {
                    AppEvent::Up => {
                        if self.selected_menu_item > 0 {
                            self.selected_menu_item -= 1;
                        }
                    }
                    AppEvent::Down => {
                        if self.selected_menu_item < MENU_ITEM_COUNT - 1 {
                            self.selected_menu_item += 1;
                        }
                    }
                    AppEvent::Enter => match self.selected_menu_item {
                        0 => self.start_typing_session(),
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
            AppState::Typing => {
                self.status_text = "Start typing!".to_string();
                match event {
                    AppEvent::Char(c) => {
                        if let Some(model) = self.typing_model.take() {
                            match typing::key_input(model, c) {
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
                        self.state = AppState::Menu;
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
                        self.state = AppState::Menu;
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
