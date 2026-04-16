// src/ui.rs

// uefi featureが有効な場合、標準のallocクレートをインポート
#[cfg(feature = "uefi")]
extern crate alloc;

// uefi で f64::floor() を使うために必要
#[cfg(feature = "uefi")]
use core_maths::CoreFloat;

// uefi と std で使用する Vec と vec! を切り替える
#[cfg(feature = "uefi")]
use alloc::{vec, vec::Vec};
#[cfg(not(feature = "uefi"))]
use std::vec::Vec;

// uefi と std で使用する String と format! を切り替える
#[cfg(feature = "uefi")]
use alloc::{format, string::{String, ToString}};
#[cfg(not(feature = "uefi"))]
use std::string::{String, ToString};

use crate::app::{App, AppState, FontChoice, ScrollCache}; // ProblemSource は AppState に含まれる
use crate::model::{Segment, TypingCorrectnessChar, TypingCorrectnessSegment, TypingCorrectnessWord};
use crate::renderer::{calculate_pixel_font_size, gui_renderer};
use crate::typing; // For calculate_total_metrics
use ab_glyph::FontVec;

/// 画面上の描画基準点を定義するenum
#[derive(Clone, Copy)]
pub enum Anchor {
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    Center,
    CenterRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

/// Anchorからのオフセット（移動量）を定義する構造体
#[derive(Clone, Copy)]
pub struct Shift {
    pub x: f32,
    pub y: f32,
}

/// 水平方向の揃え
#[derive(Clone, Copy)]
pub enum HorizontalAlign {
    Left,
    Center,
    Right,
}

/// 垂直方向の揃え
#[derive(Clone, Copy)]
pub enum VerticalAlign {
    Top,
    Center,
    Bottom,
}

/// テキストの揃え方を定義する構造体
#[derive(Clone, Copy)]
pub struct Align {
    pub horizontal: HorizontalAlign,
    pub vertical: VerticalAlign,
}

/// フォントサイズの基準を定義するenum
#[derive(Clone, Copy)]
pub enum FontSize {
    /// ウィンドウの高さに対する比率
    WindowHeight(f32),
    /// ウィンドウの面積の平方根に対する比率
    WindowAreaSqrt(f32),
}

/// グラデーションの定義
#[derive(Clone, Copy)]
pub struct Gradient {
    pub start_color: u32,
    pub end_color: u32,
}

/// 上段（目標テキスト）のセグメントの状態
pub enum UpperSegmentState {
    /// 完了済み（正しくタイプされた）
    Correct,
    /// 完了済み（間違いを含んでいた）
    Incorrect,
    /// 未入力
    Pending,
    /// 現在入力中のアクティブなセグメント
    Active,
}

/// 上段（目標テキスト）を構成する、ルビ付きの1セグメント
pub struct UpperTypingSegment {
    pub base_text: String,
    pub ruby_text: Option<String>,
    /// anno記法の注釈テキスト（ベーステキストの下に表示）
    pub anno_text: Option<String>,
    pub state: UpperSegmentState,
}

/// 下段（入力テキスト）のアクティブ（現在入力中）セグメントを構成する要素
pub enum ActiveLowerElement {
    /// タイプ済みの文字（正誤情報付き）
    Typed { character: char, is_correct: bool },
    /// カーソル
    Cursor,
    /// 未確定のローマ字入力 (例: "k", "ky")
    UnconfirmedInput(String),
    /// 直前の誤入力キー
    LastIncorrectInput(char),
}

/// 下段（入力テキスト）を構成するセグメント
pub enum LowerTypingSegment {
    /// 完了済みのセグメント
    Completed {
        base_text: String,
        ruby_text: Option<String>,
        is_correct: bool,
    },
    /// 現在入力中のアクティブなセグメント
    Active { elements: Vec<ActiveLowerElement> },
}

/// 画面に描画すべき要素の種類とレイアウト情報を定義するenum
pub enum Renderable {
    Background {
        gradient: Gradient,
    },
    Text {
        text: String,
        anchor: Anchor,
        shift: Shift,
        align: Align,
        font_size: FontSize,
        color: u32,
    },
    BigText {
        text: String,
        anchor: Anchor,
        shift: Shift,
        align: Align,
        font_size: FontSize,
        color: u32,
    },
    /// 上段の目標テキスト行全体を表す型
    TypingUpper {
        segments: Vec<UpperTypingSegment>,
        anchor: Anchor,
        shift: Shift,
        align: Align,
        font_size: FontSize, // ベーステキストのフォントサイズ
    },
    /// 下段の入力テキスト行全体を表す型
    TypingLower {
        segments: Vec<LowerTypingSegment>,
        anchor: Anchor,
        shift: Shift,
        align: Align,
        font_size: FontSize, // 入力テキストのフォントサイズ
        target_line_total_width: u32,
    },
    ProgressBar {
        anchor: Anchor,
        shift: Shift,
        width_ratio: f32, // 画面幅に対する比率
        height_ratio: f32, // 画面高さに対する比率
        progress: f32, // 0.0 to 1.0
        bg_color: u32,
        fg_color: u32,
    },
}

/// How to Use 画面に表示する各行: (テキスト, 色) の定義。
/// app.rs から行数参照のために pub(crate) で公開する。
pub(crate) const HOW_TO_USE_CONTENT: &[(&str, u32)] = &[
    // ─── 基本操作 ───
    ("[ 基本操作 ]",                              0xFF_FFDD88),
    ("  ↑ / ↓   : 項目を選択",                   0xFF_CCCCCC),
    ("  Enter    : 選択 / 決定",                  0xFF_CCCCCC),
    ("  Esc      : 前の画面に戻る",               0xFF_CCCCCC),
    ("",                                          0xFF_000000),
    // ─── タイピング ───
    ("[ タイピング ]",                            0xFF_FFDD88),
    ("  ローマ字でひらがな / カタカナを入力",     0xFF_CCCCCC),
    ("  例: か→ka  き→ki  が→ga  は→ha",        0xFF_888888),
    ("  青色 : 正しくタイプされた文字",           0xFF_9097FF),
    ("  赤色 : 間違いを含んだ文字",              0xFF_FF9898),
    ("  白色 : 現在の入力位置",                  0xFF_FFFFFF),
    ("  Backspace : 未確定ローマ字 / 誤り入力を消す", 0xFF_CCCCCC),
    ("",                                          0xFF_000000),
    // ─── 問題選択 ───
    ("[ 問題選択 ]",                              0xFF_FFDD88),
    ("  Enter : タイピング開始",                  0xFF_CCCCCC),
    ("  V     : ソースファイルを確認",            0xFF_CCCCCC),
    ("  X     : カスタム問題を削除",              0xFF_CCCCCC),
    ("  U / D : カスタム問題の順序を変更",        0xFF_CCCCCC),
    ("",                                          0xFF_000000),
    // ─── フリック入力 ───
    ("[ フリック入力 (タッチデバイス) ]",         0xFF_FFDD88),
    ("  タイピング中に画面下部にキーボードが表示", 0xFF_CCCCCC),
    ("  中央タップ : あ段  (例: か → か)",        0xFF_CCCCCC),
    ("  上フリック : う段  (例: か → く)",        0xFF_CCCCCC),
    ("  左フリック : い段  (例: か → き)",        0xFF_CCCCCC),
    ("  右フリック : え段  (例: か → け)",        0xFF_CCCCCC),
    ("  下フリック : お段  (例: か → こ)",        0xFF_CCCCCC),
    ("  大⇔小キー : 直前の誤り入力を変換",       0xFF_CCCCCC),
    ("           か→が→か  は→ば→ぱ→は  など", 0xFF_888888),
    ("           (直前が誤りのときのみ作動)",     0xFF_888888),
    ("  ゐ / ゑ  : 「い」/「え」で代替入力可",   0xFF_AAAAAA),
    ("",                                          0xFF_000000),
    // ─── カスタム問題 ───
    ("[ カスタム問題 (.ntq 形式) ]",              0xFF_FFDD88),
    ("  #title タイトル名",                       0xFF_99DDAA),
    ("  [漢字/よみがな]  : ルビ付きテキスト",    0xFF_99DDAA),
    ("  {内容/注釈}      : 注釈付きテキスト",    0xFF_99DDAA),
    ("  空白 / スラッシュ : 単語の区切り",        0xFF_99DDAA),
    ("  Web版: [Open File...] からアップロード",  0xFF_CCCCCC),
];

#[cfg(target_arch = "wasm32")]
const MENU_ITEMS: [&str; 3] = ["Start Typing", "How to Use", "Settings"];

#[cfg(not(target_arch = "wasm32"))]
const MENU_ITEMS: [&str; 4] = ["Start Typing", "How to Use", "Settings", "Quit"];

// --- タイピング画面のレイアウト定数 ---
pub const BASE_FONT_SIZE_RATIO: f32 = 0.2;
const UPPER_ROW_Y_OFFSET_FACTOR: f32 = 1.3;
const LOWER_ROW_Y_OFFSET_FACTOR: f32 = 0.2;

// --- 色定義 ---
pub const CORRECT_COLOR: u32 = 0xFF_9097FF;
pub const INCORRECT_COLOR: u32 = 0xFF_FF9898;
pub const PENDING_COLOR: u32 = 0xFF_999999;
pub const ACTIVE_COLOR: u32 = 0xFF_FFFFFF;
pub const WRONG_KEY_COLOR: u32 = 0xFF_F55252;
pub const CURSOR_COLOR: u32 = 0xFF_FFFFFF;
pub const UNCONFIRMED_COLOR: u32 = 0xFF_CCCCCC;

/// Appの状態を受け取り、描画リスト（UIレイアウト）を構築する
pub fn build_ui(app: &App, font: &FontVec, width: usize, height: usize) -> Vec<Renderable> {
    let mut render_list = Vec::new();

    let menu_gradient = Gradient { start_color: 0xFF_000010, end_color: 0xFF_000000 };
    let typing_gradient = Gradient { start_color: 0xFF_100010, end_color: 0xFF_000000 };
    let result_gradient = Gradient { start_color: 0xFF_101000, end_color: 0xFF_000000 };
    let settings_gradient = Gradient { start_color: 0xFF_001010, end_color: 0xFF_000000 };

    match app.state {
        AppState::MainMenu => build_main_menu_ui(app, &mut render_list, menu_gradient),
        AppState::Typing => build_typing_ui(app, &mut render_list, typing_gradient, font, width, height),
        AppState::ProblemSelection => build_problem_selection_ui(app, &mut render_list, menu_gradient),
        AppState::ProblemSource => build_problem_source_ui(app, &mut render_list, menu_gradient),
        AppState::Result => build_result_ui(app, &mut render_list, result_gradient),
        AppState::Settings => build_settings_ui(app, &mut render_list, settings_gradient),
        AppState::HowToUse => build_how_to_use_ui(app, &mut render_list, menu_gradient),
    }

    if app.state != AppState::Typing {
        render_list.push(Renderable::Text {
            text: app.status_text.clone(),
            anchor: Anchor::BottomLeft,
            shift: Shift { x: 0.01, y: -0.02 },
            align: Align { horizontal: HorizontalAlign::Left, vertical: VerticalAlign::Bottom },
            font_size: FontSize::WindowHeight(0.04),
            color: 0xFF_CCCCCC,
        });
    }

    // --- 画面右上のFPS表示 ---
    let fps_text = format!("FPS: {:.1}", app.fps);
    render_list.push(Renderable::Text {
        text: fps_text,
        anchor: Anchor::TopRight,
        shift: Shift { x: -0.01, y: 0.01 },
        align: Align { horizontal: HorizontalAlign::Right, vertical: VerticalAlign::Top },
        font_size: FontSize::WindowHeight(0.04),
        color: 0xFF_00FF00, // 緑色
    });

    // --- 画面下部の共通UI ---
    #[cfg(feature = "gui")]
    {
        render_list.push(Renderable::Text {
            text: "GUI".to_string(),
            anchor: Anchor::BottomRight,
            shift: Shift { x: -0.01, y: -0.06 },
            align: Align { horizontal: HorizontalAlign::Right, vertical: VerticalAlign::Bottom },
            font_size: FontSize::WindowHeight(0.04),
            color: 0xFF_AAAAAA,
        });
    }

    #[cfg(all(feature = "tui", not(feature = "gui")))]
    {
        let mode_text = format!("TUI {:?}", app.tui_display_mode);
        render_list.push(Renderable::Text {
            text: mode_text,
            anchor: Anchor::BottomRight,
            shift: Shift { x: -0.01, y: -0.06 },
            align: Align { horizontal: HorizontalAlign::Right, vertical: VerticalAlign::Bottom },
            font_size: FontSize::WindowHeight(0.04),
            color: 0xFF_AAAAAA,
        });
    }

    render_list.push(Renderable::Text {
        text: app.instructions_text.clone(),
        anchor: Anchor::BottomRight,
        shift: Shift { x: -0.01, y: -0.03 },
        align: Align { horizontal: HorizontalAlign::Right, vertical: VerticalAlign::Bottom },
        font_size: FontSize::WindowHeight(0.04),
        color: 0xFF_CCCCCC,
    });

    render_list
}

fn build_main_menu_ui(app: &App, render_list: &mut Vec<Renderable>, gradient: Gradient) {
    render_list.push(Renderable::Background { gradient });
    render_list.push(Renderable::BigText {
        text: "Neknaj Typing MP".to_string(),
        anchor: Anchor::TopCenter,
        shift: Shift { x: 0.0, y: 0.1 },
        align: Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Top },
        font_size: FontSize::WindowHeight(0.20),
        color: 0xFF_FFFFFF,
    });
    for (i, item) in MENU_ITEMS.iter().enumerate() {
        let (text, color) = if i == app.selected_main_menu_item {
            (format!("> {} <", item), 0xFF_FFFF00)
        } else {
            (item.to_string(), 0xFF_FFFFFF)
        };
        render_list.push(Renderable::Text {
            text,
            anchor: Anchor::Center,
            shift: Shift { x: 0.0, y: 0.0 + (i as f32 * 0.1) },
            align: Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Center },
            font_size: FontSize::WindowHeight(0.05),
            color,
        });
    }
}

