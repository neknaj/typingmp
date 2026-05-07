// src/ui.rs

#[cfg(feature = "uefi")]
extern crate alloc;

#[cfg(feature = "uefi")]
use core_maths::CoreFloat;

#[cfg(feature = "uefi")]
use alloc::vec::Vec;
#[cfg(not(feature = "uefi"))]
use std::vec::Vec;

#[cfg(feature = "uefi")]
use alloc::{
    format,
    string::{String, ToString},
};
#[cfg(not(feature = "uefi"))]
use std::string::{String, ToString};

use crate::app::{
    typing_line_scroll_offset, App, AppSnapshot, AppState, Script, ScrollCache, SettingsItem,
};
use crate::font::{script_for_segment, FontScript, Fonts};
use crate::model::{
    Segment, TypingCorrectnessChar, TypingCorrectnessSegment, TypingCorrectnessWord,
};
use crate::renderer::{calculate_pixel_font_size, gui_renderer};
use crate::typing; // For calculate_total_metrics

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

#[derive(Clone, Copy)]
pub struct Shift {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy)]
pub enum HorizontalAlign {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy)]
pub enum VerticalAlign {
    Top,
    Center,
    Bottom,
}

#[derive(Clone, Copy)]
pub struct Align {
    pub horizontal: HorizontalAlign,
    pub vertical: VerticalAlign,
}

#[derive(Clone, Copy)]
pub enum FontSize {
    WindowHeight(f32),
    WindowAreaSqrt(f32),
}

#[derive(Clone, Copy)]
pub struct Gradient {
    pub start_color: u32,
    pub end_color: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpperSegmentState {
    /// 打ち込みが正しく完了したセグメント。
    Correct,
    /// いま未入力のセグメント。
    Incorrect,
    /// 文字列が未確定で保留中。
    Pending,
    /// 現在入力中のセグメント。
    Active,
}

pub struct UpperTypingSegment {
    pub base_text: String,
    pub ruby_text: Option<String>,
    pub anno_text: Option<String>,
    pub script: FontScript,
    pub state: UpperSegmentState,
}

pub enum ActiveLowerElement {
    Typed { character: char, is_correct: bool },
    Cursor,
    UnconfirmedInput(String),
    LastIncorrectInput(char),
}

pub enum LowerTypingSegment {
    Completed {
        base_text: String,
        ruby_text: Option<String>,
        script: FontScript,
        is_correct: bool,
        width: u32,
    },
    Active {
        elements: Vec<ActiveLowerElement>,
        script: FontScript,
    },
}

/// Typing-line horizontal metrics shared by upper and lower play rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypingLineAlignment {
    pub full_line_width: u32,
    pub visible_start_width: u32,
}

impl TypingLineAlignment {
    pub const fn new(full_line_width: u32, visible_start_width: u32) -> Self {
        let visible_start_width = if visible_start_width > full_line_width {
            full_line_width
        } else {
            visible_start_width
        };
        Self {
            full_line_width,
            visible_start_width,
        }
    }

    pub const fn full_line(full_line_width: u32) -> Self {
        Self::new(full_line_width, 0)
    }
}

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
    TypingUpper {
        segments: Vec<UpperTypingSegment>,
        anchor: Anchor,
        shift: Shift,
        align: Align,
        font_size: FontSize,
        line_alignment: TypingLineAlignment,
    },
    TypingLower {
        segments: Vec<LowerTypingSegment>,
        anchor: Anchor,
        shift: Shift,
        align: Align,
        font_size: FontSize,
        line_alignment: TypingLineAlignment,
    },
    ProgressBar {
        anchor: Anchor,
        shift: Shift,
        width_ratio: f32,
        height_ratio: f32,
        progress: f32, // 0.0 to 1.0
        bg_color: u32,
        fg_color: u32,
    },
}

pub(crate) const HOW_TO_USE_CONTENT: &[(&str, u32)] = &[
    ("[ 基本操作 ]", 0xFF_FFDD88),
    ("", 0xFF_000000),
    ("[ タイピング ]", 0xFF_FFDD88),
    (
        "  ローマ字入力: a, ka, ki, ga, n / nn などを入力",
        0xFF_CCCCCC,
    ),
    (
        "  直接入力: 表示された文字をそのまま入力できます",
        0xFF_FFFFFF,
    ),
    ("", 0xFF_000000),
    ("[ メニュー操作 ]", 0xFF_FFDD88),
    ("  X     : 決定、または選択中の項目を実行", 0xFF_CCCCCC),
    ("  U / D : カーソルを上下に移動", 0xFF_CCCCCC),
    ("", 0xFF_000000),
    ("[ タイピング設定 ]", 0xFF_FFDD88),
    (
        "  問題ソース、スクリプト、表示モードを選択できます",
        0xFF_CCCCCC,
    ),
    (
        "  環境ごとの入力方式やフォント設定が反映されます",
        0xFF_CCCCCC,
    ),
    ("", 0xFF_000000),
    ("[ 問題ファイル ]", 0xFF_FFDD88),
    (
        "  .ntq ファイルでは [base/reading] 形式の注釈を使えます",
        0xFF_CCCCCC,
    ),
];

#[cfg(target_arch = "wasm32")]
const MENU_ITEMS: [&str; 3] = ["Start Typing", "How to Use", "Settings"];

#[cfg(not(target_arch = "wasm32"))]
const MENU_ITEMS: [&str; 4] = ["Start Typing", "How to Use", "Settings", "Quit"];

pub const BASE_FONT_SIZE_RATIO: f32 = 0.2;
const UPPER_ROW_Y_OFFSET_FACTOR: f32 = 1.3;
const LOWER_ROW_Y_OFFSET_FACTOR: f32 = 0.2;

pub const CORRECT_COLOR: u32 = 0xFF_9097FF;
pub const INCORRECT_COLOR: u32 = 0xFF_FF9898;
pub const PENDING_COLOR: u32 = 0xFF_999999;
pub const ACTIVE_COLOR: u32 = 0xFF_FFFFFF;
pub const WRONG_KEY_COLOR: u32 = 0xFF_F55252;
pub const CURSOR_COLOR: u32 = 0xFF_FFFFFF;
pub const UNCONFIRMED_COLOR: u32 = 0xFF_CCCCCC;

