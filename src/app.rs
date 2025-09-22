// ./src/app.rs

// uefi featureが有効な場合、標準のallocクレートをインポート
#[cfg(feature = "uefi")]
extern crate alloc;

// uefi と std で使用する String と format! を切り替える
#[cfg(feature = "uefi")]
use alloc::{format, string::{String, ToString}};
#[cfg(not(feature = "uefi"))]
use std::string::{String, ToString};


/// アプリケーション全体で共有される状態を保持する構造体
pub struct App {
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
            input_text: "Hello, UEFI!".to_string(),
            status_text: "Press any key. (ESC to quit)".to_string(),
            should_quit: false,
        }
    }

    /// 文字キーが押された時の処理
    pub fn on_key(&mut self, c: char) {
        if c != '\u{0}' { // NUL文字は無視する
            self.input_text.push(c);
            self.status_text = format!("Pressed: '{}', Length: {}", c, self.input_text.len());
        }
    }

    /// バックスペースキーが押された時の処理
    pub fn on_backspace(&mut self) {
        if self.input_text.pop().is_some() {
            self.status_text = format!("Backspace pressed. Length: {}", self.input_text.len());
        }
    }
}