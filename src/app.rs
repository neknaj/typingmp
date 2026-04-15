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
const MENU_ITEM_COUNT: usize = 3; // Quitなし

#[cfg(not(target_arch = "wasm32"))]
const MENU_ITEM_COUNT: usize = 4;

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

/// スクロール計算結果のキャッシュ。毎フレームの全セグメント計測を回避する。
/// カーソル位置・ウィンドウサイズ・フォントが変わった場合のみ再計算する。
pub struct ScrollCache {
    pub line: i32,
    pub word: i32,
    pub segment: i32,
    pub char_: i32,
    pub width: usize,
    pub height: usize,
    pub font_choice: FontChoice,
    pub total_width: f32,
    pub cursor_x_offset: f32,
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
    pub fps: f64,
    pub source_scroll: usize, // ProblemSource でのスクロール行数
    pub how_to_use_scroll: usize, // HowToUse でのスクロール行数
    pub scroll_cache: Option<ScrollCache>,
    #[cfg(target_arch = "wasm32")]
    pub should_reset_ime: bool,
    #[cfg(target_arch = "wasm32")]
    pub should_save_custom_problems: bool, // localStorage への保存要求フラグ
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

    /// インデックスがカスタム問題（builtin でも open-file エントリでもない）かどうか
    pub fn is_custom_problem(&self, idx: usize) -> bool {
        let builtin_count = PROBLEM_FILES_NAMES.len();
        idx >= builtin_count && idx < builtin_count + self.custom_problems.len()
    }

    /// 問題のソース種別バッジ文字を返す: "B" = builtin, "W" = web(wasm), "F" = file(non-wasm)
    pub fn problem_source_label(&self, idx: usize) -> &str {
        if idx < PROBLEM_FILES_NAMES.len() {
            "B"
        } else if idx < PROBLEM_FILES_NAMES.len() + self.custom_problems.len() {
            #[cfg(target_arch = "wasm32")]
            { "W" }
            #[cfg(not(target_arch = "wasm32"))]
            { "F" }
        } else {
            "+"
        }
    }

    /// 問題のソーステキストを返す（builtin / custom 両対応、open-file は None）
    pub fn get_problem_source(&self, idx: usize) -> Option<&str> {
        let builtin_count = PROBLEM_FILES_NAMES.len();
        if idx < builtin_count {
            Some(get_problem_content(idx))
        } else if idx < builtin_count + self.custom_problems.len() {
            Some(&self.custom_problems[idx - builtin_count].content)
        } else {
            None
        }
    }

    /// カスタム問題を削除する。選択カーソルを調整する。
    pub fn delete_custom_problem_at(&mut self, idx: usize) {
        let builtin_count = PROBLEM_FILES_NAMES.len();
        let custom_idx = idx.saturating_sub(builtin_count);
        if custom_idx < self.custom_problems.len() {
            self.custom_problems.remove(custom_idx);
            // 削除後に problem_count を超えていれば一つ前に移動
            let count = self.problem_count();
            if count > 0 && self.selected_problem_item >= count {
                self.selected_problem_item = count - 1;
            }
            #[cfg(target_arch = "wasm32")]
            { self.should_save_custom_problems = true; }
        }
    }

    /// カスタム問題を一つ上（インデックスを小さく）に移動する。選択カーソルも追従する。
    pub fn move_custom_problem_up_at(&mut self, idx: usize) {
        let builtin_count = PROBLEM_FILES_NAMES.len();
        let custom_idx = idx.saturating_sub(builtin_count);
        if custom_idx > 0 && custom_idx < self.custom_problems.len() {
            self.custom_problems.swap(custom_idx, custom_idx - 1);
            self.selected_problem_item -= 1;
            #[cfg(target_arch = "wasm32")]
            { self.should_save_custom_problems = true; }
        }
    }