pub fn build_ui(app: &App, fonts: &Fonts, width: usize, height: usize) -> Vec<Renderable> {
    let mut render_list = Vec::new();
    let snapshot = app.snapshot();

    let menu_gradient = Gradient {
        start_color: 0xFF_000010,
        end_color: 0xFF_000000,
    };
    let typing_gradient = Gradient {
        start_color: 0xFF_100010,
        end_color: 0xFF_000000,
    };
    let result_gradient = Gradient {
        start_color: 0xFF_101000,
        end_color: 0xFF_000000,
    };
    let settings_gradient = Gradient {
        start_color: 0xFF_001010,
        end_color: 0xFF_000000,
    };

    match snapshot.state {
        AppState::MainMenu => build_main_menu_ui(snapshot, &mut render_list, menu_gradient),
        AppState::Typing => {
            build_typing_ui(app, &mut render_list, typing_gradient, fonts, width, height)
        }
        AppState::ProblemSelection => {
            build_problem_selection_ui(app, snapshot, &mut render_list, menu_gradient)
        }
        AppState::ProblemSource => {
            build_problem_source_ui(app, snapshot, &mut render_list, menu_gradient)
        }
        AppState::Result => build_result_ui(app, &mut render_list, result_gradient),
        AppState::Settings => build_settings_ui(snapshot, &mut render_list, settings_gradient),
        AppState::HowToUse => build_how_to_use_ui(snapshot, &mut render_list, menu_gradient),
    }

    if snapshot.state != AppState::Typing {
        render_list.push(Renderable::Text {
            text: snapshot.status_text.to_string(),
            anchor: Anchor::BottomLeft,
            shift: Shift { x: 0.01, y: -0.02 },
            align: Align {
                horizontal: HorizontalAlign::Left,
                vertical: VerticalAlign::Bottom,
            },
            font_size: FontSize::WindowHeight(0.04),
            color: 0xFF_CCCCCC,
        });
    }

    let fps_text = format!("FPS: {:.1}", snapshot.fps);
    render_list.push(Renderable::Text {
        text: fps_text,
        anchor: Anchor::TopRight,
        shift: Shift { x: -0.01, y: 0.01 },
        align: Align {
            horizontal: HorizontalAlign::Right,
            vertical: VerticalAlign::Top,
        },
        font_size: FontSize::WindowHeight(0.04),
        color: 0xFF_00FF00,
    });

    #[cfg(feature = "gui")]
    {
        render_list.push(Renderable::Text {
            text: "GUI".to_string(),
            anchor: Anchor::BottomRight,
            shift: Shift { x: -0.01, y: -0.06 },
            align: Align {
                horizontal: HorizontalAlign::Right,
                vertical: VerticalAlign::Bottom,
            },
            font_size: FontSize::WindowHeight(0.04),
            color: 0xFF_AAAAAA,
        });
    }

    #[cfg(all(feature = "tui", not(feature = "gui")))]
    {
        let mode_text = format!("TUI {:?}", snapshot.tui_display_mode);
        render_list.push(Renderable::Text {
            text: mode_text,
            anchor: Anchor::BottomRight,
            shift: Shift { x: -0.01, y: -0.06 },
            align: Align {
                horizontal: HorizontalAlign::Right,
                vertical: VerticalAlign::Bottom,
            },
            font_size: FontSize::WindowHeight(0.04),
            color: 0xFF_AAAAAA,
        });
    }

    render_list.push(Renderable::Text {
        text: snapshot.instructions_text.to_string(),
        anchor: Anchor::BottomRight,
        shift: Shift { x: -0.01, y: -0.03 },
        align: Align {
            horizontal: HorizontalAlign::Right,
            vertical: VerticalAlign::Bottom,
        },
        font_size: FontSize::WindowHeight(0.04),
        color: 0xFF_CCCCCC,
    });

    render_list
}

fn build_main_menu_ui(
    snapshot: AppSnapshot<'_>,
    render_list: &mut Vec<Renderable>,
    gradient: Gradient,
) {
    render_list.push(Renderable::Background { gradient });
    render_list.push(Renderable::BigText {
        text: "Neknaj Typing MP".to_string(),
        anchor: Anchor::TopCenter,
        shift: Shift { x: 0.0, y: 0.1 },
        align: Align {
            horizontal: HorizontalAlign::Center,
            vertical: VerticalAlign::Top,
        },
        font_size: FontSize::WindowHeight(0.20),
        color: 0xFF_FFFFFF,
    });
    for (i, item) in MENU_ITEMS.iter().enumerate() {
        let (text, color) = if i == snapshot.selected_main_menu_item.index() {
            (format!("> {} <", item), 0xFF_FFFF00)
        } else {
            (item.to_string(), 0xFF_FFFFFF)
        };
        render_list.push(Renderable::Text {
            text,
            anchor: Anchor::Center,
            shift: Shift {
                x: 0.0,
                y: 0.0 + (i as f32 * 0.1),
            },
            align: Align {
                horizontal: HorizontalAlign::Center,
                vertical: VerticalAlign::Center,
            },
            font_size: FontSize::WindowHeight(0.05),
            color,
        });
    }
}

fn build_settings_ui(
    snapshot: AppSnapshot<'_>,
    render_list: &mut Vec<Renderable>,
    gradient: Gradient,
) {
    render_list.push(Renderable::Background { gradient });
    render_list.push(Renderable::BigText {
        text: "Settings".to_string(),
        anchor: Anchor::TopCenter,
        shift: Shift { x: 0.0, y: 0.1 },
        align: Align {
            horizontal: HorizontalAlign::Center,
            vertical: VerticalAlign::Top,
        },
        font_size: FontSize::WindowHeight(0.2),
        color: 0xFF_FFFFFF,
    });

    let fonts = [
        (
            SettingsItem::Japanese,
            Script::Japanese.settings_label().to_string(),
        ),
        (
            SettingsItem::SimplifiedChinese,
            Script::SimplifiedChinese.settings_label().to_string(),
        ),
        (
            SettingsItem::TraditionalChinese,
            Script::TraditionalChinese.settings_label().to_string(),
        ),
        (
            SettingsItem::AspectRatio,
            format!(
                "Aspect Ratio: {}",
                snapshot.display_settings.aspect_ratio.label()
            ),
        ),
        (
            SettingsItem::DisplayScale,
            format!("Display Scale: {}", snapshot.display_settings.scale.label()),
        ),
    ];

    for (i, (settings_item, label)) in fonts.iter().enumerate() {
        let is_selected = i == snapshot.selected_settings_item.index();

        let mut display_text = if is_selected {
            format!("> {}", label)
        } else {
            format!("  {}", label)
        };

        if is_selected && settings_item.font_script().is_some() {
            display_text.push_str(" <assign>");
        } else if is_selected {
            display_text.push_str(" <cycle>");
        }

        let color = if is_selected {
            0xFF_FFFF00
        } else {
            0xFF_FFFFFF
        };

        render_list.push(Renderable::Text {
            text: display_text,
            anchor: Anchor::Center,
            shift: Shift {
                x: 0.0,
                y: 0.0 + (i as f32 * 0.1),
            },
            align: Align {
                horizontal: HorizontalAlign::Center,
                vertical: VerticalAlign::Center,
            },
            font_size: FontSize::WindowHeight(0.05),
            color,
        });
    }
}