fn build_settings_ui(app: &App, render_list: &mut Vec<Renderable>, gradient: Gradient) {
    render_list.push(Renderable::Background { gradient });
    render_list.push(Renderable::BigText {
        text: "Settings".to_string(),
        anchor: Anchor::TopCenter,
        shift: Shift { x: 0.0, y: 0.1 },
        align: Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Top },
        font_size: FontSize::WindowHeight(0.2),
        color: 0xFF_FFFFFF,
    });

    let fonts = [
        (FontChoice::YujiSyuku, "Yuji Syuku"),
        (FontChoice::NotoSerifJP, "Noto Serif JP"),
    ];

    for (i, (font_choice, name)) in fonts.iter().enumerate() {
        let is_selected = i == app.selected_settings_item;
        let is_active = *font_choice == app.font_choice;

        let mut display_text = if is_selected {
            format!("> {}", name)
        } else {
            format!("  {}", name)
        };
        
        if is_active {
            display_text.push_str(" *");
        }

        let color = if is_selected { 0xFF_FFFF00 } else { 0xFF_FFFFFF };

        render_list.push(Renderable::Text {
            text: display_text,
            anchor: Anchor::Center,
            shift: Shift { x: 0.0, y: 0.0 + (i as f32 * 0.1) },
            align: Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Center },
            font_size: FontSize::WindowHeight(0.05),
            color,
        });
    }
}