    /// カスタム問題を一つ下（インデックスを大きく）に移動する。選択カーソルも追従する。
    pub fn move_custom_problem_down_at(&mut self, idx: usize) {
        let builtin_count = PROBLEM_FILES_NAMES.len();
        let custom_idx = idx.saturating_sub(builtin_count);
        if custom_idx + 1 < self.custom_problems.len() {
            self.custom_problems.swap(custom_idx, custom_idx + 1);
            self.selected_problem_item += 1;
            #[cfg(target_arch = "wasm32")]
            { self.should_save_custom_problems = true; }
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
        self.scroll_cache = None;
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
                // カーソル位置・ウィンドウサイズ・フォントが変わった場合のみ全セグメント計測を実行する。
                // 毎フレーム全セグメントを measure_text するのは長大行でのFPS低下の主因。
                let font_choice = self.font_choice;
                let status = &model.status;
                let need_recompute = self.scroll_cache.as_ref().map_or(true, |c| {
                    c.line != status.line
                        || c.word != status.word
                        || c.segment != status.segment
                        || c.char_ != status.char_
                        || c.width != width
                        || c.height != height
                        || c.font_choice != font_choice
                });

                let (total_width, cursor_x_offset) = if need_recompute {
                    // 1. Calculate the total width of the current line's BASE text for centering
                    let tw: f32 = current_line_content.words.iter().flat_map(|w| &w.segments).map(|seg| {
                        let text = seg_base_text_owned(seg);
                        gui_renderer::measure_text(font, &text, base_pixel_font_size).0 as f32
                    }).sum();

                    // 2. Calculate the width up to the cursor
                    let mut cx = 0.0f32;
                    for i in 0..status.word as usize {
                        if let Some(word) = current_line_content.words.get(i) {
                            for seg in &word.segments {
                                let text = seg_base_text_owned(seg);
                                cx += gui_renderer::measure_text(font, &text, base_pixel_font_size).0 as f32;
                            }
                        }
                    }
                    if let Some(current_word) = current_line_content.words.get(status.word as usize) {
                        for i in 0..status.segment as usize {
                            if let Some(seg) = current_word.segments.get(i) {
                                let text = seg_base_text_owned(seg);
                                cx += gui_renderer::measure_text(font, &text, base_pixel_font_size).0 as f32;
                            }
                        }
                        if let Some(seg) = current_word.segments.get(status.segment as usize) {
                            let reading_text = seg_reading_text_owned(seg);
                            let typed_part = reading_text.chars().take(status.char_ as usize).collect::<String>();
                            cx += gui_renderer::measure_text(font, &typed_part, base_pixel_font_size).0 as f32;
                        }
                    }

                    self.scroll_cache = Some(ScrollCache {
                        line: status.line, word: status.word,
                        segment: status.segment, char_: status.char_,
                        width, height, font_choice,
                        total_width: tw, cursor_x_offset: cx,
                    });
                    (tw, cx)
                } else {
                    let c = self.scroll_cache.as_ref().unwrap();
                    (c.total_width, c.cursor_x_offset)
                };

                // セッション開始時の最初のフレームで、スクロールの初期値を設定する
                if model.user_input.is_empty() && model.scroll.scroll == 0.0 {
                    model.scroll.scroll = (-(width as f32 / 2.0) - (total_width / 2.0)) as f64;
                }

                // 3. Calculate target scroll position so the cursor is centered
                let target_scroll = cursor_x_offset - total_width / 2.0;

                // 4. Smoothly update the scroll value using delta_time for frame-rate independence
                // 基本速度係数に加え、目標との距離の1.5乗に比例するボーナスを加算する。
                // これにより、問題切り替え直後などで大きく離れている場合は素早く追従し、
                // 通常タイピング中の小さなズレは穏やかに補正する。
                // 順方向(right→left, diff > 0): カーソル追従のため素早く追いかける
                // 逆方向(left→right, diff < 0): かな→漢字確定などでテキストが縮んだ際の
                //   急激な戻りを抑えつつ、長距離は加速する
                let now = model.scroll.scroll;
                let diff = target_scroll as f64 - now;
                let abs_diff = diff.abs();
                let exp_bonus = (abs_diff / 150.0).powf(1.5);
                let scroll_speed_factor = if diff > 0.0 {
                    5.0 + exp_bonus          // 順方向: 基本5倍 + 距離ボーナス
                } else {
                    1.2 + exp_bonus * 0.4    // 逆方向: 基本1.2倍 + 抑制した距離ボーナス
                };
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
                AppState::ProblemSelection => self.instructions_text = self.problem_selection_instructions(),
                AppState::ProblemSource => self.instructions_text = "Up/Down: Scroll | Enter/ESC: Back".to_string(),
                AppState::Typing => self.instructions_text = "ESC: Back to Menu | Tab: Cycle Mode".to_string(),
                AppState::Result => self.instructions_text = "Enter/ESC: Back to Menu".to_string(),
                AppState::Settings => self.instructions_text = "Up/Down: Select | Enter: Apply | ESC: Back".to_string(),
                AppState::HowToUse => self.instructions_text = "Up/Down: Scroll | Enter/ESC: Back".to_string(),
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
                            self.how_to_use_scroll = 0;
                            self.state = AppState::HowToUse;
                            self.on_event(AppEvent::ChangeScene);
                        }
                        2 => {
                            self.state = AppState::Settings;
                            self.on_event(AppEvent::ChangeScene);
                        }
                        3 => {
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
                    AppEvent::Char { c, .. } => {
                        let idx = self.selected_problem_item;
                        match c {
                            'v' | 'V' => {
                                // ソースビューアへ遷移（open-file エントリは除く）
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
                // instructions_text を選択中アイテムに応じて毎回更新
                if self.state == AppState::ProblemSelection {
                    self.instructions_text = self.problem_selection_instructions();
                }
            }
            AppState::ProblemSource => {
                match event {
                    AppEvent::Up => {
                        if self.source_scroll > 0 { self.source_scroll -= 1; }
                    }
                    AppEvent::Down => {
                        let total = self.get_problem_source(self.selected_problem_item)
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
                }
            }
            AppState::HowToUse => {
                self.status_text = "How to Use".to_string();
                match event {
                    AppEvent::Up => {
                        if self.how_to_use_scroll > 0 { self.how_to_use_scroll -= 1; }
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
                            // 誤り入力がある状態で Backspace が押された場合、その位置の TypingCorrectnessChar を
                            // Incorrect から Pending に戻す。これにより大⇔小キーで修正した後に赤表示が残らない。
                            if model.status.last_wrong_keydown.is_some() {
                                let line = model.status.line as usize;
                                let word = model.status.word as usize;
                                let seg = model.status.segment as usize;
                                let char_i = model.status.char_ as usize;
                                if let Some(c) = model.typing_correctness.lines
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