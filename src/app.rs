// ./src/app.rs

// uefi featureが有効な場合、標準のallocクレートをインポート
#[cfg(feature = "uefi")]
extern crate alloc;

// uefi と std で使用する String と format! を切り替える
#[cfg(feature = "uefi")]
use alloc::{format, string::{String, ToString}};
#[cfg(not(feature = "uefi"))]
use std::string::{String, ToString};


/// アプリケーションの現在の状態（シーン）を定義するenum
#[derive(PartialEq)]
pub enum AppState {
    Menu,
    Editing,
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
    pub input_text: String,
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
            status_text: String::new(),
            instructions_text: String::new(),
            should_quit: false,
        }
    }

    /// アプリケーションイベントを処理する
    pub fn on_event(&mut self, event: AppEvent) {
        match self.state {
            AppState::Menu => {
                self.status_text = "Welcome to Neknaj Typing Multi-Platform".to_string();
                self.instructions_text = "ESC: Menu | Enter: Select | Up/Down: Navigate".to_string();
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
                    AppEvent::Enter => {
                        match self.selected_menu_item {
                            0 => {
                                self.state = AppState::Editing;
                                App::on_event(self, AppEvent::ChangeScene);
                                self.input_text.clear();
                            }
                            1 => {
                                #[cfg(not(target_arch = "wasm32"))]
                                {
                                    self.should_quit = true;
                                }
                            }
                            _ => {}
                        }
                    }
                    AppEvent::ChangeScene => {
                        match self.state {
                            AppState::Menu => {
                                self.instructions_text = "Up/Down: Select | Enter: Choose | Q: Quit".to_string();
                            }
                            AppState::Editing => {
                                self.instructions_text = "Type: Input | Backspace: Delete | ESC: Menu".to_string();
                            }
                        }
                    }
                    _ => {}
                }
            }
            AppState::Editing => {
                self.status_text = "Start typing! (ESC to return to menu)".to_string();
                self.instructions_text = "Type: Input | Backspace: Delete | ESC: Menu".to_string();
                match event {
                    AppEvent::Char(c) => {
                        if c != '\u{0}' { // NUL文字は無視する
                            self.input_text.push(c);
                            self.status_text = format!("Pressed: '{}', Length: {}", c, self.input_text.len());
                        }
                    }
                    AppEvent::Backspace => {
                        if self.input_text.pop().is_some() {
                            self.status_text = format!("Backspace pressed. Length: {}", self.input_text.len());
                        }
                    }
                    AppEvent::Escape => {
                        self.state = AppState::Menu;
                        App::on_event(self, AppEvent::ChangeScene);
                    }
                    _ => {}
                }
            }
        }
    }
}