fn build_problem_selection_ui(app: &App, render_list: &mut Vec<Renderable>, gradient: Gradient) {
    render_list.push(Renderable::Background { gradient });
    render_list.push(Renderable::BigText {
        text: "Select Problem".to_string(),
        anchor: Anchor::TopCenter,
        shift: Shift { x: 0.0, y: 0.1 },
        align: Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Top },
        font_size: FontSize::WindowHeight(0.2),
        color: 0xFF_FFFFFF,
    });

    let item_height: f32 = 0.06;
    let list_y_start: f32 = 0.4;
    let list_height: f32 = 0.6;
    let items_per_screen = (list_height / item_height).floor() as usize;

    let mut start_index = 0;
    if app.selected_problem_item >= items_per_screen {
        start_index = app.selected_problem_item - items_per_screen + 1;
    }
    let end_index = (start_index + items_per_screen).min(app.problem_count());

    for i in start_index..end_index {
        let item = app.problem_name_at(i);
        let is_open_file = app.is_open_file_entry(i);
        // ソース種別バッジを付与: [B]=builtin, [W]=web(wasm), [F]=file(desktop), [+]=open-file
        let badge = if is_open_file { "+".to_string() } else { app.problem_source_label(i).to_string() };
        let selected = i == app.selected_problem_item;
        let (text, color) = if selected {
            (format!(">[{}] {}", badge, item), 0xFF_FFFF00u32)
        } else if is_open_file {
            (format!(" [{}] {}", badge, item), 0xFF_888888u32)
        } else {
            (format!(" [{}] {}", badge, item), 0xFF_FFFFFF)
        };
        let y_pos = list_y_start + ((i - start_index) as f32 * item_height);

        render_list.push(Renderable::Text {
            text,
            anchor: Anchor::TopCenter,
            shift: Shift { x: -0.2, y: y_pos },
            align: Align { horizontal: HorizontalAlign::Left, vertical: VerticalAlign::Top },
            font_size: FontSize::WindowHeight(0.045),
            color,
        });
    }

    if start_index > 0 {
        render_list.push(Renderable::Text { text: "▲".to_string(), anchor: Anchor::TopCenter, shift: Shift { x: 0.0, y: list_y_start - item_height },
            align: Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Center }, font_size: FontSize::WindowHeight(0.04), color: 0xFF_AAAAAA });
    }
    if end_index < app.problem_count() {
        render_list.push(Renderable::Text { text: "▼".to_string(), anchor: Anchor::TopCenter, shift: Shift { x: 0.0, y: list_y_start + list_height },
            align: Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Center }, font_size: FontSize::WindowHeight(0.04), color: 0xFF_AAAAAA });
    }
}

