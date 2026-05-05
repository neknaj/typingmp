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
#[cfg(feature = "uefi")]
use core_maths::CoreFloat;
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
        Segment::Anno { inner, .. } => inner.iter().map(seg_base_text_owned).collect(),
    }
}

// セグメントの reading テキストを返す（Anno は inner を連結）
fn seg_reading_text_owned(seg: &Segment) -> String {
    match seg {
        Segment::Plain { text } => text.clone(),
        Segment::Annotated { reading, .. } => reading.clone(),
        Segment::Anno { inner, .. } => inner.iter().map(seg_reading_text_owned).collect(),
    }
}

fn seg_ruby_text_owned(seg: &Segment) -> Option<String> {
    match seg {
        Segment::Plain { .. } => None,
        Segment::Annotated { reading, .. } => {
            if reading.is_empty() {
                None
            } else {
                Some(reading.clone())
            }
        }
        Segment::Anno { inner, .. } => {
            let reading = inner.iter().map(seg_reading_text_owned).collect::<String>();
            if reading.is_empty() {
                None
            } else {
                Some(reading)
            }
        }
    }
}
use crate::parser;
use crate::renderer::gui_renderer;
use crate::typing;
use crate::ui; // typing_rendererの代わりにuiをインポート
use ab_glyph::FontVec;

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

/// どのスクリプト種別のフォントを選択しているか
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Script {
    Japanese,
    TraditionalChinese,
    SimplifiedChinese,
}

impl Script {
    pub fn label(self) -> &'static str {
        match self {
            Script::Japanese => "Japanese",
            Script::TraditionalChinese => "Traditional Chinese",
            Script::SimplifiedChinese => "Simplified Chinese",
        }
    }
}

/// ディスカバリーされたフォントエントリ（デスクトップのみ）
#[cfg(not(any(target_arch = "wasm32", feature = "uefi")))]
#[derive(Clone)]
pub enum FontSource {
    Bundled,
    System,
}

#[cfg(not(any(target_arch = "wasm32", feature = "uefi")))]
#[derive(Clone)]
pub struct FontEntry {
    pub name: String,
    pub path: std::path::PathBuf,
    pub source: FontSource,
}

/// ロードされたフォントデータ（スクリプト別）
pub struct Fonts {
    pub japanese: FontVec,
    pub traditional_chinese: Option<FontVec>,
    pub simplified_chinese: Option<FontVec>,
}

impl Fonts {
    /// スクリプトに対応するフォントを返す（未設定時は japanese で代替）
    pub fn get_for_script(&self, script: Script) -> &FontVec {
        match script {
            Script::Japanese => &self.japanese,
            Script::TraditionalChinese => {
                self.traditional_chinese.as_ref().unwrap_or(&self.japanese)
            }
            Script::SimplifiedChinese => self.simplified_chinese.as_ref().unwrap_or(&self.japanese),
        }
    }