fn build_problem_selection_ui(
    app: &App,
    snapshot: AppSnapshot<'_>,
    render_list: &mut Vec<Renderable>,
    gradient: Gradient,
) {
    render_list.push(Renderable::Background { gradient });
    render_list.push(Renderable::BigText {
        text: "Select Problem".to_string(),
        anchor: Anchor::TopCenter,
        shift: Shift { x: 0.0, y: 0.1 },
        align: Align {
            horizontal: HorizontalAlign::Center,
            vertical: VerticalAlign::Top,
        },
        font_size: FontSize::WindowHeight(0.2),
        color: 0xFF_FFFFFF,
    });

    let item_height: f32 = 0.06;
    let list_y_start: f32 = 0.4;
    let list_height: f32 = 0.6;
    let items_per_screen = (list_height / item_height).floor() as usize;

    let mut start_index = 0;
    if snapshot.selected_problem_item >= items_per_screen {
        start_index = snapshot.selected_problem_item - items_per_screen + 1;
    }
    let end_index = (start_index + items_per_screen).min(app.problem_count());

    for i in start_index..end_index {
        let item = app.problem_name_at(i);
        let is_open_file = app.is_open_file_entry(i);
        let badge = if is_open_file {
            "+".to_string()
        } else {
            app.problem_source_label(i).to_string()
        };
        let selected = i == snapshot.selected_problem_item;
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
            align: Align {
                horizontal: HorizontalAlign::Left,
                vertical: VerticalAlign::Top,
            },
            font_size: FontSize::WindowHeight(0.045),
            color,
        });
    }

    if start_index > 0 {
        render_list.push(Renderable::Text {
            text: "↑".to_string(),
            anchor: Anchor::TopCenter,
            shift: Shift {
                x: 0.0,
                y: list_y_start - item_height,
            },
            align: Align {
                horizontal: HorizontalAlign::Center,
                vertical: VerticalAlign::Center,
            },
            font_size: FontSize::WindowHeight(0.04),
            color: 0xFF_AAAAAA,
        });
    }
    if end_index < app.problem_count() {
        render_list.push(Renderable::Text {
            text: "↓".to_string(),
            anchor: Anchor::TopCenter,
            shift: Shift {
                x: 0.0,
                y: list_y_start + list_height,
            },
            align: Align {
                horizontal: HorizontalAlign::Center,
                vertical: VerticalAlign::Center,
            },
            font_size: FontSize::WindowHeight(0.04),
            color: 0xFF_AAAAAA,
        });
    }
}

fn build_problem_source_ui(
    app: &App,
    snapshot: AppSnapshot<'_>,
    render_list: &mut Vec<Renderable>,
    gradient: Gradient,
) {
    render_list.push(Renderable::Background { gradient });

    let idx = snapshot.selected_problem_item;
    let label = app.problem_source_label(idx);
    let name = app.problem_name_at(idx);

    // [source] 現在選択中の問題情報を上部に表示する
    render_list.push(Renderable::BigText {
        text: format!("[{}] {}", label, name),
        anchor: Anchor::TopCenter,
        shift: Shift { x: 0.0, y: 0.05 },
        align: Align {
            horizontal: HorizontalAlign::Center,
            vertical: VerticalAlign::Top,
        },
        font_size: FontSize::WindowHeight(0.09),
        color: 0xFF_AADDFF,
    });

    let line_h: f32 = 0.046;
    let content_y: f32 = 0.21;
    let max_lines = ((1.0f32 - content_y - 0.12) / line_h).floor() as usize;

    if let Some(content) = app.get_problem_source(idx) {
        let total_lines = content.lines().count();

        for (i, line) in content
            .lines()
            .skip(snapshot.source_scroll)
            .take(max_lines)
            .enumerate()
        {
            let ch_count = line.chars().count();
            let display = if ch_count > 60 {
                let truncated: String = line.chars().take(60).collect();
                format!("{}...", truncated)
            } else {
                line.to_string()
            };

            render_list.push(Renderable::Text {
                text: display,
                anchor: Anchor::TopCenter,
                shift: Shift {
                    x: -0.46,
                    y: content_y + i as f32 * line_h,
                },
                align: Align {
                    horizontal: HorizontalAlign::Left,
                    vertical: VerticalAlign::Top,
                },
                font_size: FontSize::WindowHeight(0.033),
                color: 0xFF_99DDAA,
            });
        }

        let scroll_text = if total_lines == 0 {
            "0/0".to_string()
        } else {
            format!("{}/{}", snapshot.source_scroll + 1, total_lines)
        };
        render_list.push(Renderable::Text {
            text: scroll_text,
            anchor: Anchor::TopRight,
            shift: Shift { x: -0.01, y: 0.14 },
            align: Align {
                horizontal: HorizontalAlign::Right,
                vertical: VerticalAlign::Top,
            },
            font_size: FontSize::WindowHeight(0.035),
            color: 0xFF_666688,
        });

        if snapshot.source_scroll > 0 {
            render_list.push(Renderable::Text {
                text: "↑".to_string(),
                anchor: Anchor::TopCenter,
                shift: Shift {
                    x: 0.45,
                    y: content_y - line_h,
                },
                align: Align {
                    horizontal: HorizontalAlign::Center,
                    vertical: VerticalAlign::Top,
                },
                font_size: FontSize::WindowHeight(0.035),
                color: 0xFF_888888,
            });
        }
        if snapshot.source_scroll + max_lines < total_lines {
            render_list.push(Renderable::Text {
                text: "↓".to_string(),
                anchor: Anchor::TopCenter,
                shift: Shift {
                    x: 0.45,
                    y: content_y + max_lines as f32 * line_h,
                },
                align: Align {
                    horizontal: HorizontalAlign::Center,
                    vertical: VerticalAlign::Top,
                },
                font_size: FontSize::WindowHeight(0.035),
                color: 0xFF_888888,
            });
        }
    }
}
fn build_how_to_use_ui(
    snapshot: AppSnapshot<'_>,
    render_list: &mut Vec<Renderable>,
    gradient: Gradient,
) {
    render_list.push(Renderable::Background { gradient });

    render_list.push(Renderable::BigText {
        text: "How to Use".to_string(),
        anchor: Anchor::TopCenter,
        shift: Shift { x: 0.0, y: 0.05 },
        align: Align {
            horizontal: HorizontalAlign::Center,
            vertical: VerticalAlign::Top,
        },
        font_size: FontSize::WindowHeight(0.09),
        color: 0xFF_AADDFF,
    });

    let line_h: f32 = 0.046;
    let content_y: f32 = 0.21;
    let max_lines = ((1.0f32 - content_y - 0.12) / line_h).floor() as usize;

    let total_lines = HOW_TO_USE_CONTENT.len();
    let scroll = snapshot.how_to_use_scroll;

    for (i, (text, color)) in HOW_TO_USE_CONTENT
        .iter()
        .skip(scroll)
        .take(max_lines)
        .enumerate()
    {
        render_list.push(Renderable::Text {
            text: text.to_string(),
            anchor: Anchor::TopCenter,
            shift: Shift {
                x: -0.46,
                y: content_y + i as f32 * line_h,
            },
            align: Align {
                horizontal: HorizontalAlign::Left,
                vertical: VerticalAlign::Top,
            },
            font_size: FontSize::WindowHeight(0.033),
            color: *color,
        });
    }

    let scroll_text = format!("{}/{}", scroll + 1, total_lines.max(1));
    render_list.push(Renderable::Text {
        text: scroll_text,
        anchor: Anchor::TopRight,
        shift: Shift { x: -0.01, y: 0.14 },
        align: Align {
            horizontal: HorizontalAlign::Right,
            vertical: VerticalAlign::Top,
        },
        font_size: FontSize::WindowHeight(0.035),
        color: 0xFF_666688,
    });

    if scroll > 0 {
        render_list.push(Renderable::Text {
            text: "↑".to_string(),
            anchor: Anchor::TopCenter,
            shift: Shift {
                x: 0.45,
                y: content_y - line_h,
            },
            align: Align {
                horizontal: HorizontalAlign::Center,
                vertical: VerticalAlign::Center,
            },
            font_size: FontSize::WindowHeight(0.035),
            color: 0xFF_888888,
        });
    }

    if scroll + max_lines < total_lines {
        render_list.push(Renderable::Text {
            text: "↓".to_string(),
            anchor: Anchor::TopCenter,
            shift: Shift {
                x: 0.45,
                y: content_y + max_lines as f32 * line_h,
            },
            align: Align {
                horizontal: HorizontalAlign::Center,
                vertical: VerticalAlign::Top,
            },
            font_size: FontSize::WindowHeight(0.035),
            color: 0xFF_888888,
        });
    }
}