/// 問題ファイルのソースコードを閲覧するシーンを描画する
fn build_problem_source_ui(app: &App, render_list: &mut Vec<Renderable>, gradient: Gradient) {
    render_list.push(Renderable::Background { gradient });

    let idx = app.selected_problem_item;
    let label = app.problem_source_label(idx);
    let name = app.problem_name_at(idx);

    // ヘッダー: "[種別] 問題名"
    render_list.push(Renderable::BigText {
        text: format!("[{}] {}", label, name),
        anchor: Anchor::TopCenter,
        shift: Shift { x: 0.0, y: 0.05 },
        align: Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Top },
        font_size: FontSize::WindowHeight(0.09),
        color: 0xFF_AADDFF,
    });

    // ソースコンテンツ（1行ずつ描画）
    let line_h: f32 = 0.046;
    let content_y: f32 = 0.21;
    // 下部の status_text / instructions_text 領域を避けるため 0.12 を余白として確保
    let max_lines = ((1.0f32 - content_y - 0.12) / line_h).floor() as usize;

    if let Some(content) = app.get_problem_source(idx) {
        let total_lines = content.lines().count();

        for (i, line) in content.lines().skip(app.source_scroll).take(max_lines).enumerate() {
            // 長い行は60文字で切り詰め（バイト境界ではなく文字境界で）
            let ch_count = line.chars().count();
            let display = if ch_count > 60 {
                let truncated: String = line.chars().take(60).collect();
                format!("{}…", truncated)
            } else {
                line.to_string()
            };

            render_list.push(Renderable::Text {
                text: display,
                anchor: Anchor::TopCenter,
                shift: Shift { x: -0.46, y: content_y + i as f32 * line_h },
                align: Align { horizontal: HorizontalAlign::Left, vertical: VerticalAlign::Top },
                font_size: FontSize::WindowHeight(0.033),
                color: 0xFF_99DDAA,
            });
        }

        // スクロール位置インジケータ (右上)
        let scroll_text = if total_lines == 0 {
            "0/0".to_string()
        } else {
            format!("{}/{}", app.source_scroll + 1, total_lines)
        };
        render_list.push(Renderable::Text {
            text: scroll_text,
            anchor: Anchor::TopRight,
            shift: Shift { x: -0.01, y: 0.14 },
            align: Align { horizontal: HorizontalAlign::Right, vertical: VerticalAlign::Top },
            font_size: FontSize::WindowHeight(0.035),
            color: 0xFF_666688,
        });

        // 上下スクロール可能であることを示す矢印
        if app.source_scroll > 0 {
            render_list.push(Renderable::Text {
                text: "▲".to_string(),
                anchor: Anchor::TopCenter,
                shift: Shift { x: 0.45, y: content_y - line_h },
                align: Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Top },
                font_size: FontSize::WindowHeight(0.035),
                color: 0xFF_888888,
            });
        }
        if app.source_scroll + max_lines < total_lines {
            render_list.push(Renderable::Text {
                text: "▼".to_string(),
                anchor: Anchor::TopCenter,
                shift: Shift { x: 0.45, y: content_y + max_lines as f32 * line_h },
                align: Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Top },
                font_size: FontSize::WindowHeight(0.035),
                color: 0xFF_888888,
            });
        }
    }
}