    /// メインフォント（日本語フォント）を返す
    pub fn primary(&self) -> &FontVec {
        &self.japanese
    }
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
/// カーソル位置・ウィンドウサイズが変わった場合のみ再計算する。
/// スクロール計算結果のキャッシュ。毎フレームの全セグメント計測を回避する。
#[derive(Clone)]
pub(crate) struct ScrollLineSegmentCache {
    pub base_text: String,
    pub ruby_text: Option<String>,
    pub reading_text: String,
    pub base_width: f32,
    pub reading_width_prefix: Vec<f32>,
    pub word_index: usize,
    pub segment_index: usize,
}

#[derive(Clone)]
pub(crate) struct ScrollLineCache {
    pub line: i32,
    pub total_width: f32,
    /// セグメントの累積幅（長さ = segments.len()+1）
    pub segment_prefix_width: Vec<f32>,
    pub word_segment_starts: Vec<usize>,
    pub segments: Vec<ScrollLineSegmentCache>,
}

#[derive(Clone)]
struct ScrollCursorState {
    pub line: i32,
    pub word: i32,
    pub segment: i32,
    pub char_: i32,
}

#[derive(Clone)]
pub(crate) struct ScrollCacheState {
    pub width: usize,
    pub height: usize,
    pub font_pixel_size: f32,
    pub gap_width: f32,
    pub line_origin: f32,
    pub cursor_in_line: f32,
    pub cursor_world: f32,
    pub cursor_state: ScrollCursorState,
    pub current: ScrollLineCache,
}

#[derive(Clone)]
pub(crate) enum ScrollCache {
    Empty,
    Ready(ScrollCacheState),
}

fn build_reading_width_prefix(font: &FontVec, text: &str, font_pixel_size: f32) -> Vec<f32> {
    let mut prefix = Vec::with_capacity(text.chars().count() + 1);
    prefix.push(0.0);
    let mut total = 0.0f32;
    for character in text.chars() {
        let mut buf = [0u8; 4];
        let ch = character.encode_utf8(&mut buf);
        total += gui_renderer::measure_text(font, ch, font_pixel_size).0 as f32;
        prefix.push(total);
    }
    prefix
}

fn build_scroll_line_cache(
    line: &crate::model::Line,
    font: &FontVec,
    font_pixel_size: f32,
    line_index: i32,
) -> ScrollLineCache {
    let mut segments = Vec::new();
    let mut segment_prefix_width = Vec::new();
    segment_prefix_width.push(0.0);
    let mut word_segment_starts = Vec::with_capacity(line.words.len());
    let mut total_width = 0.0f32;

    for word in &line.words {
        word_segment_starts.push(segments.len());
        for segment in &word.segments {
            let base_text = seg_base_text_owned(segment);
            let reading_text = seg_reading_text_owned(segment);
            let ruby_text = seg_ruby_text_owned(segment);
            let base_width = gui_renderer::measure_text(font, &base_text, font_pixel_size).0 as f32;
            let reading_width_prefix =
                build_reading_width_prefix(font, &reading_text, font_pixel_size);
            total_width += base_width;
            segment_prefix_width.push(total_width);
            segments.push(ScrollLineSegmentCache {
                base_text,
                ruby_text,
                reading_text,
                base_width,
                reading_width_prefix,
                word_index: 0,
                segment_index: 0,
            });
        }
    }

    for word_index in 0..line.words.len() {
        let start = word_segment_starts.get(word_index).copied().unwrap_or(0);
        let end = word_segment_starts
            .get(word_index + 1)
            .copied()
            .unwrap_or(segments.len());
        for segment_index in start..end {
            if let Some(item) = segments.get_mut(segment_index) {
                item.word_index = word_index;
                item.segment_index = segment_index - start;
            }
        }
    }

    ScrollLineCache {
        line: line_index,
        total_width,
        segment_prefix_width,
        word_segment_starts,
        segments,
    }
}

fn line_total_width(line: &crate::model::Line, font: &FontVec, font_pixel_size: f32) -> f32 {
    let mut total = 0.0f32;
    for word in &line.words {
        for segment in &word.segments {
            total +=
                gui_renderer::measure_text(font, &seg_base_text_owned(segment), font_pixel_size).0
                    as f32;
        }
    }
    total
}

fn line_origin_from_start(
    target_line: usize,
    lines: &[crate::model::Line],
    font: &FontVec,
    font_pixel_size: f32,
    gap_width: f32,
) -> f32 {
    let mut origin = 0.0f32;
    let max_line = target_line.min(lines.len());
    for line_idx in 0..max_line {
        let line = if let Some(line) = lines.get(line_idx) {
            line
        } else {
            return origin;
        };
        origin += line_total_width(line, font, font_pixel_size) + gap_width;
    }
    origin
}

fn line_origin_from_previous(
    previous: &ScrollCacheState,
    target_line: usize,
    lines: &[crate::model::Line],
    font: &FontVec,
    font_pixel_size: f32,
    gap_width: f32,
) -> f32 {
    let Some(previous_line) = usize::try_from(previous.current.line).ok() else {
        return line_origin_from_start(target_line, lines, font, font_pixel_size, gap_width);
    };

    if target_line == previous_line {
        return previous.line_origin;
    }

    let mut origin = previous.line_origin;
    if target_line > previous_line {
        for line_idx in previous_line..target_line {
            let line = if let Some(line) = lines.get(line_idx) {
                line
            } else {
                return line_origin_from_start(
                    target_line,
                    lines,
                    font,
                    font_pixel_size,
                    gap_width,
                );
            };
            let width = if line_idx == previous_line {
                previous.current.total_width
            } else {
                line_total_width(line, font, font_pixel_size)
            };
            origin += width + gap_width;
        }
    } else {
        for line_idx in target_line..previous_line {
            let line = if let Some(line) = lines.get(line_idx) {
                line
            } else {
                return line_origin_from_start(
                    target_line,
                    lines,
                    font,
                    font_pixel_size,
                    gap_width,
                );
            };
            let width = line_total_width(line, font, font_pixel_size);
            origin -= width + gap_width;
        }
    }

    origin
}

fn cursor_position_from_status(
    cache: &ScrollLineCache,
    status_line: i32,
    status_word: i32,
    status_segment: i32,
    status_char: i32,
) -> (f32, ScrollCursorState) {
    let status_word_usize = usize::try_from(status_word).ok();
    let status_segment_usize = usize::try_from(status_segment).ok();
    let status_char_usize = usize::try_from(status_char).ok().unwrap_or(0);
    let mut cursor_in_line = cache.segment_prefix_width.last().copied().unwrap_or(0.0);

    if let Some(word_idx) = status_word_usize {
        if word_idx < cache.word_segment_starts.len() {
            let segment_start = cache.word_segment_starts[word_idx];
            let segment_end = cache
                .word_segment_starts
                .get(word_idx + 1)
                .copied()
                .unwrap_or(cache.segments.len());
            let segment_count = segment_end.saturating_sub(segment_start);
            let segment_idx = if segment_count == 0 {
                segment_start
            } else {
                segment_start + status_segment_usize.unwrap_or(0).min(segment_count - 1)
            };
            let base = cache
                .segment_prefix_width
                .get(segment_idx)
                .copied()
                .unwrap_or(0.0);
            let mut typed_width = 0.0f32;
            if let Some(seg_cache) = cache.segments.get(segment_idx) {
                let typed_len =
                    status_char_usize.min(seg_cache.reading_width_prefix.len().saturating_sub(1));
                typed_width = seg_cache
                    .reading_width_prefix
                    .get(typed_len)
                    .copied()
                    .unwrap_or(0.0);
            }
            cursor_in_line = (base + typed_width).min(cache.total_width);
        } else {
            cursor_in_line = cache.total_width;
        }
    }

    (
        cursor_in_line,
        ScrollCursorState {
            line: status_line,
            word: status_word,
            segment: status_segment,
            char_: status_char,
        },
    )
}

/// アプリケーション全体で共有される状態を保持する構造体
pub struct App {
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
    pub fonts: Fonts,
    /// Settings画面で選択中のスクリプト
    pub settings_script: Script,
    /// フォントピッカーを開いているか
    pub settings_picking_font: bool,
    /// フォントピッカー内の選択インデックス
    pub selected_font_item: usize,
    /// 発見されたフォント一覧（起動時にディスカバリー）
    #[cfg(not(any(target_arch = "wasm32", feature = "uefi")))]
    pub available_fonts: Vec<FontEntry>,
    pub fps: f64,
    pub source_scroll: usize,     // ProblemSource でのスクロール行数
    pub how_to_use_scroll: usize, // HowToUse でのスクロール行数
    pub scroll_cache: Option<ScrollCache>,
    #[cfg(target_arch = "wasm32")]
    pub should_reset_ime: bool,
    #[cfg(target_arch = "wasm32")]
    pub should_save_custom_problems: bool, // localStorage への保存要求フラグ
}

/// 非WASM・非UEFI環境でフォントファイルを探索し FontEntry のリストを返す
#[cfg(not(any(target_arch = "wasm32", feature = "uefi")))]
pub fn discover_available_fonts() -> Vec<FontEntry> {
    let mut entries: Vec<FontEntry> = Vec::new();

    let mut search_dirs: Vec<(std::path::PathBuf, FontSource)> = Vec::new();

    // バンドル済みフォントディレクトリ
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            search_dirs.push((dir.join("fonts"), FontSource::Bundled));
        }
    }
    search_dirs.push((std::path::PathBuf::from("fonts"), FontSource::Bundled));

    // OS 別のシステムフォントディレクトリ
    #[cfg(target_os = "windows")]
    {
        search_dirs.push((
            std::path::PathBuf::from(r"C:\Windows\Fonts"),
            FontSource::System,
        ));
    }
    #[cfg(target_os = "macos")]
    {
        search_dirs.push((
            std::path::PathBuf::from("/System/Library/Fonts"),
            FontSource::System,
        ));
        search_dirs.push((
            std::path::PathBuf::from("/Library/Fonts"),
            FontSource::System,
        ));
        if let Ok(home) = std::env::var("HOME") {
            search_dirs.push((
                std::path::PathBuf::from(home).join("Library/Fonts"),
                FontSource::System,
            ));
        }
    }
    #[cfg(target_os = "linux")]
    {
        search_dirs.push((
            std::path::PathBuf::from("/usr/share/fonts"),
            FontSource::System,
        ));
        search_dirs.push((
            std::path::PathBuf::from("/usr/local/share/fonts"),
            FontSource::System,
        ));
        if let Ok(home) = std::env::var("HOME") {
            search_dirs.push((
                std::path::PathBuf::from(home).join(".local/share/fonts"),
                FontSource::System,
            ));
        }
    }

    let mut seen_names: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (dir, source) in search_dirs {
        if let Ok(read_dir) = std::fs::read_dir(&dir) {
            for entry in read_dir.flatten() {
                let path = entry.path();
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    let ext_lower = ext.to_lowercase();
                    if ext_lower == "ttf" || ext_lower == "otf" {
                        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                            let name = stem.to_string();
                            if seen_names.insert(name.clone()) {
                                entries.push(FontEntry {
                                    name,
                                    path,
                                    source: source.clone(),
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    entries
}

impl App {
    /// Appの新しいインスタンスを生成する
    pub fn new(fonts: Fonts) -> Self {
        #[cfg(feature = "uefi")]
        uefi::println!("APP: START");

        #[cfg(not(any(target_arch = "wasm32", feature = "uefi")))]
        let available_fonts = discover_available_fonts();

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
            settings_script: Script::Japanese,
            settings_picking_font: false,
            selected_font_item: 0,
            #[cfg(not(any(target_arch = "wasm32", feature = "uefi")))]
            available_fonts,
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
        {
            base + 1
        }
        #[cfg(not(any(feature = "gui", target_arch = "wasm32")))]
        {
            base
        }
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
        {
            idx == PROBLEM_FILES_NAMES.len() + self.custom_problems.len()
        }
        #[cfg(not(any(feature = "gui", target_arch = "wasm32")))]
        {
            let _ = idx;
            false
        }
    }

    /// カスタム問題を追加し、そのインデックスを選択状態にする
    pub fn add_custom_problem(&mut self, name: String, content: String, timestamp_ms: u64) {
        self.custom_problems.push(CustomProblem {
            name,
            content,
            timestamp_ms,
        });
        // 追加された問題のインデックスを選択
        self.selected_problem_item = PROBLEM_FILES_NAMES.len() + self.custom_problems.len() - 1;
        if self.state != AppState::ProblemSelection {
            self.state = AppState::ProblemSelection;
            self.on_event(AppEvent::ChangeScene);
        }
    }

    /// 現在の設定言語に合わせたメインフォントへの参照を取得する
    pub fn get_current_font(&self) -> &FontVec {
        self.fonts.get_for_script(self.settings_script)
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
            {
                "W"
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                "F"
            }
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
            {
                self.should_save_custom_problems = true;
            }
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
            {
                self.should_save_custom_problems = true;
            }
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

    /// 選択中のスクリプトに対してフォントファイルをロードして適用する
    #[cfg(not(feature = "uefi"))]
    pub fn load_font_for_script(&mut self, script: Script, path: &std::path::Path) {
        if let Ok(data) = std::fs::read(path) {
            if let Ok(font) = FontVec::try_from_vec(data) {
                match script {
                    Script::Japanese => self.fonts.japanese = font,
                    Script::TraditionalChinese => self.fonts.traditional_chinese = Some(font),
                    Script::SimplifiedChinese => self.fonts.simplified_chinese = Some(font),
                }
                // フォントが変わったのでスクロールキャッシュを破棄する
                self.scroll_cache = None;
            }
        }
    }

    #[cfg(feature = "uefi")]
    pub fn load_font_for_script(&mut self, _script: Script, _path: &str) {
        self.scroll_cache = None;
    }

    /// 新しいタイピングセッションを開始する
    fn start_typing_session(&mut self, problem_index: usize) {
        // 選択されたインデックスに基づいて問題文を読み込む
        let builtin_count = PROBLEM_FILES_NAMES.len();
        let problem_text_owned: String;
        let problem_text: &str = if problem_index < builtin_count {
            get_problem_content(problem_index)
        } else {
            problem_text_owned = self.custom_problems[problem_index - builtin_count]
                .content
                .clone();
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
            total_type_count: 0,
            total_miss_count: 0,
            first_input_time: None,
            last_input_time: None,
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
    /// 毎フレームの状態更新（スクロール計算など）
    pub fn update(&mut self, width: usize, height: usize, delta_time: f64) {
        // FPSを計算して保存
        if delta_time > 0.0 {
            let new_fps = 1000.0 / delta_time;
            self.fps = if self.fps == 0.0 {
                new_fps
            } else {
                self.fps * 0.9 + new_fps * 0.1
            };
        }

        if self.state != AppState::Typing {
            return;
        }
        // delta_timeが極端に大きい場合（デバッガで停止した場合など）にスクロールが飛びすぎるのを防ぐ
        let clamped_delta_time = delta_time.min(100.0);

        if let Some(model) = self.typing_model.as_mut() {
            let font = self.fonts.get_for_script(self.settings_script);
            let base_font_size_enum = crate::ui::FontSize::WindowHeight(ui::BASE_FONT_SIZE_RATIO);
            let base_pixel_font_size =
                crate::renderer::calculate_pixel_font_size(base_font_size_enum, width, height);
            let gap_width = width as f32;

            let line_idx = match usize::try_from(model.status.line) {
                Ok(line_idx) => line_idx,
                Err(_) => return,
            };
            if let Some(current_line_content) = model.content.lines.get(line_idx) {
                let status = &model.status;

                let rebuild_cache = self.scroll_cache.as_ref().is_none_or(|cache| match cache {
                    ScrollCache::Empty => true,
                    ScrollCache::Ready(ready) => {
                        ready.width != width
                            || ready.height != height
                            || (ready.font_pixel_size - base_pixel_font_size).abs() > f32::EPSILON
                            || ready.current.line != status.line
                    }
                });

                let current_cache = if rebuild_cache {
                    build_scroll_line_cache(
                        current_line_content,
                        font,
                        base_pixel_font_size,
                        status.line,
                    )
                } else {
                    match &self.scroll_cache {
                        Some(ScrollCache::Ready(ready)) => ready.current.clone(),
                        _ => build_scroll_line_cache(
                            current_line_content,
                            font,
                            base_pixel_font_size,
                            status.line,
                        ),
                    }
                };

                let line_origin = match &self.scroll_cache {
                    Some(ScrollCache::Ready(previous_cache))
                        if (previous_cache.font_pixel_size - base_pixel_font_size).abs()
                            <= f32::EPSILON =>
                    {
                        line_origin_from_previous(
                            previous_cache,
                            line_idx,
                            &model.content.lines,
                            font,
                            base_pixel_font_size,
                            gap_width,
                        )
                    }
                    _ => line_origin_from_start(
                        line_idx,
                        &model.content.lines,
                        font,
                        base_pixel_font_size,
                        gap_width,
                    ),
                };

                let (cursor_in_line, cursor_state) = cursor_position_from_status(
                    &current_cache,
                    status.line,
                    status.word,
                    status.segment,
                    status.char_,
                );
                let cursor_world = line_origin + cursor_in_line;

                let mut target_scroll = f64::from(cursor_world) - (f64::from(gap_width) * 0.5);
                if let Some(ScrollCache::Ready(previous_cache)) = &self.scroll_cache {
                    let previous_target = previous_cache.cursor_world as f64
                        - (previous_cache.gap_width as f64 * 0.5);
                    if (cursor_world >= previous_cache.cursor_world
                        && target_scroll < previous_target)
                        || (cursor_world < previous_cache.cursor_world
                            && target_scroll > previous_target)
                    {
                        target_scroll = previous_target;
                    }
                }

                if model.user_input.is_empty() && model.scroll.scroll == 0.0 {
                    model.scroll.scroll = target_scroll;
                }

                let now = model.scroll.scroll;
                let diff = target_scroll - now;
                model.scroll.scroll += diff * 7.5 * (clamped_delta_time / 1000.0);

                self.scroll_cache = Some(ScrollCache::Ready(ScrollCacheState {
                    width,
                    height,
                    font_pixel_size: base_pixel_font_size,
                    gap_width,
                    line_origin,
                    cursor_in_line,
                    cursor_world,
                    cursor_state,
                    current: current_cache,
                }));
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
                AppState::MainMenu => {
                    self.instructions_text = "Up/Down: Navigate | Enter: Select".to_string()
                }
                AppState::ProblemSelection => {
                    self.instructions_text = self.problem_selection_instructions()
                }
                AppState::ProblemSource => {
                    self.instructions_text = "Up/Down: Scroll | Enter/ESC: Back".to_string()
                }
                AppState::Typing => {
                    self.instructions_text = "ESC: Back to Menu | Tab: Cycle Mode".to_string()
                }
                AppState::Result => self.instructions_text = "Enter/ESC: Back to Menu".to_string(),
                AppState::Settings => {
                    self.instructions_text =
                        "Up/Down: Select | Enter: Apply | ESC: Back".to_string()
                }
                AppState::HowToUse => {
                    self.instructions_text = "Up/Down: Scroll | Enter/ESC: Back".to_string()
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
                            {
                                self.should_quit = true;
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
            AppState::Settings => {
                self.status_text = "Select a font for each script.".to_string();
                if !self.settings_picking_font {
                    // スクリプト選択モード: Up/Down で 3 スクリプトをサイクル
                    const SCRIPT_COUNT: usize = 3;
                    match event {
                        AppEvent::Up => {
                            if self.selected_settings_item > 0 {
                                self.selected_settings_item -= 1;
                            }
                            self.settings_script = match self.selected_settings_item {
                                0 => Script::Japanese,
                                1 => Script::TraditionalChinese,
                                _ => Script::SimplifiedChinese,
                            };
                        }
                        AppEvent::Down => {
                            if self.selected_settings_item < SCRIPT_COUNT - 1 {
                                self.selected_settings_item += 1;
                            }
                            self.settings_script = match self.selected_settings_item {
                                0 => Script::Japanese,
                                1 => Script::TraditionalChinese,
                                _ => Script::SimplifiedChinese,
                            };
                        }
                        AppEvent::Enter => {
                            // 選択スクリプトのフォントピッカーを開く
                            self.settings_picking_font = true;
                            self.selected_font_item = 0;
                        }
                        AppEvent::Escape => {
                            self.state = AppState::MainMenu;
                            self.on_event(AppEvent::ChangeScene);
                        }
                        _ => {}
                    }
                } else {
                    // フォントピッカーモード: Up/Down で available_fonts を選択、Enter で適用
                    #[cfg(not(any(target_arch = "wasm32", feature = "uefi")))]
                    {
                        let font_count = self.available_fonts.len();
                        match event {
                            AppEvent::Up => {
                                if self.selected_font_item > 0 {
                                    self.selected_font_item -= 1;
                                }
                            }
                            AppEvent::Down => {
                                if font_count > 0 && self.selected_font_item < font_count - 1 {
                                    self.selected_font_item += 1;
                                }
                            }
                            AppEvent::Enter => {
                                if self.selected_font_item < font_count {
                                    let path =
                                        self.available_fonts[self.selected_font_item].path.clone();
                                    let script = self.settings_script;
                                    self.load_font_for_script(script, &path);
                                }
                                self.settings_picking_font = false;
                            }
                            AppEvent::Escape => {
                                self.settings_picking_font = false;
                            }
                            _ => {}
                        }
                    }
                    // WASM / UEFI ではフォントピッカー操作なし
                    #[cfg(any(target_arch = "wasm32", feature = "uefi"))]
                    {
                        match event {
                            AppEvent::Escape | AppEvent::Enter => {
                                self.settings_picking_font = false;
                            }
                            _ => {}
                        }
                    }
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
                        if self.problem_count() > 0
                            && self.selected_problem_item < self.problem_count() - 1
                        {
                            self.selected_problem_item += 1;
                        }
                    }
                    AppEvent::Enter => {
                        let idx = self.selected_problem_item;
                        if self.is_open_file_entry(idx) {
                            self.should_open_file_dialog = true;
                        } else {
                            self.start_typing_session(idx);
                        }
                    }
                    AppEvent::Escape => {
                        self.state = AppState::MainMenu;
                        self.on_event(AppEvent::ChangeScene);
                    }
                    AppEvent::Char { c, .. } => {
                        let idx = self.selected_problem_item;
                        match c {
                            'v' | 'V' => {
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
                if self.state == AppState::ProblemSelection {
                    self.instructions_text = self.problem_selection_instructions();
                }
            }
            AppState::ProblemSource => match event {
                AppEvent::Up => {
                    if self.source_scroll > 0 {
                        self.source_scroll -= 1;
                    }
                }
                AppEvent::Down => {
                    let total = self
                        .get_problem_source(self.selected_problem_item)
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
            },
            AppState::HowToUse => {
                self.status_text = "How to Use".to_string();
                match event {
                    AppEvent::Up => {
                        if self.how_to_use_scroll > 0 {
                            self.how_to_use_scroll -= 1;
                        }
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
                            #[cfg(target_arch = "wasm32")]
                            let old_position = (model.status.line, model.status.word);

                            match typing::key_input(model, c, timestamp) {
                                Model::Typing(new_model) => {
                                    #[cfg(target_arch = "wasm32")]
                                    {
                                        if (new_model.status.line, new_model.status.word)
                                            != old_position
                                        {
                                            self.should_reset_ime = true;
                                        }
                                    }
                                    self.typing_model = Some(new_model)
                                }
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
                            if model.status.last_wrong_keydown.is_some() {
                                let line = model.status.line as usize;
                                let word = model.status.word as usize;
                                let seg = model.status.segment as usize;
                                let char_i = model.status.char_ as usize;
                                if let Some(c) = model
                                    .typing_correctness
                                    .lines
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