fn segment_base_text(seg: &Segment) -> String {
    match seg {
        Segment::Plain { text } => text.clone(),
        Segment::Annotated { base, .. } => base.clone(),
        Segment::Anno { inner, .. } => inner.iter().map(segment_base_text).collect(),
    }
}

fn segment_reading_text(seg: &Segment) -> String {
    match seg {
        Segment::Plain { text } => text.clone(),
        Segment::Annotated { reading, .. } => reading.clone(),
        Segment::Anno { inner, .. } => inner.iter().map(segment_reading_text).collect(),
    }
}

fn segment_display_parts(seg: &Segment) -> (String, Option<String>, Option<String>) {
    match seg {
        Segment::Plain { text } => (text.clone(), None, None),
        Segment::Annotated { base, reading } => (base.clone(), Some(reading.clone()), None),
        Segment::Anno { inner, annotation } => {
            let base = inner.iter().map(segment_base_text).collect::<String>();
            let reading = inner.iter().map(segment_reading_text).collect::<String>();
            let ruby = if reading.is_empty() {
                None
            } else {
                Some(reading)
            };
            (base, ruby, Some(annotation.clone()))
        }
    }
}

fn is_word_correct(word: &TypingCorrectnessWord) -> bool {
    word.segments.iter().all(is_segment_correct)
}

fn is_segment_correct(segment: &TypingCorrectnessSegment) -> bool {
    !segment.chars.contains(&TypingCorrectnessChar::Incorrect)
}

fn measure_line_base_width(line: &crate::model::Line, fonts: &Fonts, font_size: f32) -> u32 {
    line.words
        .iter()
        .flat_map(|word| &word.segments)
        .map(|segment| {
            let text = segment_base_text(segment);
            let script = script_for_segment(segment);
            gui_renderer::measure_text(fonts.get_for_script(script), &text, font_size).0
        })
        .sum()
}

const TYPING_SCROLL_OVERSCAN_RATIO: f32 = 0.25;

fn typing_line_shift_x(scroll_offset: f32, width: usize) -> f32 {
    if width == 0 {
        0.0
    } else {
        (-scroll_offset - (width as f32 * 0.5)) / width as f32
    }
}