/// How to Use 画面を描画する。
/// コンテンツは HOW_TO_USE_CONTENT 定数で定義し、スクロールに対応する。
fn build_how_to_use_ui(app: &App, render_list: &mut Vec<Renderable>, gradient: Gradient) {
    render_list.push(Renderable::Background { gradient });

    render_list.push(Renderable::BigText {
        text: "How to Use".to_string(),
        anchor: Anchor::TopCenter,
        shift: Shift { x: 0.0, y: 0.05 },
        align: Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Top },
        font_size: FontSize::WindowHeight(0.09),
        color: 0xFF_AADDFF,
    });

    // ProblemSource と同じレイアウト定数を使用
    let line_h: f32 = 0.046;
    let content_y: f32 = 0.21;
    // 下部の status_text / instructions_text 領域 (0.12) を除いた最大表示行数
    let max_lines = ((1.0f32 - content_y - 0.12) / line_h).floor() as usize;

    let total_lines = HOW_TO_USE_CONTENT.len();
    let scroll = app.how_to_use_scroll;

    for (i, (text, color)) in HOW_TO_USE_CONTENT
        .iter()
        .skip(scroll)
        .take(max_lines)
        .enumerate()
    {
        render_list.push(Renderable::Text {
            text: text.to_string(),
            anchor: Anchor::TopCenter,
            shift: Shift { x: -0.46, y: content_y + i as f32 * line_h },
            align: Align { horizontal: HorizontalAlign::Left, vertical: VerticalAlign::Top },
            font_size: FontSize::WindowHeight(0.033),
            color: *color,
        });
    }

    // スクロール位置インジケータ (右上)
    let scroll_text = format!("{}/{}", scroll + 1, total_lines.max(1));
    render_list.push(Renderable::Text {
        text: scroll_text,
        anchor: Anchor::TopRight,
        shift: Shift { x: -0.01, y: 0.14 },
        align: Align { horizontal: HorizontalAlign::Right, vertical: VerticalAlign::Top },
        font_size: FontSize::WindowHeight(0.035),
        color: 0xFF_666688,
    });

    // 上スクロール可能な場合 ▲ を表示
    if scroll > 0 {
        render_list.push(Renderable::Text {
            text: "▲".to_string(),
            anchor: Anchor::TopCenter,
            shift: Shift { x: 0.45, y: content_y - line_h },
            align: Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Top },
            font_size: FontSize::WindowHeight(0.035),
            color: 0xFF_888888,
        });
    }

    // 下スクロール可能な場合 ▼ を表示
    if scroll + max_lines < total_lines {
        render_list.push(Renderable::Text {
            text: "▼".to_string(),
            anchor: Anchor::TopCenter,
            shift: Shift { x: 0.45, y: content_y + max_lines as f32 * line_h },
            align: Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Top },
            font_size: FontSize::WindowHeight(0.035),
            color: 0xFF_888888,
        });
    }
}

