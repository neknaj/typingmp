extern crate alloc;

use super::{App, AppEvent};
use crate::io::{CustomProblem, FontEntry, ProblemSourceProvider};
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

impl App {
    /// 組み込み問題 + カスタム問題 + (gui/wasm では「Open File」エントリ1件) の合計数
    pub fn problem_count(&self) -> usize {
        self.problem_repository.problem_count()
    }

    /// インデックスに対応する表示名を返す
    pub fn problem_name_at(&self, idx: usize) -> &str {
        self.problem_repository
            .problem_name_at(idx)
            .unwrap_or("[ Unknown ]")
    }

    /// そのインデックスが「Open File」エントリかどうか
    pub fn is_open_file_entry(&self, idx: usize) -> bool {
        self.problem_repository.is_open_file_entry(idx)
    }

    /// カスタム問題を追加し、そのインデックスを選択状態にする
    pub fn add_custom_problem(&mut self, name: String, content: String, timestamp_ms: u64) {
        let selected_index = self.problem_repository.add_custom_problem(CustomProblem {
            name,
            content,
            timestamp_ms,
        });
        self.selected_problem_item = selected_index;
        if self.state != super::AppState::ProblemSelection {
            self.state = super::AppState::ProblemSelection;
            self.on_event(AppEvent::ChangeScene);
        }
    }

    pub fn set_custom_problems(&mut self, problems: Vec<CustomProblem>) {
        self.problem_repository.set_custom_problems(problems);
        let count = self.problem_count();
        if count > 0 && self.selected_problem_item >= count {
            self.selected_problem_item = count - 1;
        }
    }

    pub fn custom_problems(&self) -> &[CustomProblem] {
        self.problem_repository.custom_problems()
    }

    /// インデックスがカスタム問題（builtin でも open-file エントリでもない）かどうか
    pub fn is_custom_problem(&self, idx: usize) -> bool {
        self.problem_repository.is_custom_problem(idx)
    }

    /// 問題のソース種別バッジ文字を返す: "B" = builtin, "W" = web(wasm), "F" = file(non-wasm)
    pub fn problem_source_label(&self, idx: usize) -> &str {
        self.problem_repository.problem_source_label(idx)
    }

    /// 問題のソーステキストを返す（builtin / custom 両対応、open-file は None）
    pub fn get_problem_source(&self, idx: usize) -> Option<&str> {
        self.problem_repository.problem_content_ref(idx)
    }

    /// カスタム問題を削除する。選択カーソルを調整する。
    pub fn delete_custom_problem_at(&mut self, idx: usize) {
        if self.problem_repository.delete_custom_problem_at(idx) {
            let count = self.problem_count();
            if count > 0 && self.selected_problem_item >= count {
                self.selected_problem_item = count - 1;
            }
            #[cfg(feature = "wasm")]
            {
                self.should_save_custom_problems = true;
            }
        }
    }

    /// カスタム問題を一つ上（インデックスを小さく）に移動する。選択カーソルも追従する。
    pub fn move_custom_problem_up_at(&mut self, idx: usize) {
        if self.problem_repository.move_custom_problem_up_at(idx) {
            self.selected_problem_item -= 1;
            #[cfg(feature = "wasm")]
            {
                self.should_save_custom_problems = true;
            }
        }
    }

    /// カスタム問題を一つ下（インデックスを大きく）に移動する。選択カーソルも追従する。
    pub fn move_custom_problem_down_at(&mut self, idx: usize) {
        if self.problem_repository.move_custom_problem_down_at(idx) {
            self.selected_problem_item += 1;
            #[cfg(feature = "wasm")]
            {
                self.should_save_custom_problems = true;
            }
        }
    }

    /// ProblemSelection の操作説明を、選択中アイテムに応じて動的に返す
    pub fn problem_selection_instructions(&self) -> String {
        let idx = self.selected_problem_item;
        if self.is_open_file_entry(idx) {
            "Enter: Open | ESC: Back".to_string()
        } else if self.is_custom_problem(idx) {
            "Enter: Start | V: Source | X: Delete | U: Move↑ | D: Move↓ | ESC: Back".to_string()
        } else {
            "Enter: Start | V: Source | ESC: Back".to_string()
        }
    }

    pub fn set_available_fonts(&mut self, entries: Vec<FontEntry>) {
        self.available_fonts = entries;
        if self.selected_font_item >= self.available_fonts.len() {
            self.selected_font_item = 0;
        }
    }
}