fn typing_visible_line_bounds(scroll_offset: f32, width: usize) -> (f32, f32) {
    let viewport = width as f32;
    if viewport <= 0.0 {
        return (0.0, 0.0);
    }

    let overscan = viewport * TYPING_SCROLL_OVERSCAN_RATIO;
    (
        (scroll_offset - overscan).max(0.0),
        scroll_offset + viewport + overscan,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VisibleSegmentRange {
    start: usize,
    end: usize,
    start_width: u32,
}

impl VisibleSegmentRange {
    fn contains(self, index: usize) -> bool {
        index >= self.start && index < self.end
    }
}

fn visible_segment_range(
    segment_prefix_width: &[f32],
    segment_count: usize,
    left_bound: f32,
    right_bound: f32,
) -> VisibleSegmentRange {
    let start = segment_prefix_width
        .partition_point(|value| *value <= left_bound)
        .saturating_sub(1)
        .min(segment_count);
    let mut end = segment_prefix_width
        .partition_point(|value| *value <= right_bound)
        .min(segment_count);
    if end < start {
        end = start;
    }

    let start_width = segment_prefix_width
        .get(start)
        .copied()
        .unwrap_or(0.0)
        .max(0.0) as u32;

    VisibleSegmentRange {
        start,
        end,
        start_width,
    }
}

fn cached_status_segment_index(
    word_segment_starts: &[usize],
    segment_count: usize,
    status_word: usize,
    status_segment: usize,
) -> usize {
    if status_word >= word_segment_starts.len() {
        return segment_count;
    }

    let word_start = word_segment_starts
        .get(status_word)
        .copied()
        .unwrap_or(segment_count);
    let word_end = word_segment_starts
        .get(status_word + 1)
        .copied()
        .unwrap_or(segment_count);
    let segment_count_in_word = word_end.saturating_sub(word_start);
    word_start + status_segment.min(segment_count_in_word)
}

fn build_typing_ui(
    app: &App,
    render_list: &mut Vec<Renderable>,
    gradient: Gradient,
    fonts: &Fonts,
    width: usize,
    height: usize,
) {
    render_list.push(Renderable::Background { gradient });

    if let Some(model) = app.typing_model() {
        render_list.push(Renderable::BigText {
            text: model.content.title.to_string(),
            anchor: Anchor::TopCenter,
            shift: Shift { x: 0.0, y: 0.01 },
            align: Align {
                horizontal: HorizontalAlign::Center,
                vertical: VerticalAlign::Top,
            },
            font_size: FontSize::WindowHeight(0.12),
            color: ACTIVE_COLOR,
        });

        let base_font_size = FontSize::WindowHeight(BASE_FONT_SIZE_RATIO);
        let base_pixel_font_size = calculate_pixel_font_size(base_font_size, width, height)
            * app.display_settings().scale.multiplier();
        let line_idx = model.status.line.get();
        let content_line = if let Some(line) = model.content.lines.get(line_idx) {
            line
        } else {
            return;
        };
        let correctness_line = if let Some(line) = model.typing_correctness.lines.get(line_idx) {
            line
        } else {
            return;
        };
        let status = &model.status;
        let cached_cache = match app.scroll_cache() {
            Some(ScrollCache::Ready(state)) if state.current.line == status.line => Some(state),
            _ => None,
        };
        let line_origin = cached_cache
            .map(|state| state.line_origin as f64)
            .unwrap_or(0.0);
        let full_line_width = match cached_cache {
            Some(state) => state.current.total_width as u32,
            None => measure_line_base_width(content_line, fonts, base_pixel_font_size),
        };
        let scroll_offset = if cached_cache.is_some() {
            (model.scroll.scroll - line_origin) as f32
        } else {
            typing_line_scroll_offset(full_line_width as f32, 0.0, width)
        };
        let line_shift_x = typing_line_shift_x(scroll_offset, width);
        let status_word = status.word.get();
        let status_segment = status.segment.get();
        let upper_visible_range = cached_cache.map(|state| {
            let current_cache = &state.current;
            let (left_bound, right_bound) = typing_visible_line_bounds(scroll_offset, width);
            let mut range = visible_segment_range(
                &current_cache.segment_prefix_width,
                current_cache.segments.len(),
                left_bound,
                right_bound,
            );
            let active_segment_idx = cached_status_segment_index(
                &current_cache.word_segment_starts,
                current_cache.segments.len(),
                status_word,
                status_segment,
            );

            if active_segment_idx < range.start {
                range.start = active_segment_idx;
            }
            if active_segment_idx < current_cache.segments.len() && active_segment_idx >= range.end
            {
                range.end = active_segment_idx + 1;
            }
            if range.end < range.start {
                range.end = range.start;
            }
            range.start_width = current_cache
                .segment_prefix_width
                .get(range.start)
                .copied()
                .unwrap_or(0.0)
                .max(0.0) as u32;
            range
        });
        let upper_visible_start_width = upper_visible_range
            .map(|range| range.start_width)
            .unwrap_or(0);
        let mut upper_segments = Vec::new();
        let mut flat_segment_index = 0usize;
        for (word_idx, word) in content_line.words.iter().enumerate() {
            for (seg_idx, seg) in word.segments.iter().enumerate() {
                let segment_index = flat_segment_index;
                flat_segment_index += 1;
                if upper_visible_range.is_some_and(|range| !range.contains(segment_index)) {
                    continue;
                }

                let state = if word_idx < status.word.get() {
                    if is_word_correct(&correctness_line.words[word_idx]) {
                        UpperSegmentState::Correct
                    } else {
                        UpperSegmentState::Incorrect
                    }
                } else if word_idx == status.word.get() {
                    if seg_idx < status.segment.get() {
                        let is_current_segment_correct = correctness_line
                            .words
                            .get(word_idx)
                            .and_then(|w| w.segments.get(seg_idx))
                            .is_some_and(is_segment_correct);
                        if is_current_segment_correct {
                            UpperSegmentState::Correct
                        } else {
                            UpperSegmentState::Incorrect
                        }
                    } else if seg_idx == status.segment.get() {
                        UpperSegmentState::Active
                    } else {
                        UpperSegmentState::Pending
                    }
                } else {
                    UpperSegmentState::Pending
                };

                let segment_script = script_for_segment(seg);
                let (base_text, ruby_text, anno_text) = segment_display_parts(seg);
                if word_idx == status.word.get()
                    && seg_idx == status.segment.get()
                    && ruby_text.is_none()
                    && anno_text.is_none()
                {
                    if let Some(correctness_segment) = correctness_line
                        .words
                        .get(word_idx)
                        .and_then(|word| word.segments.get(seg_idx))
                    {
                        let base_char_count = base_text.chars().count();
                        if base_char_count == correctness_segment.chars.len() {
                            for (char_idx, ch) in base_text.chars().enumerate() {
                                let state = match correctness_segment.chars[char_idx] {
                                    TypingCorrectnessChar::Correct => UpperSegmentState::Correct,
                                    TypingCorrectnessChar::Incorrect => {
                                        UpperSegmentState::Incorrect
                                    }
                                    TypingCorrectnessChar::Pending
                                        if char_idx == status.char_.get() =>
                                    {
                                        UpperSegmentState::Active
                                    }
                                    TypingCorrectnessChar::Pending => UpperSegmentState::Pending,
                                };
                                upper_segments.push(UpperTypingSegment {
                                    base_text: ch.to_string(),
                                    ruby_text: None,
                                    anno_text: None,
                                    script: segment_script,
                                    state,
                                });
                            }
                            continue;
                        }
                    }
                }
                upper_segments.push(UpperTypingSegment {
                    base_text,
                    ruby_text,
                    anno_text,
                    script: segment_script,
                    state,
                });
            }
        }

        let upper_y_shift_from_center =
            -(base_pixel_font_size * UPPER_ROW_Y_OFFSET_FACTOR) / height as f32 + 0.17;
        render_list.push(Renderable::TypingUpper {
            segments: upper_segments,
            anchor: Anchor::Center,
            shift: Shift {
                x: line_shift_x,
                y: upper_y_shift_from_center,
            },
            align: Align {
                horizontal: HorizontalAlign::Left,
                vertical: VerticalAlign::Center,
            },
            font_size: base_font_size,
            line_alignment: TypingLineAlignment::new(full_line_width, upper_visible_start_width),
        });

        let mut lower_segments = Vec::new();
        let mut lower_visible_start_width = 0;

        if let Some(state) = cached_cache {
            let current_cache = &state.current;
            let status_word_idx = status_word;
            let (left_bound, right_bound) = typing_visible_line_bounds(scroll_offset, width);

            let active_segment_idx = cached_status_segment_index(
                &current_cache.word_segment_starts,
                current_cache.segments.len(),
                status_word,
                status_segment,
            );

            let visible_range = visible_segment_range(
                &current_cache.segment_prefix_width,
                current_cache.segments.len(),
                left_bound,
                right_bound,
            );
            let mut visible_start = visible_range.start;
            let mut visible_end = visible_range.end;
            visible_start = visible_start.min(active_segment_idx);
            visible_end = visible_end
                .min(active_segment_idx)
                .min(current_cache.segments.len());
            if visible_end < visible_start {
                visible_end = visible_start;
            }

            if active_segment_idx < visible_start {
                visible_start = active_segment_idx;
            }
            if active_segment_idx > visible_end {
                visible_end = active_segment_idx;
            }

            lower_visible_start_width = current_cache
                .segment_prefix_width
                .get(visible_start)
                .copied()
                .unwrap_or(0.0) as u32;

            for cache_index in visible_start..visible_end {
                if let Some(cache_seg) = current_cache.segments.get(cache_index) {
                    let is_correct = match correctness_line.words.get(cache_seg.word_index) {
                        Some(correctness_word) if cache_seg.word_index < status_word => {
                            is_word_correct(correctness_word)
                        }
                        Some(correctness_word) if cache_seg.word_index == status_word_idx => {
                            correctness_word
                                .segments
                                .get(cache_seg.segment_index)
                                .is_some_and(is_segment_correct)
                        }
                        _ => false,
                    };

                    lower_segments.push(LowerTypingSegment::Completed {
                        base_text: cache_seg.base_text.clone(),
                        ruby_text: cache_seg.ruby_text.clone(),
                        script: cache_seg.script,
                        is_correct,
                        width: cache_seg.base_width as u32,
                    });
                }
            }

            if let Some(active_word_content) = content_line.words.get(status_word) {
                let active_word_idx = status_word;
                if active_word_idx < correctness_line.words.len() {
                    let active_correctness_word = &correctness_line.words[active_word_idx];
                    if let Some(active_seg_content) =
                        active_word_content.segments.get(status_segment)
                    {
                        let active_script = script_for_segment(active_seg_content);
                        let reading_text = segment_reading_text(active_seg_content);
                        let mut active_elements = Vec::new();

                        let correctness_seg = &active_correctness_word.segments[status_segment];
                        for (char_idx, character) in
                            reading_text.chars().enumerate().take(status.char_.get())
                        {
                            let is_correct =
                                correctness_seg.chars[char_idx] != TypingCorrectnessChar::Incorrect;
                            active_elements.push(ActiveLowerElement::Typed {
                                character,
                                is_correct,
                            });
                        }

                        if let Some(wrong_char) = status.last_wrong_keydown {
                            active_elements
                                .push(ActiveLowerElement::LastIncorrectInput(wrong_char));
                        } else {
                            if !status.unconfirmed.is_empty() {
                                let unconfirmed_text: String = status.unconfirmed.iter().collect();
                                active_elements
                                    .push(ActiveLowerElement::UnconfirmedInput(unconfirmed_text));
                            }
                            active_elements.push(ActiveLowerElement::Cursor);
                        }

                        lower_segments.push(LowerTypingSegment::Active {
                            elements: active_elements,
                            script: active_script,
                        });
                    }
                }
            }
        } else {
            for word_idx in 0..status_word {
                if let (Some(word), Some(correctness_word)) = (
                    content_line.words.get(word_idx),
                    correctness_line.words.get(word_idx),
                ) {
                    for seg in &word.segments {
                        let script = script_for_segment(seg);
                        let (base_text, ruby_text, _) = segment_display_parts(seg);
                        let seg_width = gui_renderer::measure_text(
                            fonts.get_for_script(script),
                            &base_text,
                            base_pixel_font_size,
                        )
                        .0;
                        lower_segments.push(LowerTypingSegment::Completed {
                            base_text,
                            ruby_text,
                            script,
                            is_correct: is_word_correct(correctness_word),
                            width: seg_width,
                        });
                    }
                }
            }

            if let (Some(active_word_content), Some(active_correctness_word)) = (
                content_line.words.get(status_word),
                correctness_line.words.get(status_word),
            ) {
                for seg_idx in 0..status_segment {
                    if let Some(seg) = active_word_content.segments.get(seg_idx) {
                        let script = script_for_segment(seg);
                        let (base_text, ruby_text, _) = segment_display_parts(seg);
                        let is_correct = active_correctness_word
                            .segments
                            .get(seg_idx)
                            .is_some_and(is_segment_correct);
                        let width = gui_renderer::measure_text(
                            fonts.get_for_script(script),
                            &base_text,
                            base_pixel_font_size,
                        )
                        .0;
                        lower_segments.push(LowerTypingSegment::Completed {
                            base_text,
                            ruby_text,
                            script,
                            is_correct,
                            width,
                        });
                    }
                }

                if let Some(active_seg_content) = active_word_content.segments.get(status_segment) {
                    let active_script = script_for_segment(active_seg_content);
                    let reading_text = segment_reading_text(active_seg_content);
                    let mut active_elements = Vec::new();

                    let correctness_seg = &active_correctness_word.segments[status_segment];
                    for (char_idx, character) in
                        reading_text.chars().enumerate().take(status.char_.get())
                    {
                        let is_correct =
                            correctness_seg.chars[char_idx] != TypingCorrectnessChar::Incorrect;
                        active_elements.push(ActiveLowerElement::Typed {
                            character,
                            is_correct,
                        });
                    }

                    if let Some(wrong_char) = status.last_wrong_keydown {
                        active_elements.push(ActiveLowerElement::LastIncorrectInput(wrong_char));
                    } else {
                        if !status.unconfirmed.is_empty() {
                            let unconfirmed_text: String = status.unconfirmed.iter().collect();
                            active_elements
                                .push(ActiveLowerElement::UnconfirmedInput(unconfirmed_text));
                        }
                        active_elements.push(ActiveLowerElement::Cursor);
                    }

                    lower_segments.push(LowerTypingSegment::Active {
                        elements: active_elements,
                        script: active_script,
                    });
                }
            }
        }

        let lower_y_shift_from_center =
            (base_pixel_font_size * LOWER_ROW_Y_OFFSET_FACTOR) / height as f32 + 0.01;
        render_list.push(Renderable::TypingLower {
            segments: lower_segments,
            anchor: Anchor::Center,
            shift: Shift {
                x: line_shift_x,
                y: lower_y_shift_from_center,
            },
            align: Align {
                horizontal: HorizontalAlign::Left,
                vertical: VerticalAlign::Top,
            },
            font_size: base_font_size,
            line_alignment: TypingLineAlignment::new(full_line_width, lower_visible_start_width),
        });

        let line_count = model.content.lines.len();
        for &offset in &[-1, 1] {
            let line_to_display_signed = model.status.line.get() as isize + offset;
            if line_to_display_signed >= 0 && (line_to_display_signed as usize) < line_count {
                let line_idx_context = line_to_display_signed as usize;
                render_list.push(Renderable::Text {
                    text: model.content.lines[line_idx_context].to_string(),
                    anchor: Anchor::Center,
                    shift: Shift {
                        x: 0.0,
                        y: (offset as f32 * 0.37) + 0.05,
                    },
                    align: Align {
                        horizontal: HorizontalAlign::Center,
                        vertical: VerticalAlign::Center,
                    },
                    font_size: FontSize::WindowHeight(0.08),
                    color: 0xFF_444444,
                });
            }
        }

        let metrics = typing::calculate_total_metrics(model);
        let time = metrics.total_time / 1000.0;
        let status_items = [
            format!("Progress: {} / {}", model.status.line.get() + 1, line_count),
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
                shift: Shift {
                    x: 0.02,
                    y: -0.02
                        - progress_bar_height_ratio
                        - ((status_items.len() - 1 - i) as f32 * status_item_height_ratio),
                },
                align: Align {
                    horizontal: HorizontalAlign::Left,
                    vertical: VerticalAlign::Bottom,
                },
                font_size: FontSize::WindowHeight(status_item_height_ratio),
                color: 0xFF_DDDDDD,
            });
        }

        let char_progress_in_line =
            model.status.word.get() as f32 / content_line.words.len().max(1) as f32;
        let detailed_progress_ratio = if line_count > 0 {
            (model.status.line.get() as f32 + char_progress_in_line) / (line_count as f32)
        } else {
            0.0
        };

        render_list.push(Renderable::ProgressBar {
            anchor: Anchor::BottomLeft,
            shift: Shift { x: 0.0, y: -0.005 },
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
        align: Align {
            horizontal: HorizontalAlign::Center,
            vertical: VerticalAlign::Center,
        },
        font_size: FontSize::WindowHeight(0.15),
        color: 0xFF_FFFF00,
    });

    if let Some(result) = app.result_model() {
        let metrics = crate::typing::calculate_total_metrics(&result.typing_model);
        let result_texts = [
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
                shift: Shift {
                    x: 0.0,
                    y: -0.1 + (i as f32 * 0.08),
                },
                align: Align {
                    horizontal: HorizontalAlign::Center,
                    vertical: VerticalAlign::Center,
                },
                font_size: FontSize::WindowHeight(0.05),
                color: 0xFF_FFFFFF,
            });
        }
    }
}