// Segment の base テキストを文字列として返す（Anno は inner の base を連結）
fn segment_base_text(seg: &Segment) -> String {
    match seg {
        Segment::Plain { text } => text.clone(),
        Segment::Annotated { base, .. } => base.clone(),
        Segment::Anno { inner, .. } => inner.iter().map(|s| segment_base_text(s)).collect(),
    }
}

// Segment の reading テキストを文字列として返す（Anno は inner の reading を連結）
fn segment_reading_text(seg: &Segment) -> String {
    match seg {
        Segment::Plain { text } => text.clone(),
        Segment::Annotated { reading, .. } => reading.clone(),
        Segment::Anno { inner, .. } => inner.iter().map(|s| segment_reading_text(s)).collect(),
    }
}

// 表示用の (base_text, ruby_text, anno_text) を返す
fn segment_display_parts(seg: &Segment) -> (String, Option<String>, Option<String>) {
    match seg {
        Segment::Plain { text } => (text.clone(), None, None),
        Segment::Annotated { base, reading } => (base.clone(), Some(reading.clone()), None),
        Segment::Anno { inner, annotation } => {
            let base = inner.iter().map(|s| segment_base_text(s)).collect::<String>();
            let reading = inner.iter().map(|s| segment_reading_text(s)).collect::<String>();
            let ruby = if reading.is_empty() { None } else { Some(reading) };
            (base, ruby, Some(annotation.clone()))
        }
    }
}

fn is_word_correct(word: &TypingCorrectnessWord) -> bool {
    word.segments.iter().all(is_segment_correct)
}

fn is_segment_correct(segment: &TypingCorrectnessSegment) -> bool {
    !segment.chars.iter().any(|c| *c == TypingCorrectnessChar::Incorrect)
}

