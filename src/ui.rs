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

use crate::app::{App, AppState};
use crate::model::{Segment, TypingCorrectnessChar, TypingCorrectnessSegment, TypingModel};
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

/// 画面に描画すべき要素の種類とレイアウト情報を定義するenum
pub enum Renderable {
    Background { gradient: Gradient },
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
    TypingBase {
        text: String,
        anchor: Anchor,
        shift: Shift,
        align: Align,
        font_size: FontSize,
        color: u32,
    },
    TypingRuby {
        text: String,
        anchor: Anchor,
        shift: Shift,
        align: Align,
        font_size: FontSize,
        color: u32,
    },
}

#[cfg(target_arch = "wasm32")]
const MENU_ITEMS: [&str; 1] = ["Start Typing"];

#[cfg(not(target_arch = "wasm32"))]
const MENU_ITEMS: [&str; 2] = ["Start Typing", "Quit"];

// --- タイピング画面のレイアウト定数 ---
pub const BASE_FONT_SIZE_RATIO: f32 = 0.3;
const UPPER_ROW_Y_OFFSET_FACTOR: f32 = 1.0;
const LOWER_ROW_Y_OFFSET_FACTOR: f32 = 0.2;
const RUBY_Y_OFFSET_FACTOR: f32 = 0.3;

// --- 色定義 ---
const CORRECT_COLOR: u32 = 0xFF_9097FF;
const INCORRECT_COLOR: u32 = 0xFF_FF9898;
const PENDING_COLOR: u32 = 0xFF_999999;
const WRONG_KEY_COLOR: u32 = 0xFF_F55252;
const CURSOR_COLOR: u32 = 0xFF_FFFFFF;


/// Appの状態を受け取り、描画リスト（UIレイアウト）を構築する
pub fn build_ui(app: &App, font: &FontRef, width: usize, height: usize) -> Vec<Renderable> {
    let mut render_list = Vec::new();

    let menu_gradient = Gradient { start_color: 0xFF_000010, end_color: 0xFF_000000 };
    let typing_gradient = Gradient { start_color: 0xFF_100010, end_color: 0xFF_000000 };
    let result_gradient = Gradient { start_color: 0xFF_101000, end_color: 0xFF_000000 };

    match app.state {
        AppState::MainMenu => build_main_menu_ui(app, &mut render_list, menu_gradient),
        AppState::Typing => build_typing_ui(app, &mut render_list, typing_gradient, font, width, height),
        AppState::ProblemSelection => build_problem_selection_ui(app, &mut render_list, menu_gradient),
        AppState::Result => build_result_ui(app, &mut render_list, result_gradient),
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
        render_list.push(Renderable::Text {
            text: app.instructions_text.clone(),
            anchor: Anchor::BottomRight,
            shift: Shift { x: -0.01, y: -0.02 },
            align: Align { horizontal: HorizontalAlign::Right, vertical: VerticalAlign::Bottom },
            font_size: FontSize::WindowHeight(0.04),
            color: 0xFF_CCCCCC,
        });
    }

    render_list
}

fn build_main_menu_ui(app: &App, render_list: &mut Vec<Renderable>, gradient: Gradient) {
    render_list.push(Renderable::Background { gradient });
    render_list.push(Renderable::BigText {
        text: "Neknaj Typing MP".to_string(),
        anchor: Anchor::Center,
        shift: Shift { x: 0.0, y: -0.3 },
        align: Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Center },
        font_size: FontSize::WindowHeight(0.2),
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
            shift: Shift { x: 0.0, y: -0.1 + (i as f32 * 0.1) },
            align: Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Center },
            font_size: FontSize::WindowHeight(0.05),
            color,
        });
    }
}

