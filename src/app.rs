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

/// アプリケーションで発生するイベントを定義するenum
pub enum AppEvent {
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
            status_text: "Welcome to Neknaj Typing Multi-Platform".to_string(),
            should_quit: false,
        }
    }

    /// アプリケーションイベントを処理する
    pub fn on_event(&mut self, event: AppEvent) {
        match self.state {
            AppState::Menu => {
                match event {
                    AppEvent::Up => {
                        if self.selected_menu_item > 0 {
                            self.selected_menu_item -= 1;
                        }
                    }
                    AppEvent::Down => {
                        // Assuming 2 menu items: "Start Editing" and "Quit"
                        if self.selected_menu_item < 1 {
                            self.selected_menu_item += 1;
                        }
                    }
                    AppEvent::Enter => {
                        match self.selected_menu_item {
                            0 => {
                                self.state = AppState::Editing;
                                self.status_text = "Start typing! (ESC to return to menu)".to_string();
                            }
                            1 => self.should_quit = true,
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
            AppState::Editing => {
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
                        self.status_text = "Welcome to Neknaj Typing Multi-Platform".to_string();
                    }
                    _ => {}
                }
            }
        }
    }
}