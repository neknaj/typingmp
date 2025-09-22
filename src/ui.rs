// src/ui.rs

// uefi featureが有効な場合、標準のallocクレートをインポート
#[cfg(feature = "uefi")]
extern crate alloc;

// uefi と std で使用する Vec と vec! を切り替える
#[cfg(feature = "uefi")]
use alloc::vec::Vec;
#[cfg(not(feature = "uefi"))]
use std::vec::Vec;

// uefi と std で使用する String と format! を切り替える
#[cfg(feature = "uefi")]
use alloc::{format, string::{String, ToString}};
#[cfg(not(feature = "uefi"))]
use std::string::{String, ToString};

use crate::app::{App, AppState};
use crate::model::{Line, Segment, TypingCorrectnessChar, TypingCorrectnessLine, TypingStatus};
use crate::typing; // For calculate_total_metrics

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
    BigText { // BigText is kept for other potential uses (like result screen title)
        text: String,
        anchor: Anchor,
        shift: Shift,
        align: Align,
        font_size: FontSize,
        color: u32,
    },
    // A dedicated variant for the complex typing line
    TypingText {
        content_line: Line,
        correctness_line: TypingCorrectnessLine,
        status: TypingStatus,
        anchor: Anchor,
        shift: Shift,
        font_size: FontSize,
    }
}

#[cfg(target_arch = "wasm32")]
const MENU_ITEMS: [&str; 1] = ["Start Typing"];

#[cfg(not(target_arch = "wasm32"))]
const MENU_ITEMS: [&str; 2] = ["Start Typing", "Quit"];

/// Appの状態を受け取り、描画リスト（UIレイアウト）を構築する
pub fn build_ui(app: &App) -> Vec<Renderable> {
    let mut render_list = Vec::new();

    let menu_gradient = Gradient { start_color: 0xFF_000010, end_color: 0xFF_000000 };
    let typing_gradient = Gradient { start_color: 0xFF_100010, end_color: 0xFF_000000 };
    let result_gradient = Gradient { start_color: 0xFF_101000, end_color: 0xFF_000000 };

    match app.state {
        AppState::Menu => build_menu_ui(app, &mut render_list, menu_gradient),
        AppState::Typing => build_typing_ui(app, &mut render_list, typing_gradient),
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

fn build_menu_ui(app: &App, render_list: &mut Vec<Renderable>, gradient: Gradient) {
    render_list.push(Renderable::Background { gradient });
    render_list.push(Renderable::Text {
        text: "Neknaj Typing MP".to_string(),
        anchor: Anchor::Center,
        shift: Shift { x: 0.0, y: -0.3 },
        align: Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Center },
        font_size: FontSize::WindowHeight(0.1),
        color: 0xFF_FFFFFF,
    });
    for (i, item) in MENU_ITEMS.iter().enumerate() {
        let (text, color) = if i == app.selected_menu_item {
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

fn build_typing_ui(app: &App, render_list: &mut Vec<Renderable>, gradient: Gradient) {
    render_list.push(Renderable::Background { gradient });

    if let Some(model) = &app.typing_model {
        let current_line_signed = model.status.line;
        let line_count = model.content.lines.len();

        // --- Render Context Lines (Previous and Next) ---
        for &offset in &[-1, 1] { // Previous and Next lines
            let line_to_display_signed = current_line_signed + offset;
            if line_to_display_signed >= 0 && (line_to_display_signed as usize) < line_count {
                let line_idx = line_to_display_signed as usize;
                render_list.push(Renderable::Text {
                    text: model.content.lines[line_idx].to_string(),
                    anchor: Anchor::Center,
                    shift: Shift { x: 0.0, y: offset as f32 * 0.25 }, // Position above/below center
                    align: Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Center },
                    font_size: FontSize::WindowHeight(0.05),
                    color: 0xFF_444444,
                });
            }
        }
        
        // --- Create the main TypingText Renderable ---
        if (current_line_signed as usize) < line_count {
            let line_idx = current_line_signed as usize;
            render_list.push(Renderable::TypingText {
                content_line: model.content.lines[line_idx].clone(),
                correctness_line: model.typing_correctness.lines[line_idx].clone(),
                status: model.status.clone(),
                anchor: Anchor::Center,
                shift: Shift { x: 0.0, y: 0.0 }, // Centered
                font_size: FontSize::WindowHeight(0.125), // Base font size
            });
        }
        
        // --- Render Status Panel (Bottom Left) ---
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
    // Similar to build_menu_ui, no changes needed here.
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