fn build_typing_ui(app: &App, render_list: &mut Vec<Renderable>, gradient: Gradient, font: &FontVec, width: usize, height: usize) {
    render_list.push(Renderable::Background { gradient });

    if let Some(model) = &app.typing_model {
        // --- レイアウト調整値 ---
        let v_offset = 0.08; // UI全体を下にずらす割合

        // --- 問題タイトル表示 ---
        render_list.push(Renderable::BigText {
            text: model.content.title.to_string(),
            anchor: Anchor::TopCenter,
            shift: Shift { x: 0.0, y: 0.01 },
            align: Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Top },
            font_size: FontSize::WindowHeight(0.12),
            color: ACTIVE_COLOR,
        });

        let line_idx = model.status.line as usize;
        let content_line = if let Some(line) = model.content.lines.get(line_idx) { line } else { return; };
        let correctness_line = if let Some(line) = model.typing_correctness.lines.get(line_idx) { line } else { return; };
        let status = &model.status;
        let scroll_offset = model.scroll.scroll as f32;
        
        let base_font_size = FontSize::WindowHeight(BASE_FONT_SIZE_RATIO);
        let base_pixel_font_size = calculate_pixel_font_size(base_font_size, width, height);
        
        // スクロールキャッシュに total_width が既にあればそれを使い、毎フレームの
        // 全セグメント measure_text を回避する。app::update() がこのフレームより先に
        // キャッシュを更新しているため、タイピング中は常にキャッシュが有効である。
        let target_line_total_width = app.scroll_cache
            .as_ref()
            .filter(|c: &&ScrollCache| c.line == model.status.line)
            .map(|c| c.total_width as u32)
            .unwrap_or_else(|| {
                content_line.words.iter().flat_map(|w| &w.segments).map(|seg| {
                    let text = segment_base_text(seg);
                    gui_renderer::measure_text(font, &text, base_pixel_font_size).0
                }).sum::<u32>()
            });

        // --- 上段（目標テキスト）の構築 ---
        let mut upper_segments = Vec::new();
        for (word_idx, word) in content_line.words.iter().enumerate() {
            for (seg_idx, seg) in word.segments.iter().enumerate() {
                let state = if (word_idx as i32) < status.word {
                    if is_word_correct(&correctness_line.words[word_idx]) {
                        UpperSegmentState::Correct
                    } else {
                        UpperSegmentState::Incorrect
                    }
                } else if (word_idx as i32) == status.word {
                    if (seg_idx as i32) < status.segment {
                        // このワード内の完了済みセグメント
                        if correctness_line.words.get(word_idx)
                            .and_then(|w| w.segments.get(seg_idx))
                            .map_or(false, is_segment_correct)
                        {
                            UpperSegmentState::Correct
                        } else {
                            UpperSegmentState::Incorrect
                        }
                    } else if (seg_idx as i32) == status.segment {
                        UpperSegmentState::Active
                    } else {
                        UpperSegmentState::Pending
                    }
                } else {
                    UpperSegmentState::Pending
                };

                let (base_text, ruby_text, anno_text) = segment_display_parts(seg);
                upper_segments.push(UpperTypingSegment { base_text, ruby_text, anno_text, state });
            }
        }
        
        let upper_y_shift_from_center = -(base_pixel_font_size * UPPER_ROW_Y_OFFSET_FACTOR) / height as f32 + 0.17;
        render_list.push(Renderable::TypingUpper {
            segments: upper_segments,
            anchor: Anchor::Center,
            shift: Shift { x: -scroll_offset / width as f32, y: upper_y_shift_from_center },
            align: Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Center },
            font_size: base_font_size,
        });

        // --- 下段（入力テキスト）の構築 ---
        let mut lower_segments = Vec::new();
        for word_idx in 0..(status.word as usize) {
            let word = &content_line.words[word_idx];
            let correctness_word = &correctness_line.words[word_idx];
            for seg in &word.segments {
                let (base_text, ruby_text, _) = segment_display_parts(seg);
                lower_segments.push(LowerTypingSegment::Completed {
                    base_text,
                    ruby_text,
                    is_correct: is_word_correct(correctness_word),
                });
            }
        }

        if let Some(active_word_content) = content_line.words.get(status.word as usize) {
            let active_correctness_word = &correctness_line.words[status.word as usize];
            for seg_idx in 0..(status.segment as usize) {
                let seg = &active_word_content.segments[seg_idx];
                let (base_text, ruby_text, _) = segment_display_parts(seg);
                lower_segments.push(LowerTypingSegment::Completed {
                    base_text,
                    ruby_text,
                    is_correct: is_segment_correct(&active_correctness_word.segments[seg_idx]),
                });
            }

            if let Some(active_seg_content) = active_word_content.segments.get(status.segment as usize) {
                let reading_text = segment_reading_text(active_seg_content);
                let mut active_elements = Vec::new();
                
                let correctness_seg = &active_correctness_word.segments[status.segment as usize];
                for (char_idx, character) in reading_text.chars().enumerate().take(status.char_ as usize) {
                    let is_correct = correctness_seg.chars[char_idx] != TypingCorrectnessChar::Incorrect;
                    active_elements.push(ActiveLowerElement::Typed { character, is_correct });
                }
                
                if let Some(wrong_char) = status.last_wrong_keydown {
                    active_elements.push(ActiveLowerElement::LastIncorrectInput(wrong_char));
                } else {
                    if !status.unconfirmed.is_empty() {
                        let unconfirmed_text: String = status.unconfirmed.iter().collect();
                        active_elements.push(ActiveLowerElement::UnconfirmedInput(unconfirmed_text));
                    }
                    active_elements.push(ActiveLowerElement::Cursor);
                }

                lower_segments.push(LowerTypingSegment::Active { elements: active_elements });
            }
        }
        
        let lower_y_shift_from_center = (base_pixel_font_size * LOWER_ROW_Y_OFFSET_FACTOR) / height as f32 + 0.01;
        render_list.push(Renderable::TypingLower {
            segments: lower_segments,
            anchor: Anchor::Center,
            shift: Shift { x: -scroll_offset / width as f32, y: lower_y_shift_from_center },
            align: Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Top },
            font_size: base_font_size,
            target_line_total_width,
        });

        // --- コンテキスト行（前後の行）を描画 ---
        let line_count = model.content.lines.len();
        for &offset in &[-1, 1] {
            let line_to_display_signed = model.status.line + offset;
            if line_to_display_signed >= 0 && (line_to_display_signed as usize) < line_count {
                let line_idx_context = line_to_display_signed as usize;
                render_list.push(Renderable::Text {
                    text: model.content.lines[line_idx_context].to_string(),
                    anchor: Anchor::Center,
                    shift: Shift { x: 0.0, y: (offset as f32 * 0.37) + 0.05 },
                    align: Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Center },
                    font_size: FontSize::WindowHeight(0.08),
                    color: 0xFF_444444,
                });
            }
        }
        
        // --- ステータスパネル ---
        let metrics = typing::calculate_total_metrics(model);
        let time = metrics.total_time / 1000.0;
        let status_items = vec![
            format!("Progress: {} / {}", model.status.line as usize + 1, line_count),
            format!("Speed: {:.2} KPS", metrics.speed),
            format!("Accuracy: {:.1}%", metrics.accuracy * 100.0),
            format!("Misses: {}", metrics.miss_count),
            format!("Time: {:02.0}:{:05.2}", (time / 60.0).floor(), time % 60.0),
        ];
        
        let progress_bar_height_ratio = 0.02;
        let status_item_height_ratio = 0.04;

        for (i, item) in status_items.iter().enumerate() {
            render_list.push(Renderable::Text {
                text: item.clone(),
                anchor: Anchor::BottomLeft,
                shift: Shift { x: 0.02, y: -0.02 - progress_bar_height_ratio - ((status_items.len() - 1 - i) as f32 * status_item_height_ratio)},
                align: Align {horizontal: HorizontalAlign::Left, vertical: VerticalAlign::Bottom },
                font_size: FontSize::WindowHeight(status_item_height_ratio),
                color: 0xFF_DDDDDD,
            });
        }

        // --- 進捗バー ---
        let char_progress_in_line = model.status.word as f32 / content_line.words.len().max(1) as f32;
        let detailed_progress_ratio = if line_count > 0 {
            (model.status.line as f32 + char_progress_in_line) / (line_count as f32)
        } else {
            0.0
        };

        render_list.push(Renderable::ProgressBar {
            anchor: Anchor::BottomLeft,
            shift: Shift { x: 0.0, y: -0.005 }, // 少しだけ底から浮かせる
            width_ratio: 1.0,
            height_ratio: progress_bar_height_ratio,
            progress: detailed_progress_ratio,
            bg_color: 0xFF_555555,
            fg_color: CORRECT_COLOR,
        });
    }
}

