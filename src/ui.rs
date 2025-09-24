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

use crate::app::{App, AppState, FontChoice};
use crate::model::{Segment, TypingCorrectnessChar, TypingCorrectnessSegment, TypingCorrectnessWord};
use crate::renderer::gui_renderer;
use crate::typing; // For calculate_total_metrics
use ab_glyph::FontRef; // FontRefを渡すために必要

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
}

#[cfg(target_arch = "wasm32")]
const MENU_ITEMS: [&str; 2] = ["Start Typing", "Settings"];

#[cfg(not(target_arch = "wasm32"))]
const MENU_ITEMS: [&str; 3] = ["Start Typing", "Settings", "Quit"];

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
pub fn build_ui<'a>(app: &App<'a>, font: &FontRef<'a>, width: usize, height: usize) -> Vec<Renderable> {
    let mut render_list = Vec::new();

    let menu_gradient = Gradient { start_color: 0xFF_000010, end_color: 0xFF_000000 };
    let typing_gradient = Gradient { start_color: 0xFF_100010, end_color: 0xFF_000000 };
    let result_gradient = Gradient { start_color: 0xFF_101000, end_color: 0xFF_000000 };
    let settings_gradient = Gradient { start_color: 0xFF_001010, end_color: 0xFF_000000 };

    match app.state {
        AppState::MainMenu => build_main_menu_ui(app, &mut render_list, menu_gradient),
        AppState::Typing => build_typing_ui(app, &mut render_list, typing_gradient, font, width, height),
        AppState::ProblemSelection => build_problem_selection_ui(app, &mut render_list, menu_gradient),
        AppState::Result => build_result_ui(app, &mut render_list, result_gradient),
        AppState::Settings => build_settings_ui(app, &mut render_list, settings_gradient),
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
        shift: Shift { x: -0.01, y: -0.02 },
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
    let list_y_start: f32 = 0.25;
    let list_height: f32 = 0.6;
    let items_per_screen = (list_height / item_height).floor() as usize;

    let mut start_index = 0;
    if app.selected_problem_item >= items_per_screen {
        start_index = app.selected_problem_item - items_per_screen + 1;
    }
    let end_index = (start_index + items_per_screen).min(app.problem_list.len());

    for i in start_index..end_index {
        let item = app.problem_list[i];
        let (text, color) = if i == app.selected_problem_item {
            (format!("> {}", item), 0xFF_FFFF00)
        } else {
            (format!("  {}", item), 0xFF_FFFFFF)
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
    if end_index < app.problem_list.len() {
        render_list.push(Renderable::Text { text: "▼".to_string(), anchor: Anchor::TopCenter, shift: Shift { x: 0.0, y: list_y_start + list_height },
            align: Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Center }, font_size: FontSize::WindowHeight(0.04), color: 0xFF_AAAAAA });
    }
}

fn is_word_correct(word: &TypingCorrectnessWord) -> bool {
    word.segments.iter().all(is_segment_correct)
}

fn is_segment_correct(segment: &TypingCorrectnessSegment) -> bool {
    !segment.chars.iter().any(|c| *c == TypingCorrectnessChar::Incorrect)
}

fn build_typing_ui<'a>(app: &App<'a>, render_list: &mut Vec<Renderable>, gradient: Gradient, font: &FontRef<'a>, width: usize, height: usize) {
    render_list.push(Renderable::Background { gradient });

    if let Some(model) = &app.typing_model {
        let line_idx = model.status.line as usize;
        let content_line = if let Some(line) = model.content.lines.get(line_idx) { line } else { return; };
        let correctness_line = if let Some(line) = model.typing_correctness.lines.get(line_idx) { line } else { return; };
        let status = &model.status;
        let scroll_offset = model.scroll.scroll as f32;
        
        let base_font_size = FontSize::WindowHeight(BASE_FONT_SIZE_RATIO);
        let base_pixel_font_size = crate::renderer::calculate_pixel_font_size(base_font_size, width, height);
        
        let target_line_total_width = content_line.words.iter().flat_map(|w| &w.segments).map(|seg| {
            let text = match seg {
                Segment::Plain { text } => text.as_str(),
                Segment::Annotated { base, .. } => base.as_str(),
            };
            gui_renderer::measure_text(font, text, base_pixel_font_size).0
        }).sum::<u32>();

        // --- 上段（目標テキスト）の構築 ---
        let mut upper_segments = Vec::new();
        for (word_idx, word) in content_line.words.iter().enumerate() {
            for (seg_idx, seg) in word.segments.iter().enumerate() {
                // セグメントの状態（色）を、単語単位の状態で決定する
                let state = if (word_idx as i32) < status.word {
                    // 完了した単語は、その単語全体の正誤に基づいてハイライト
                    if is_word_correct(&correctness_line.words[word_idx]) {
                        UpperSegmentState::Correct
                    } else {
                        UpperSegmentState::Incorrect
                    }
                } else if (word_idx as i32) == status.word {
                    // 現在入力中の単語は、すべてのセグメントをアクティブとしてハイライト
                    UpperSegmentState::Active
                } else {
                    // これから入力する単語はペンディング
                    UpperSegmentState::Pending
                };

                let (base_text, ruby_text) = match seg {
                    Segment::Plain { text } => (text.clone(), None),
                    Segment::Annotated { base, reading } => (base.clone(), Some(reading.clone())),
                };
                
                upper_segments.push(UpperTypingSegment { base_text, ruby_text, state });
            }
        }
        
        let upper_y = (height as f32 / 2.0) - base_pixel_font_size * UPPER_ROW_Y_OFFSET_FACTOR;
        render_list.push(Renderable::TypingUpper {
            segments: upper_segments,
            anchor: Anchor::TopCenter,
            shift: Shift { x: -scroll_offset / width as f32, y: upper_y / height as f32 },
            align: Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Top },
            font_size: base_font_size,
        });

        // --- 下段（入力テキスト）の構築 ---
        let mut lower_segments = Vec::new();
        for word_idx in 0..(status.word as usize) {
            let word = &content_line.words[word_idx];
            let correctness_word = &correctness_line.words[word_idx];
            for seg in &word.segments {
                let (base_text, ruby_text) = match seg {
                    Segment::Plain { text } => (text.clone(), None),
                    Segment::Annotated { base, reading } => (base.clone(), Some(reading.clone())),
                };
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
                 let (base_text, ruby_text) = match seg {
                    Segment::Plain { text } => (text.clone(), None),
                    Segment::Annotated { base, reading } => (base.clone(), Some(reading.clone())),
                };
                lower_segments.push(LowerTypingSegment::Completed {
                    base_text,
                    ruby_text,
                    is_correct: is_segment_correct(&active_correctness_word.segments[seg_idx]),
                });
            }

            if let Some(active_seg_content) = active_word_content.segments.get(status.segment as usize) {
                let reading_text = match active_seg_content {
                    Segment::Plain { text } => text,
                    Segment::Annotated { reading, .. } => reading,
                };
                let mut active_elements = Vec::new();
                
                let correctness_seg = &active_correctness_word.segments[status.segment as usize];
                for (char_idx, character) in reading_text.chars().enumerate().take(status.char_ as usize) {
                    let is_correct = correctness_seg.chars[char_idx] != TypingCorrectnessChar::Incorrect;
                    active_elements.push(ActiveLowerElement::Typed { character, is_correct });
                }
                
                if let Some(wrong_char) = status.last_wrong_keydown {
                    active_elements.push(ActiveLowerElement::Cursor);
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
        
        let lower_y = (height as f32 / 2.0) + base_pixel_font_size * LOWER_ROW_Y_OFFSET_FACTOR;
        render_list.push(Renderable::TypingLower {
            segments: lower_segments,
            anchor: Anchor::TopCenter,
            shift: Shift { x: -scroll_offset / width as f32, y: lower_y / height as f32 },
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
                    shift: Shift { x: 0.0, y: offset as f32 * 0.45 },
                    align: Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Center },
                    font_size: FontSize::WindowHeight(0.05),
                    color: 0xFF_444444,
                });
            }
        }
        
        // --- ステータスパネル ---
        let metrics = typing::calculate_total_metrics(model);
        let time = metrics.total_time / 1000.0;
        let status_items = vec![
            format!("Speed: {:.2} KPS", metrics.speed),
            format!("Accuracy: {:.1}%", metrics.accuracy * 100.0),
            format!("Misses: {}", metrics.miss_count),
            format!("Time: {:02.0}:{:05.2}", (time / 60.0).floor(), time % 60.0),
        ];

        for (i, item) in status_items.iter().enumerate() {
            render_list.push(Renderable::Text {
                text: item.clone(),
                anchor: Anchor::BottomLeft,
                shift: Shift { x: 0.02, y: -0.16 + (i as f32 * 0.04)},
                align: Align {horizontal: HorizontalAlign::Left, vertical: VerticalAlign::Bottom },
                font_size: FontSize::WindowHeight(0.04),
                color: 0xFF_DDDDDD,
            });
        }
    }
}

fn build_result_ui(app: &App, render_list: &mut Vec<Renderable>, gradient: Gradient) {
    render_list.push(Renderable::Background { gradient });
    render_list.push(Renderable::Text {
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