pub fn calculate_anchor_position(
    anchor: Anchor,
    shift: Shift,
    width: usize,
    height: usize,
) -> (i32, i32) {
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

pub fn calculate_aligned_position(
    anchor_pos: (i32, i32),
    text_width: u32,
    text_height: u32,
    align: Align,
) -> (i32, i32) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{App, AppEvent, Fonts};
    use crate::display::DisplayScale;
    use crate::font::FontScript;
    use ab_glyph::FontVec;

    fn test_fonts() -> Fonts {
        fn font() -> FontVec {
            FontVec::try_from_vec(include_bytes!("../fonts/YujiSyuku-Regular.ttf").to_vec())
                .expect("test font should parse")
        }

        Fonts::new(font(), font(), font())
    }

    fn typing_app(problem: &str) -> App {
        let mut app = App::new(test_fonts());
        app.add_custom_problem("plain".to_string(), problem.to_string(), 0);
        app.on_event(AppEvent::Enter);
        app
    }

    fn typing_rows(render_list: &[Renderable]) -> (&[UpperTypingSegment], &[LowerTypingSegment]) {
        let upper_segments = render_list
            .iter()
            .find_map(|item| match item {
                Renderable::TypingUpper { segments, .. } => Some(segments.as_slice()),
                _ => None,
            })
            .expect("typing upper renderable should exist");
        let lower_segments = render_list
            .iter()
            .find_map(|item| match item {
                Renderable::TypingLower { segments, .. } => Some(segments.as_slice()),
                _ => None,
            })
            .expect("typing lower renderable should exist");
        (upper_segments, lower_segments)
    }

    fn typing_alignment(render_list: &[Renderable]) -> (TypingLineAlignment, TypingLineAlignment) {
        let upper_alignment = render_list
            .iter()
            .find_map(|item| match item {
                Renderable::TypingUpper { line_alignment, .. } => Some(*line_alignment),
                _ => None,
            })
            .expect("typing upper renderable should exist");
        let lower_alignment = render_list
            .iter()
            .find_map(|item| match item {
                Renderable::TypingLower { line_alignment, .. } => Some(*line_alignment),
                _ => None,
            })
            .expect("typing lower renderable should exist");
        (upper_alignment, lower_alignment)
    }

    fn upper_line_width(render_list: &[Renderable]) -> u32 {
        render_list
            .iter()
            .find_map(|item| match item {
                Renderable::TypingUpper { line_alignment, .. } => {
                    Some(line_alignment.full_line_width)
                }
                _ => None,
            })
            .expect("typing upper renderable should exist")
    }

    fn assert_line_left_matches_scroll_offset(
        anchor: Anchor,
        shift: Shift,
        align: Align,
        line_width: u32,
        scroll_offset: f32,
    ) {
        assert!(matches!(align.horizontal, HorizontalAlign::Left));

        let anchor_pos = calculate_anchor_position(anchor, shift, 240, 500);
        let (line_left, _) = calculate_aligned_position(anchor_pos, line_width, 1, align);
        assert!(
            (line_left as f32 + scroll_offset).abs() <= 2.0,
            "line_left {line_left} should render as -scroll_offset {scroll_offset}"
        );
    }

    #[test]
    fn active_plain_upper_segments_follow_lower_typed_colors() {
        let mut app = typing_app("#title Test\n色は句");
        app.on_event(AppEvent::Char {
            c: '色',
            timestamp: 1.0,
        });
        app.on_event(AppEvent::Char {
            c: 'は',
            timestamp: 2.0,
        });

        let render_list = build_ui(&app, app.fonts(), 800, 500);
        let (upper_segments, lower_segments) = typing_rows(&render_list);
        assert_eq!(upper_segments[0].base_text, "色");
        assert_eq!(upper_segments[0].state, UpperSegmentState::Correct);
        assert_eq!(upper_segments[1].base_text, "は");
        assert_eq!(upper_segments[1].state, UpperSegmentState::Correct);
        assert_eq!(upper_segments[2].base_text, "句");
        assert_eq!(upper_segments[2].state, UpperSegmentState::Active);

        let LowerTypingSegment::Active { elements, .. } = &lower_segments[0] else {
            panic!("first lower segment should be active");
        };
        assert!(matches!(
            elements.as_slice(),
            [
                ActiveLowerElement::Typed {
                    character: '色',
                    is_correct: true
                },
                ActiveLowerElement::Typed {
                    character: 'は',
                    is_correct: true
                },
                ActiveLowerElement::Cursor
            ]
        ));

        let (upper_alignment, lower_alignment) = typing_alignment(&render_list);
        assert_eq!(upper_alignment.visible_start_width, 0);
        assert_eq!(
            lower_alignment,
            TypingLineAlignment::full_line(upper_alignment.full_line_width)
        );
    }

    #[test]
    fn problem_readings_route_each_segment_to_its_font_script() {
        let app = typing_app("#title Test\n[色/いろ][字/zi][字/ㄗˋ]");

        let render_list = build_ui(&app, app.fonts(), 800, 500);
        let (upper_segments, lower_segments) = typing_rows(&render_list);

        assert_eq!(upper_segments[0].script, FontScript::Japanese);
        assert_eq!(upper_segments[1].script, FontScript::SimplifiedChinese);
        assert_eq!(upper_segments[2].script, FontScript::TraditionalChinese);

        let LowerTypingSegment::Active { script, .. } = &lower_segments[0] else {
            panic!("first lower segment should be active");
        };
        assert_eq!(*script, FontScript::Japanese);
    }

    #[test]
    fn display_scale_is_applied_to_typing_line_measurement() {
        let mut app = typing_app("#title Test\n[色/いろ][字/zi][字/ㄗˋ]");

        let normal_width = upper_line_width(&build_ui(&app, app.fonts(), 800, 500));
        app.display_settings.scale = DisplayScale::Percent200;
        let scaled_width = upper_line_width(&build_ui(&app, app.fonts(), 800, 500));

        assert!(scaled_width > normal_width);
        assert!(
            (scaled_width as f32 - normal_width as f32 * 2.0).abs() <= 2.0,
            "scaled width {scaled_width} should be roughly double normal width {normal_width}"
        );
    }

    #[test]
    fn cached_lower_segments_keep_their_full_line_offset() {
        let mut app = typing_app(
            "#title Test\n[a/a][b/b][c/c][d/d][e/e][f/f][g/g][h/h][i/i][j/j][k/k][l/l][m/m][n/n][o/o][p/p][q/q][r/r][s/s][t/t][u/u][v/v][w/w][x/x][y/y][z/z]",
        );
        for (index, c) in "abcdefghijklmnopqr".chars().enumerate() {
            app.on_event(AppEvent::Char {
                c,
                timestamp: index as f64,
            });
        }
        app.update(240, 500, 100.0);

        let render_list = build_ui(&app, app.fonts(), 240, 500);
        let (upper_alignment, lower_alignment) = typing_alignment(&render_list);
        let (upper_segments, lower_segments) = typing_rows(&render_list);
        let Some(ScrollCache::Ready(cache)) = app.scroll_cache() else {
            panic!("scroll cache should be ready");
        };

        assert_eq!(
            upper_alignment.full_line_width,
            cache.current.total_width as u32
        );
        assert_eq!(
            lower_alignment.full_line_width,
            upper_alignment.full_line_width
        );
        assert!(
            upper_alignment.visible_start_width > 0,
            "long cached upper rows should preserve a non-zero clipped prefix"
        );
        assert!(
            lower_alignment.visible_start_width > 0,
            "long cached rows should preserve a non-zero clipped prefix"
        );
        assert!(
            upper_segments.len() < cache.current.segments.len(),
            "cached upper row should only include the visible segment window"
        );

        let first_upper_text = &upper_segments
            .first()
            .expect("clipped upper row should contain visible segments")
            .base_text;
        let first_upper_segment = cache
            .current
            .segments
            .iter()
            .position(|segment| &segment.base_text == first_upper_text)
            .expect("visible upper segment should come from the scroll cache");
        assert_eq!(
            upper_alignment.visible_start_width,
            cache.current.segment_prefix_width[first_upper_segment] as u32
        );

        let first_completed_text = lower_segments
            .iter()
            .find_map(|segment| match segment {
                LowerTypingSegment::Completed { base_text, .. } => Some(base_text),
                LowerTypingSegment::Active { .. } => None,
            })
            .expect("clipped lower row should contain visible completed segments");
        let first_visible_segment = cache
            .current
            .segments
            .iter()
            .position(|segment| &segment.base_text == first_completed_text)
            .expect("visible lower segment should come from the scroll cache");
        assert_eq!(
            lower_alignment.visible_start_width,
            cache.current.segment_prefix_width[first_visible_segment] as u32
        );
    }

    #[test]
    fn cached_typing_rows_use_scroll_as_viewport_left() {
        let mut app = typing_app(
            "#title Test\n[a/a][b/b][c/c][d/d][e/e][f/f][g/g][h/h][i/i][j/j][k/k][l/l][m/m][n/n][o/o][p/p][q/q][r/r][s/s][t/t][u/u][v/v][w/w][x/x][y/y][z/z]",
        );
        for (index, c) in "abcdefghijklmnopqr".chars().enumerate() {
            app.on_event(AppEvent::Char {
                c,
                timestamp: index as f64,
            });
        }
        app.update(240, 500, 100.0);

        let render_list = build_ui(&app, app.fonts(), 240, 500);
        let Some(model) = app.typing_model() else {
            panic!("typing model should exist");
        };
        let Some(ScrollCache::Ready(cache)) = app.scroll_cache() else {
            panic!("scroll cache should be ready");
        };
        let scroll_offset = (model.scroll.scroll - cache.line_origin as f64) as f32;
        let mut checked_rows = 0;

        for item in &render_list {
            match item {
                Renderable::TypingUpper {
                    anchor,
                    shift,
                    align,
                    line_alignment,
                    ..
                } => {
                    assert_line_left_matches_scroll_offset(
                        *anchor,
                        *shift,
                        *align,
                        line_alignment.full_line_width,
                        scroll_offset,
                    );
                    checked_rows += 1;
                }
                Renderable::TypingLower {
                    anchor,
                    shift,
                    align,
                    line_alignment,
                    ..
                } => {
                    assert_line_left_matches_scroll_offset(
                        *anchor,
                        *shift,
                        *align,
                        line_alignment.full_line_width,
                        scroll_offset,
                    );
                    checked_rows += 1;
                }
                _ => {}
            }
        }

        assert_eq!(checked_rows, 2);
    }
}