fn build_problem_selection_ui(app: &App, render_list: &mut Vec<Renderable>, gradient: Gradient) {
    render_list.push(Renderable::Background { gradient });
    render_list.push(Renderable::Text {
        text: "Select Problem".to_string(),
        anchor: Anchor::TopCenter,
        shift: Shift { x: 0.0, y: 0.1 },
        align: Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Center },
        font_size: FontSize::WindowHeight(0.1),
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

/// セグメントが正しくタイプされたかをチェックするヘルパー関数
fn is_segment_correct(segment: &TypingCorrectnessSegment) -> bool {
    !segment.chars.iter().any(|c| *c == TypingCorrectnessChar::Incorrect)
}

fn build_typing_ui(app: &App, render_list: &mut Vec<Renderable>, gradient: Gradient, font: &FontRef, width: usize, height: usize) {
    render_list.push(Renderable::Background { gradient });

    if let Some(model) = &app.typing_model {
        // --- コンテキスト行（前後の行）を描画 ---
        let current_line_signed = model.status.line;
        let line_count = model.content.lines.len();
        for &offset in &[-1, 1] {
            let line_to_display_signed = current_line_signed + offset;
            if line_to_display_signed >= 0 && (line_to_display_signed as usize) < line_count {
                let line_idx = line_to_display_signed as usize;
                render_list.push(Renderable::Text {
                    text: model.content.lines[line_idx].to_string(),
                    anchor: Anchor::Center,
                    shift: Shift { x: 0.0, y: offset as f32 * 0.35 },
                    align: Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Center },
                    font_size: FontSize::WindowHeight(0.05),
                    color: 0xFF_444444,
                });
            }
        }
        
        // --- メインのタイピング行を描画 ---
        let line_idx = model.status.line as usize;
        let content_line = if let Some(line) = model.content.lines.get(line_idx) { line } else { return; };
        let correctness_line = if let Some(line) = model.typing_correctness.lines.get(line_idx) { line } else { return; };
        let status = &model.status;
        let scroll_offset = model.scroll.scroll as f32;

        let base_font_size = FontSize::WindowHeight(BASE_FONT_SIZE_RATIO);
        let base_pixel_font_size = crate::renderer::calculate_pixel_font_size(base_font_size, width, height);
        let ruby_pixel_font_size = base_pixel_font_size * 0.4;
        let small_ruby_pixel_font_size = base_pixel_font_size * 0.3;

        let total_layout_width = content_line.segments.iter().map(|seg| {
            let text = match seg {
                Segment::Plain { text } => text.as_str(),
                Segment::Annotated { base, .. } => base.as_str(),
            };
            gui_renderer::measure_text(font, text, base_pixel_font_size).0 as f32
        }).sum::<f32>();

        let block_start_x = (width as f32 - total_layout_width) / 2.0 - scroll_offset;

        // --- 上段（目標テキスト） ---
        let upper_y = (height as f32 / 2.0) - base_pixel_font_size * UPPER_ROW_Y_OFFSET_FACTOR;
        let mut upper_pen_x = block_start_x;

        for (seg_idx, seg) in content_line.segments.iter().enumerate() {
            let is_typed_segment = seg_idx < status.segment as usize;
            let color = if is_typed_segment {
                if is_segment_correct(&correctness_line.segments[seg_idx]) { CORRECT_COLOR } else { INCORRECT_COLOR }
            } else {
                PENDING_COLOR
            };

            let base_text = match seg {
                Segment::Plain { text } => text,
                Segment::Annotated { base, .. } => base,
            };
            
            render_list.push(Renderable::TypingBase { text: base_text.to_string(), anchor: Anchor::TopLeft, shift: Shift {x: upper_pen_x / width as f32, y: upper_y / height as f32}, align: Align {horizontal: HorizontalAlign::Left, vertical: VerticalAlign::Top}, font_size: base_font_size, color });
            
            if let Segment::Annotated { base, reading, .. } = seg {
                let (base_w, ..) = gui_renderer::measure_text(font, base, base_pixel_font_size);
                let (ruby_w, ..) = gui_renderer::measure_text(font, reading, ruby_pixel_font_size);
                let ruby_x = upper_pen_x + (base_w as f32 - ruby_w as f32) / 2.0;
                let ruby_y = upper_y - ruby_pixel_font_size * RUBY_Y_OFFSET_FACTOR;
                render_list.push(Renderable::TypingRuby { text: reading.clone(), anchor: Anchor::TopLeft, shift: Shift {x: ruby_x / width as f32, y: ruby_y / height as f32}, align: Align {horizontal: HorizontalAlign::Left, vertical: VerticalAlign::Top}, font_size: FontSize::WindowHeight(BASE_FONT_SIZE_RATIO * 0.4), color });
            }

            let (base_w, ..) = gui_renderer::measure_text(font, base_text, base_pixel_font_size);
            upper_pen_x += base_w as f32;
        }

        // --- 下段（入力中テキスト） ---
        let lower_y = (height as f32 / 2.0) + base_pixel_font_size * LOWER_ROW_Y_OFFSET_FACTOR;
        let mut lower_pen_x = block_start_x;
        
        // 完了済みセグメント
        for seg_idx in 0..(status.segment as usize) {
            let seg = &content_line.segments[seg_idx];
            let color = if is_segment_correct(&correctness_line.segments[seg_idx]) { CORRECT_COLOR } else { INCORRECT_COLOR };
            let base_text = match seg {
                Segment::Plain { text } => text,
                Segment::Annotated { base, .. } => base,
            };
            render_list.push(Renderable::TypingBase { text: base_text.to_string(), anchor:Anchor::TopLeft, shift: Shift { x: lower_pen_x / width as f32, y: lower_y / height as f32 }, align: Align {horizontal: HorizontalAlign::Left, vertical: VerticalAlign::Top}, font_size: base_font_size, color });
            if let Segment::Annotated { base, reading, .. } = seg {
                let (base_w, ..) = gui_renderer::measure_text(font, base, base_pixel_font_size);
                let (small_reading_w, ..) = gui_renderer::measure_text(font, reading, small_ruby_pixel_font_size);
                let ruby_x = lower_pen_x + (base_w as f32 - small_reading_w as f32) / 2.0;
                let ruby_y = lower_y - small_ruby_pixel_font_size * RUBY_Y_OFFSET_FACTOR;
                render_list.push(Renderable::TypingRuby { text: reading.clone(), anchor: Anchor::TopLeft, shift: Shift { x: ruby_x / width as f32, y: ruby_y / height as f32 }, align: Align {horizontal: HorizontalAlign::Left, vertical: VerticalAlign::Top}, font_size: FontSize::WindowHeight(BASE_FONT_SIZE_RATIO * 0.3), color });
            }
            let (base_w, ..) = gui_renderer::measure_text(font, base_text, base_pixel_font_size);
            lower_pen_x += base_w as f32;
        }

        // 入力中セグメント
        if let Some(seg) = content_line.segments.get(status.segment as usize) {
            let reading_text = match seg {
                Segment::Plain { text } => text.as_str(),
                Segment::Annotated { base: _, reading } => reading.as_str(),
            };

            // 入力済みの部分を色付きで描画
            let mut reading_width_before: u32 = 0;
            for (char_idx, character) in reading_text.chars().enumerate().take(status.char_ as usize) {
                let char_str = character.to_string();
                let color = match correctness_line.segments[status.segment as usize].chars[char_idx] {
                    TypingCorrectnessChar::Correct => CORRECT_COLOR,
                    _ => INCORRECT_COLOR,
                };
                let reading_part_up_to_char = reading_text.chars().take(char_idx + 1).collect::<String>();
                let (reading_width_up_to_char, ..) = gui_renderer::measure_text(font, &reading_part_up_to_char, base_pixel_font_size);
                let char_advance_width = (reading_width_up_to_char - reading_width_before) as f32;
                
                // BigTextを使用して、TUIでAA化されないようにする
                render_list.push(Renderable::BigText {
                    text: char_str.clone(),
                    anchor: Anchor::TopLeft,
                    shift: Shift { x: lower_pen_x / width as f32, y: lower_y / height as f32 },
                    align: Align { horizontal: HorizontalAlign::Left, vertical: VerticalAlign::Top },
                    font_size: base_font_size,
                    color,
                });

                lower_pen_x += char_advance_width;
                reading_width_before = reading_width_up_to_char;
            }
        }
        
        // カーソルと未確定文字
        let cursor_y = lower_y;
        render_list.push(Renderable::BigText {text: "|".to_string(), anchor: Anchor::TopLeft, shift: Shift {x: lower_pen_x / width as f32, y: cursor_y / height as f32}, align: Align {horizontal: HorizontalAlign::Left, vertical: VerticalAlign::Top}, font_size: base_font_size, color: CURSOR_COLOR});
        
        let extras_x = lower_pen_x + gui_renderer::measure_text(font, "|", base_pixel_font_size).0 as f32 * 0.5;
        if !status.unconfirmed.is_empty() {
            let unconfirmed_text: String = status.unconfirmed.iter().collect();
            render_list.push(Renderable::Text {text: unconfirmed_text, anchor: Anchor::TopLeft, shift: Shift {x: extras_x / width as f32, y: lower_y / height as f32}, align: Align {horizontal: HorizontalAlign::Left, vertical: VerticalAlign::Top}, font_size: base_font_size, color: PENDING_COLOR});
        } else if let Some(wrong_char) = status.last_wrong_keydown {
            let wrong_text = wrong_char.to_string();
            render_list.push(Renderable::Text {text: wrong_text, anchor: Anchor::TopLeft, shift: Shift {x: extras_x / width as f32, y: lower_y / height as f32}, align: Align {horizontal: HorizontalAlign::Left, vertical: VerticalAlign::Top}, font_size: base_font_size, color: WRONG_KEY_COLOR});
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