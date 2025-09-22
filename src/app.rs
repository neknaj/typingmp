// -----------------------------------------------------------------------------
// モジュール：app - アプリケーションの共通状態とロジック
// -----------------------------------------------------------------------------
pub struct App {
    pub input_text: String,
    pub should_quit: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            input_text: "Hello, World!".to_string(),
            should_quit: false,
        }
    }

    pub fn on_key(&mut self, c: char) {
        self.input_text.push(c);
    }

    pub fn on_backspace(&mut self) {
        self.input_text.pop();
    }
}