fn build_result_ui(app: &App, render_list: &mut Vec<Renderable>, gradient: Gradient) {
    render_list.push(Renderable::Background { gradient });
    render_list.push(Renderable::BigText {
        text: "Result".to_string(),
        anchor: Anchor::Center,
        shift: Shift { x: 0.0, y: -0.3 },
        align: Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Center },
        font_size: FontSize::WindowHeight(0.15),
        color: 0xFF_FFFF00,
    });

    if let Some(result) = &app.result_model {
        let metrics = crate::typing::calculate_total_metrics(&result.typing_model);
        let result_texts = vec![
            format!("Typed Chars: {}", metrics.type_count),
            format!("Misses: {}", metrics.miss_count),
            format!("Time: {:.2}s", metrics.total_time / 1000.0),
            format!("Accuracy: {:.2}%", metrics.accuracy * 100.0),
            format!("Speed: {:.2} chars/sec", metrics.speed),
        ];

        for (i, text) in result_texts.iter().enumerate() {
            render_list.push(Renderable::Text {
                text: text.clone(),
                anchor: Anchor::Center,
                shift: Shift { x: 0.0, y: -0.1 + (i as f32 * 0.08) },
                align: Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Center },
                font_size: FontSize::WindowHeight(0.05),
                color: 0xFF_FFFFFF,
            });
        }
    }
}


/// AnchorとShiftから、基準となる座標(x, y)を計算する
pub fn calculate_anchor_position(
    anchor: Anchor, shift: Shift, width: usize, height: usize) -> (i32, i32) {
    let (w, h) = (width as i32, height as i32);
    let base_pos = match anchor {
        Anchor::TopLeft => (0, 0),
        Anchor::TopCenter => (w / 2, 0),
        Anchor::TopRight => (w, 0),
        Anchor::CenterLeft => (0, h / 2),
        Anchor::Center => (w / 2, h / 2),
        Anchor::CenterRight => (w, h / 2),
        Anchor::BottomLeft => (0, h),
        Anchor::BottomCenter => (w / 2, h),
        Anchor::BottomRight => (w, h),
    };
    let shift_x = (width as f32 * shift.x) as i32;
    let shift_y = (height as f32 * shift.y) as i32;
    (base_pos.0 + shift_x, base_pos.1 + shift_y)
}

/// 基準点、テキストの寸法、揃え方から、最終的な描画開始座標（左上）を計算する
pub fn calculate_aligned_position(
    anchor_pos: (i32, i32), text_width: u32, text_height: u32, align: Align) -> (i32, i32) {
    let (tw, th) = (text_width as i32, text_height as i32);
    let (ax, ay) = anchor_pos;

    let x = match align.horizontal {
        HorizontalAlign::Left => ax,
        HorizontalAlign::Center => ax - tw / 2,
        HorizontalAlign::Right => ax - tw,
    };

    let y = match align.vertical {
        VerticalAlign::Top => ay,
        VerticalAlign::Center => ay - th / 2,
        VerticalAlign::Bottom => ay - th,
    };

    (x, y)
}
