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
    typing_line_scroll_offset, App, AppSnapshot, AppState, FontTarget, ScrollCache, SettingsItem,
};
use crate::font::{
    plain_text_script_runs, script_for_segment, scripts_for_line, segment_script_runs, FontScript,
    Fonts,
};
use crate::model::{
    Segment, TypingCorrectnessChar, TypingCorrectnessSegment, TypingCorrectnessWord, TypingMetrics,
    TypingModel,
};
use crate::pinyin;
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
    /// Context lines shown before/after the active typing line.
    Muted,
}

pub struct UpperTypingSegment {
    pub base_text: String,
    pub ruby_text: Option<String>,
    pub anno_text: Option<String>,
    pub anno_group_run_count: usize,
    pub anno_script: Option<FontScript>,
    pub script: FontScript,
    pub state: UpperSegmentState,
}

#[derive(Clone, Copy)]
enum UpperRubyDisplay {
    InputKeys,
    Presentation,
}

pub enum ActiveLowerElement {
    Typed {
        character: char,
        is_correct: bool,
        script: FontScript,
    },
    Cursor,
    UnconfirmedInput {
        text: String,
        script: FontScript,
    },
    LastIncorrectInput {
        character: char,
        script: FontScript,
    },
}

pub fn active_lower_element_uses_unconfirmed_font(element: &ActiveLowerElement) -> bool {
    match element {
        ActiveLowerElement::UnconfirmedInput { .. } => true,
        ActiveLowerElement::Typed { script, .. } => script.is_chinese(),
        ActiveLowerElement::LastIncorrectInput { script, .. } => script.is_cjk(),
        ActiveLowerElement::Cursor => false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ActiveLowerMeasureStyle {
    script: FontScript,
    use_unconfirmed_font: bool,
}

fn active_lower_measure_style(
    element: &ActiveLowerElement,
    fallback_script: FontScript,
) -> ActiveLowerMeasureStyle {
    match element {
        ActiveLowerElement::Typed { script, .. }
        | ActiveLowerElement::LastIncorrectInput { script, .. }
        | ActiveLowerElement::UnconfirmedInput { script, .. } => ActiveLowerMeasureStyle {
            script: *script,
            use_unconfirmed_font: active_lower_element_uses_unconfirmed_font(element),
        },
        ActiveLowerElement::Cursor => ActiveLowerMeasureStyle {
            script: fallback_script,
            use_unconfirmed_font: false,
        },
    }
}

fn measure_active_lower_style_height(
    fonts: &Fonts,
    style: ActiveLowerMeasureStyle,
    pixel_font_size: f32,
) -> f32 {
    if style.use_unconfirmed_font {
        let size = fonts.scaled_size_for_unconfirmed_script(style.script, pixel_font_size);
        gui_renderer::measure_text(fonts.get_unconfirmed_for_script(style.script), "Hg", size).1
            as f32
    } else {
        let size = fonts.scaled_size_for_script(style.script, pixel_font_size);
        gui_renderer::measure_text(fonts.get_for_script(style.script), "Hg", size).1 as f32
    }
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
        "  UI、各言語、日/中ruby/unconfirmedのfontとscaleを選択できます",
        0xFF_CCCCCC,
    ),
    (
        "  titleと問題文はruby/baseとscript判定で描画されます",
        0xFF_CCCCCC,
    ),
    ("", 0xFF_000000),
    ("[ 問題ファイル ]", 0xFF_FFDD88),
    (
        "  .ntq ファイルでは [base/reading] 形式の注釈を使えます",
        0xFF_CCCCCC,
    ),
    (
        "  Chinese pinyin ruby uses numbered tones: [有/you3]",
        0xFF_CCCCCC,
    ),
    (
        "  上段入力だけyou3、title/lower/unconfirmedはy\u{01d2}u表示",
        0xFF_CCCCCC,
    ),
];

#[cfg(feature = "wasm")]
const MENU_ITEMS: [&str; 3] = ["Start Typing", "How to Use", "Settings"];

#[cfg(not(feature = "wasm"))]
const MENU_ITEMS: [&str; 4] = ["Start Typing", "How to Use", "Settings", "Quit"];

pub const BASE_FONT_SIZE_RATIO: f32 = 0.2;
const TYPING_TITLE_FONT_SIZE_RATIO: f32 = 0.10;
const TYPING_CONTEXT_FONT_SIZE_RATIO: f32 = 0.06;
const TYPING_CORE_CENTER_RATIO: f32 = 0.51;
const TYPING_CORE_MIN_GAP_RATIO: f32 = 0.012;
const TYPING_FLOAT_TOP_MARGIN_RATIO: f32 = 0.035;
const TYPING_FLOAT_MIN_GAP_RATIO: f32 = 0.012;
const TYPING_PREVIOUS_CONTEXT_ROUNDING_GUARD_PX: f32 = 12.0;
const TYPING_STATUS_LEFT_MARGIN_RATIO: f32 = 0.02;
const TYPING_STATUS_LABEL_VALUE_GAP_RATIO: f32 = 0.012;
const TYPING_STATUS_NEXT_CONTEXT_GAP_RATIO: f32 = 0.035;
const TYPING_STATUS_BOTTOM_MARGIN_RATIO: f32 = 0.02;
const TYPING_STATUS_PROGRESS_BAR_HEIGHT_RATIO: f32 = 0.02;
const TYPING_STATUS_ITEM_HEIGHT_RATIO: f32 = 0.04;

pub const CORRECT_COLOR: u32 = 0xFF_9097FF;
pub const INCORRECT_COLOR: u32 = 0xFF_FF9898;
pub const PENDING_COLOR: u32 = 0xFF_999999;
pub const ACTIVE_COLOR: u32 = 0xFF_FFFFFF;
pub const WRONG_KEY_COLOR: u32 = 0xFF_F55252;
pub const CURSOR_COLOR: u32 = 0xFF_FFFFFF;
pub const UNCONFIRMED_COLOR: u32 = 0xFF_CCCCCC;

#[derive(Debug, Clone, Copy)]
struct TypingLineVerticalMetrics {
    base_height: f32,
    top_extra: f32,
    bottom_extra: f32,
}

struct StatusTableRow {
    label: &'static str,
    value: String,
    value_width_hint: &'static str,
}

#[derive(Debug, Clone, Copy)]
struct StatusTableLayout {
    label_right: f32,
    value_left: f32,
    right: f32,
}

fn typing_status_rows(
    model: &TypingModel,
    line_count: usize,
    metrics: TypingMetrics,
) -> [StatusTableRow; 5] {
    let time = metrics.total_time / 1000.0;
    [
        StatusTableRow {
            label: "Progress:",
            value: format!("{} / {}", model.status.line.get() + 1, line_count),
            value_width_hint: "999 / 999",
        },
        StatusTableRow {
            label: "Speed:",
            value: format!("{:.2} KPS", metrics.speed),
            value_width_hint: "999.99 KPS",
        },
        StatusTableRow {
            label: "Accuracy:",
            value: format!("{:.1}%", metrics.accuracy * 100.0),
            value_width_hint: "100.0%",
        },
        StatusTableRow {
            label: "Misses:",
            value: metrics.miss_count.to_string(),
            value_width_hint: "999",
        },
        StatusTableRow {
            label: "Time:",
            value: format!("{:02.0}:{:05.2}", (time / 60.0).floor(), time % 60.0),
            value_width_hint: "99:59.99",
        },
    ]
}

impl TypingLineVerticalMetrics {
    fn total_height(self) -> f32 {
        self.top_extra + self.base_height + self.bottom_extra
    }
}

#[derive(Debug, Clone, Copy)]
struct TypingCoreVerticalLayout {
    upper_shift_y: f32,
    lower_shift_y: f32,
    top: f32,
}

fn typing_core_vertical_layout(
    height: usize,
    upper: TypingLineVerticalMetrics,
    lower: TypingLineVerticalMetrics,
) -> TypingCoreVerticalLayout {
    let viewport_height = height.max(1) as f32;
    let gap = viewport_height * TYPING_CORE_MIN_GAP_RATIO;
    let upper_height = upper.total_height();
    let lower_height = lower.total_height();
    let group_height = upper_height + gap + lower_height;
    let preferred_top = viewport_height * TYPING_CORE_CENTER_RATIO - group_height * 0.5;
    let max_top = (viewport_height - group_height).max(0.0);
    let top = preferred_top.clamp(0.0, max_top);
    let upper_base_y = top + upper.top_extra;
    let upper_anchor_y = upper_base_y + upper.base_height * 0.5;
    let upper_bottom = top + upper_height;
    let lower_top = upper_bottom + gap;
    let lower_base_y = lower_top + lower.top_extra;
    let lower_anchor_y = lower_base_y;

    TypingCoreVerticalLayout {
        upper_shift_y: (upper_anchor_y - viewport_height * 0.5) / viewport_height,
        lower_shift_y: (lower_anchor_y - viewport_height * 0.5) / viewport_height,
        top,
    }
}

fn upper_typing_vertical_metrics(
    segments: &[UpperTypingSegment],
    fonts: &Fonts,
    pixel_font_size: f32,
) -> TypingLineVerticalMetrics {
    let fallback_base_height =
        gui_renderer::measure_text(fonts.primary(), " ", pixel_font_size).1 as f32;
    let mut base_height = fallback_base_height;
    let mut top_extra = 0.0_f32;
    let mut bottom_extra = 0.0_f32;

    for segment in segments {
        let base_size = fonts.scaled_size_for_script(segment.script, pixel_font_size);
        let ruby_pixel_font_size =
            fonts.scaled_size_for_ruby_script(segment.script, pixel_font_size * 0.4);
        let base_font = fonts.get_for_script(segment.script);
        let ruby_font = fonts.get_ruby_for_script(segment.script);
        let segment_base_height = gui_renderer::measure_text(base_font, " ", base_size).1 as f32;
        let ruby_measure_text = segment.ruby_text.as_deref().unwrap_or(" ");
        let ruby_height =
            gui_renderer::measure_text(ruby_font, ruby_measure_text, ruby_pixel_font_size).1 as f32;
        let ruby_y = -ruby_pixel_font_size * 0.5;
        let anno_bottom_extra = if let Some(anno_text) = segment.anno_text.as_deref() {
            let anno_script = segment.anno_script.unwrap_or(segment.script);
            let anno_font = fonts.get_ruby_for_script(anno_script);
            let anno_pixel_font_size =
                fonts.scaled_size_for_ruby_script(anno_script, pixel_font_size * 0.3);
            let anno_height =
                gui_renderer::measure_text(anno_font, anno_text, anno_pixel_font_size).1 as f32;
            anno_pixel_font_size * 0.15 + anno_height
        } else {
            0.0
        };

        base_height = base_height.max(segment_base_height);
        top_extra = top_extra.max(-ruby_y);
        bottom_extra = bottom_extra
            .max((ruby_y + ruby_height - segment_base_height).max(0.0))
            .max(anno_bottom_extra);
    }

    TypingLineVerticalMetrics {
        base_height,
        top_extra,
        bottom_extra,
    }
}

fn lower_typing_vertical_metrics(
    segments: &[LowerTypingSegment],
    fonts: &Fonts,
    pixel_font_size: f32,
) -> TypingLineVerticalMetrics {
    let fallback_base_height =
        gui_renderer::measure_text(fonts.primary(), " ", pixel_font_size).1 as f32;
    let mut base_height = fallback_base_height;
    let mut top_extra = 0.0_f32;
    let mut bottom_extra = 0.0_f32;

    for segment in segments {
        let script = match segment {
            LowerTypingSegment::Completed { script, .. } => *script,
            LowerTypingSegment::Active { script, .. } => *script,
        };
        let base_size = fonts.scaled_size_for_script(script, pixel_font_size);
        let ruby_pixel_font_size = fonts.scaled_size_for_ruby_script(script, pixel_font_size * 0.3);
        let base_font = fonts.get_for_script(script);
        let ruby_font = fonts.get_ruby_for_script(script);
        let segment_base_height = gui_renderer::measure_text(base_font, " ", base_size).1 as f32;
        let ruby_measure_text = match segment {
            LowerTypingSegment::Completed { ruby_text, .. } => ruby_text.as_deref().unwrap_or(" "),
            LowerTypingSegment::Active { .. } => " ",
        };
        let ruby_height =
            gui_renderer::measure_text(ruby_font, ruby_measure_text, ruby_pixel_font_size).1 as f32;
        let ruby_y = -ruby_pixel_font_size * 0.5;

        base_height = base_height.max(segment_base_height);
        top_extra = top_extra.max(-ruby_y);
        bottom_extra = bottom_extra.max((ruby_y + ruby_height - segment_base_height).max(0.0));

        if let LowerTypingSegment::Active { elements, .. } = segment {
            let mut measured_styles = Vec::new();
            for element in elements {
                let style = active_lower_measure_style(element, script);
                if measured_styles.contains(&style) {
                    continue;
                }
                measured_styles.push(style);
                let height = measure_active_lower_style_height(fonts, style, pixel_font_size);
                base_height = base_height.max(height);
            }
        }
    }

    TypingLineVerticalMetrics {
        base_height,
        top_extra,
        bottom_extra,
    }
}

fn typing_status_region_top(height: usize, status_item_count: usize, status_row_step: f32) -> f32 {
    let reserved_ratio = TYPING_STATUS_BOTTOM_MARGIN_RATIO
        + TYPING_STATUS_PROGRESS_BAR_HEIGHT_RATIO
        + status_item_count as f32 * status_row_step
        + TYPING_FLOAT_MIN_GAP_RATIO;
    height.max(1) as f32 * (1.0 - reserved_ratio).max(0.0)
}

fn typing_status_text_block_top(
    height: usize,
    status_item_count: usize,
    status_row_step: f32,
) -> f32 {
    typing_status_region_top(height, status_item_count, status_row_step)
        + height.max(1) as f32 * TYPING_FLOAT_MIN_GAP_RATIO
}

fn ui_text_pixel_font_size(
    fonts: &Fonts,
    font_size: FontSize,
    width: usize,
    height: usize,
    display_scale: f32,
) -> f32 {
    fonts.scaled_size_for_ui(calculate_pixel_font_size(font_size, width, height) * display_scale)
}

fn measured_ui_row_step(
    fonts: &Fonts,
    font_size: FontSize,
    width: usize,
    height: usize,
    display_scale: f32,
    minimum_ratio: f32,
) -> f32 {
    let pixel_font_size = ui_text_pixel_font_size(fonts, font_size, width, height, display_scale);
    let measured_height = gui_renderer::measure_text(fonts.ui(), "Hg", pixel_font_size).1 as f32;
    ((measured_height + 6.0) / height.max(1) as f32).max(minimum_ratio)
}

fn status_table_layout(
    fonts: &Fonts,
    rows: &[StatusTableRow],
    width: usize,
    height: usize,
    display_scale: f32,
) -> StatusTableLayout {
    let pixel_font_size = ui_text_pixel_font_size(
        fonts,
        FontSize::WindowHeight(TYPING_STATUS_ITEM_HEIGHT_RATIO),
        width,
        height,
        display_scale,
    );
    let label_width = rows
        .iter()
        .map(|row| gui_renderer::measure_text(fonts.ui(), row.label, pixel_font_size).0)
        .max()
        .unwrap_or(0) as f32;
    let value_width = rows
        .iter()
        .map(|row| {
            let value_width = gui_renderer::measure_text(fonts.ui(), &row.value, pixel_font_size).0;
            let hint_width =
                gui_renderer::measure_text(fonts.ui(), row.value_width_hint, pixel_font_size).0;
            value_width.max(hint_width)
        })
        .max()
        .unwrap_or(0) as f32;

    let viewport_width = width.max(1) as f32;
    let label_right = viewport_width * TYPING_STATUS_LEFT_MARGIN_RATIO + label_width;
    let value_left = label_right + viewport_width * TYPING_STATUS_LABEL_VALUE_GAP_RATIO;
    StatusTableLayout {
        label_right,
        value_left,
        right: value_left + value_width,
    }
}

fn status_float_right_edge(
    fonts: &Fonts,
    rows: &[StatusTableRow],
    width: usize,
    height: usize,
    display_scale: f32,
) -> f32 {
    status_table_layout(fonts, rows, width, height, display_scale).right
        + width.max(1) as f32 * TYPING_STATUS_NEXT_CONTEXT_GAP_RATIO
}

fn top_anchor_shift_y_for_box_top(
    box_top: f32,
    metrics: TypingLineVerticalMetrics,
    height: usize,
) -> f32 {
    (box_top + metrics.top_extra) / height.max(1) as f32
}

fn center_anchor_top_shift_y_for_box_top(
    box_top: f32,
    metrics: TypingLineVerticalMetrics,
    height: usize,
) -> f32 {
    let viewport_height = height.max(1) as f32;
    (box_top + metrics.top_extra - viewport_height * 0.5) / viewport_height
}

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
        AppState::ProblemSelection => build_problem_selection_ui(
            app,
            snapshot,
            &mut render_list,
            menu_gradient,
            fonts,
            width,
            height,
        ),
        AppState::ProblemSource => {
            build_problem_source_ui(app, snapshot, &mut render_list, menu_gradient)
        }
        AppState::Result => build_result_ui(app, &mut render_list, result_gradient),
        AppState::Settings => build_settings_ui(
            app,
            snapshot,
            &mut render_list,
            settings_gradient,
            fonts,
            width,
            height,
        ),
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
    app: &App,
    snapshot: AppSnapshot<'_>,
    render_list: &mut Vec<Renderable>,
    gradient: Gradient,
    fonts: &Fonts,
    width: usize,
    height: usize,
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
        font_size: FontSize::WindowHeight(0.12),
        color: 0xFF_FFFFFF,
    });

    if app.settings_picking_font {
        build_font_picker_ui(app, snapshot, render_list, fonts, width, height);
        return;
    }

    let settings_rows = settings_table_rows(app, snapshot);
    let total_rows = settings_rows.len();
    if total_rows == 0 {
        return;
    }

    let display_scale = snapshot.display_settings.scale.multiplier();
    let list_y_start = 0.29;
    let list_y_end = 0.84;
    let cell_font_size = FontSize::WindowHeight(0.042);
    let row_step = measured_ui_row_step(fonts, cell_font_size, width, height, display_scale, 0.058);
    let cell_pixel_font_size =
        ui_text_pixel_font_size(fonts, cell_font_size, width, height, display_scale);
    let label_x = settings_cell_left_x(width, -0.35);
    let value_x = settings_cell_left_x(width, 0.05);
    let action_x = settings_cell_left_x(width, 0.36);
    let label_max_width = (value_x - label_x - 8.0).max(24.0);
    let value_max_width = (action_x - value_x - 8.0).max(24.0);
    let action_max_width = (width as f32 * 0.98 - action_x).max(24.0);
    let list_capacity = (((list_y_end - list_y_start) / row_step).floor() as usize)
        .max(1)
        .min(total_rows);
    let selected_index = snapshot.selected_settings_item.index().min(total_rows - 1);
    let start_index = selected_index
        .saturating_add(1)
        .saturating_sub(list_capacity)
        .min(total_rows - list_capacity);
    let end_index = (start_index + list_capacity).min(total_rows);

    if start_index > 0 {
        render_list.push(Renderable::Text {
            text: "↑".to_string(),
            anchor: Anchor::TopCenter,
            shift: Shift {
                x: 0.0,
                y: list_y_start - 0.045,
            },
            align: Align {
                horizontal: HorizontalAlign::Center,
                vertical: VerticalAlign::Top,
            },
            font_size: FontSize::WindowHeight(0.032),
            color: 0xFF_888888,
        });
    }

    for (visible_index, row) in settings_rows[start_index..end_index].iter().enumerate() {
        let row_index = start_index + visible_index;
        let is_selected = row_index == selected_index;
        let color = if is_selected {
            0xFF_FFFF00
        } else {
            0xFF_FFFFFF
        };
        let action_color = if is_selected {
            0xFF_FFFF00
        } else {
            0xFF_888888
        };
        let y = list_y_start + visible_index as f32 * row_step;
        let marker = if is_selected { ">" } else { " " };

        render_settings_cell(render_list, marker.to_string(), -0.43, y, color);
        render_settings_cell(
            render_list,
            fit_table_text_to_width(&row.label, label_max_width, fonts, cell_pixel_font_size),
            -0.35,
            y,
            color,
        );
        render_settings_cell(
            render_list,
            fit_table_text_to_width(&row.value, value_max_width, fonts, cell_pixel_font_size),
            0.05,
            y,
            color,
        );
        render_settings_cell(
            render_list,
            fit_table_text_to_width(row.action, action_max_width, fonts, cell_pixel_font_size),
            0.36,
            y,
            action_color,
        );
    }

    if end_index < total_rows {
        render_list.push(Renderable::Text {
            text: "↓".to_string(),
            anchor: Anchor::TopCenter,
            shift: Shift {
                x: 0.0,
                y: list_y_start + list_capacity as f32 * row_step,
            },
            align: Align {
                horizontal: HorizontalAlign::Center,
                vertical: VerticalAlign::Top,
            },
            font_size: FontSize::WindowHeight(0.032),
            color: 0xFF_888888,
        });
    }
}

struct SettingsTableRow {
    label: String,
    value: String,
    action: &'static str,
}

fn settings_table_rows(app: &App, snapshot: AppSnapshot<'_>) -> Vec<SettingsTableRow> {
    SettingsItem::all()
        .iter()
        .map(|item| settings_table_row(app, snapshot, *item))
        .collect()
}

fn settings_table_row(
    app: &App,
    snapshot: AppSnapshot<'_>,
    item: SettingsItem,
) -> SettingsTableRow {
    match item {
        SettingsItem::FontFamily(target) => SettingsTableRow {
            label: target.settings_label().to_string(),
            value: app.fonts().name_for_target(target).to_string(),
            action: "assign",
        },
        SettingsItem::FontScale(target) => SettingsTableRow {
            label: target.scale_settings_label().to_string(),
            value: app.fonts().scale_for_target(target).label().to_string(),
            action: "cycle",
        },
        SettingsItem::AspectRatio => SettingsTableRow {
            label: "Aspect Ratio".to_string(),
            value: snapshot.display_settings.aspect_ratio.label().to_string(),
            action: "cycle",
        },
        SettingsItem::DisplayScale => SettingsTableRow {
            label: "Display Scale".to_string(),
            value: snapshot.display_settings.scale.label().to_string(),
            action: "cycle",
        },
        SettingsItem::ImeInput => SettingsTableRow {
            label: "IME Input".to_string(),
            value: if app.accepts_ime_input() {
                "Enabled"
            } else {
                "Disabled"
            }
            .to_string(),
            action: "cycle",
        },
    }
}

fn render_settings_cell(
    render_list: &mut Vec<Renderable>,
    text: String,
    x: f32,
    y: f32,
    color: u32,
) {
    render_list.push(Renderable::Text {
        text,
        anchor: Anchor::TopCenter,
        shift: Shift { x, y },
        align: Align {
            horizontal: HorizontalAlign::Left,
            vertical: VerticalAlign::Top,
        },
        font_size: FontSize::WindowHeight(0.042),
        color,
    });
}

fn settings_cell_left_x(width: usize, shift_x: f32) -> f32 {
    width as f32 * (0.5 + shift_x)
}

fn fit_table_text_to_width(
    text: &str,
    max_width: f32,
    fonts: &Fonts,
    pixel_font_size: f32,
) -> String {
    if gui_renderer::measure_text(fonts.ui(), text, pixel_font_size).0 as f32 <= max_width {
        return text.to_string();
    }

    let ellipsis = "...";
    let ellipsis_width = gui_renderer::measure_text(fonts.ui(), ellipsis, pixel_font_size).0 as f32;
    if ellipsis_width > max_width {
        return String::new();
    }

    let mut result = String::new();
    for character in text.chars() {
        let mut candidate = result.clone();
        candidate.push(character);
        candidate.push_str(ellipsis);
        if gui_renderer::measure_text(fonts.ui(), &candidate, pixel_font_size).0 as f32 > max_width
        {
            break;
        }
        result.push(character);
    }
    result.push_str(ellipsis);
    result
}

fn build_font_picker_ui(
    app: &App,
    snapshot: AppSnapshot<'_>,
    render_list: &mut Vec<Renderable>,
    fonts: &Fonts,
    width: usize,
    height: usize,
) {
    let selected_target = app
        .selected_settings_item
        .font_target()
        .unwrap_or(FontTarget::Ui);

    render_list.push(Renderable::Text {
        text: format!(
            "{}: {}",
            selected_target.settings_label(),
            app.fonts().name_for_target(selected_target)
        ),
        anchor: Anchor::TopCenter,
        shift: Shift { x: 0.0, y: 0.32 },
        align: Align {
            horizontal: HorizontalAlign::Center,
            vertical: VerticalAlign::Top,
        },
        font_size: FontSize::WindowHeight(0.055),
        color: 0xFF_AADDFF,
    });

    let item_font_size = FontSize::WindowHeight(0.04);
    let item_height = measured_ui_row_step(
        fonts,
        item_font_size,
        width,
        height,
        snapshot.display_settings.scale.multiplier(),
        0.052,
    );
    let list_y_start = 0.43;
    let list_height = 0.45;
    let items_per_screen = (list_height / item_height) as usize;
    let font_count = app.available_fonts.len();
    let mut start_index = 0;
    if app.selected_font_item >= items_per_screen {
        start_index = app.selected_font_item - items_per_screen + 1;
    }
    let end_index = (start_index + items_per_screen).min(font_count);

    for index in start_index..end_index {
        let font = &app.available_fonts[index];
        let selected = index == app.selected_font_item;
        let source = match font.source {
            crate::app::FontSource::Bundled => "fonts",
            crate::app::FontSource::System => "system",
        };
        let marker = if selected { ">" } else { " " };
        let color = if selected { 0xFF_FFFF00 } else { 0xFF_FFFFFF };

        render_list.push(Renderable::Text {
            text: format!("{marker} [{}] {}", source, font.name),
            anchor: Anchor::TopCenter,
            shift: Shift {
                x: -0.26,
                y: list_y_start + (index - start_index) as f32 * item_height,
            },
            align: Align {
                horizontal: HorizontalAlign::Left,
                vertical: VerticalAlign::Top,
            },
            font_size: item_font_size,
            color,
        });
    }

    if font_count == 0 {
        render_list.push(Renderable::Text {
            text: "No fonts found".to_string(),
            anchor: Anchor::Center,
            shift: Shift { x: 0.0, y: 0.12 },
            align: Align {
                horizontal: HorizontalAlign::Center,
                vertical: VerticalAlign::Center,
            },
            font_size: FontSize::WindowHeight(0.045),
            color: 0xFF_FF8888,
        });
    }
}

fn problem_menu_title_line(app: &App, index: usize) -> Option<crate::model::Line> {
    let first_line = app.get_problem_source(index)?.lines().next()?;
    if !first_line.starts_with("#title") {
        return None;
    }

    let content = crate::parser::parse_problem(&format!("{first_line}\n_")).ok()?;
    (!content.title.words.is_empty()).then_some(content.title)
}

fn build_problem_selection_ui(
    app: &App,
    snapshot: AppSnapshot<'_>,
    render_list: &mut Vec<Renderable>,
    gradient: Gradient,
    fonts: &Fonts,
    width: usize,
    height: usize,
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

    let item_height: f32 = 0.07;
    let list_y_start: f32 = 0.38;
    let list_height: f32 = 0.52;
    let items_per_screen = (list_height / item_height).floor() as usize;
    let title_font_size = FontSize::WindowHeight(0.044);
    let title_pixel_font_size = calculate_pixel_font_size(title_font_size, width, height)
        * app.display_settings().scale.multiplier();

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
        let color = if selected {
            0xFF_FFFF00u32
        } else if is_open_file {
            0xFF_888888u32
        } else {
            0xFF_FFFFFF
        };
        let y_pos = list_y_start + ((i - start_index) as f32 * item_height);
        let marker = if selected { ">" } else { " " };

        render_list.push(Renderable::Text {
            text: format!("{marker}[{badge}]"),
            anchor: Anchor::TopCenter,
            shift: Shift { x: -0.34, y: y_pos },
            align: Align {
                horizontal: HorizontalAlign::Left,
                vertical: VerticalAlign::Top,
            },
            font_size: FontSize::WindowHeight(0.042),
            color,
        });

        if let Some(title_line) = problem_menu_title_line(app, i) {
            let title_segments = upper_segments_for_line(
                &title_line,
                if selected {
                    UpperSegmentState::Active
                } else {
                    UpperSegmentState::Pending
                },
                UpperRubyDisplay::Presentation,
            );
            if !title_segments.is_empty() {
                render_list.push(Renderable::TypingUpper {
                    segments: title_segments,
                    anchor: Anchor::TopCenter,
                    shift: Shift { x: -0.22, y: y_pos },
                    align: Align {
                        horizontal: HorizontalAlign::Left,
                        vertical: VerticalAlign::Top,
                    },
                    font_size: title_font_size,
                    line_alignment: TypingLineAlignment::full_line(measure_line_base_width(
                        &title_line,
                        fonts,
                        title_pixel_font_size,
                    )),
                });
                continue;
            }
        }

        render_list.push(Renderable::Text {
            text: item.to_string(),
            anchor: Anchor::TopCenter,
            shift: Shift { x: -0.22, y: y_pos },
            align: Align {
                horizontal: HorizontalAlign::Left,
                vertical: VerticalAlign::Top,
            },
            font_size: FontSize::WindowHeight(0.042),
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

fn script_for_plain_character(character: char, context: FontScript) -> FontScript {
    let mut buffer = [0_u8; 4];
    plain_text_script_runs(character.encode_utf8(&mut buffer), Some(context))
        .into_iter()
        .next()
        .map(|run| run.script)
        .unwrap_or(context)
}

fn script_for_active_character(
    segment: &Segment,
    context: FontScript,
    char_index: usize,
    character: char,
) -> FontScript {
    match segment {
        Segment::Plain { .. } => script_for_plain_character(character, context),
        Segment::Anno { .. } => {
            let mut offset = 0usize;
            for run in segment_script_runs(segment, context) {
                let run_len = run.reading_text.chars().count();
                if char_index < offset + run_len {
                    return run.script;
                }
                offset += run_len;
            }
            context
        }
        Segment::Annotated { .. } => context,
    }
}

fn script_for_input_feedback_character(
    segment: &Segment,
    context: FontScript,
    char_index: usize,
    character: char,
) -> FontScript {
    if matches!(segment, Segment::Plain { .. })
        && context != FontScript::English
        && (character.is_ascii_alphanumeric() || character.is_ascii_punctuation())
    {
        return context;
    }

    script_for_active_character(segment, context, char_index, character)
}

fn plain_base_runs(text: &str, context: FontScript) -> Vec<crate::font::TextScriptRun> {
    plain_text_script_runs(text, Some(context))
}

fn segment_display_runs(
    segment: &Segment,
    base_text: &str,
    ruby_text: Option<String>,
    script: FontScript,
) -> Vec<crate::font::SegmentScriptRun> {
    if matches!(segment, Segment::Anno { .. }) {
        return segment_script_runs(segment, script);
    }

    if matches!(segment, Segment::Plain { .. }) && ruby_text.is_none() {
        return plain_base_runs(base_text, script)
            .into_iter()
            .map(|run| crate::font::SegmentScriptRun {
                reading_text: run.text.clone(),
                base_text: run.text,
                ruby_text: None,
                script: run.script,
            })
            .collect();
    }

    Vec::from([crate::font::SegmentScriptRun {
        base_text: base_text.to_string(),
        reading_text: segment_reading_text(segment),
        ruby_text,
        script,
    }])
}

fn presentation_ruby_text(ruby_text: Option<String>, script: FontScript) -> Option<String> {
    let ruby_text = ruby_text?;
    if matches!(
        script,
        FontScript::ChineseSimplified | FontScript::TraditionalChinese
    ) {
        return pinyin::numbered_pinyin_to_tone_marks(&ruby_text).or(Some(ruby_text));
    }
    Some(ruby_text)
}

struct UpperTypingSegmentInput<'a> {
    source_segment: &'a Segment,
    base_text: String,
    ruby_text: Option<String>,
    anno_text: Option<String>,
    script: FontScript,
    state: UpperSegmentState,
    ruby_display: UpperRubyDisplay,
}

fn push_upper_typing_segment(
    segments: &mut Vec<UpperTypingSegment>,
    input: UpperTypingSegmentInput<'_>,
) {
    let UpperTypingSegmentInput {
        source_segment,
        base_text,
        ruby_text,
        anno_text,
        script,
        state,
        ruby_display,
    } = input;

    if matches!(source_segment, Segment::Plain { .. } | Segment::Anno { .. }) {
        let runs = segment_display_runs(source_segment, &base_text, ruby_text, script);
        let last_run_index = runs.len().saturating_sub(1);
        for (run_index, run) in runs.into_iter().enumerate() {
            let ruby_text = match ruby_display {
                UpperRubyDisplay::InputKeys => run.ruby_text,
                UpperRubyDisplay::Presentation => presentation_ruby_text(run.ruby_text, run.script),
            };
            let run_anno_text =
                if matches!(source_segment, Segment::Anno { .. }) && run_index == last_run_index {
                    anno_text.clone()
                } else {
                    None
                };
            let anno_group_run_count = if run_anno_text.is_some() {
                last_run_index + 1
            } else {
                0
            };
            let anno_script = run_anno_text.as_ref().map(|_| script);
            segments.push(UpperTypingSegment {
                base_text: run.base_text,
                ruby_text,
                anno_text: run_anno_text,
                anno_group_run_count,
                anno_script,
                script: run.script,
                state,
            });
        }
        return;
    }

    let ruby_text = match ruby_display {
        UpperRubyDisplay::InputKeys => ruby_text,
        UpperRubyDisplay::Presentation => presentation_ruby_text(ruby_text, script),
    };
    let anno_group_run_count = usize::from(anno_text.is_some());
    let anno_script = anno_text.as_ref().map(|_| script);
    segments.push(UpperTypingSegment {
        base_text,
        ruby_text,
        anno_text,
        anno_group_run_count,
        anno_script,
        script,
        state,
    });
}

fn upper_segments_for_line(
    line: &crate::model::Line,
    state: UpperSegmentState,
    ruby_display: UpperRubyDisplay,
) -> Vec<UpperTypingSegment> {
    let line_scripts = scripts_for_line(line);
    let mut segments = Vec::new();
    let mut segment_index = 0usize;

    for word in &line.words {
        for segment in &word.segments {
            let segment_script = line_scripts
                .get(segment_index)
                .copied()
                .unwrap_or_else(|| script_for_segment(segment));
            segment_index += 1;
            let (base_text, ruby_text, anno_text) = segment_display_parts(segment);
            push_upper_typing_segment(
                &mut segments,
                UpperTypingSegmentInput {
                    source_segment: segment,
                    base_text,
                    ruby_text,
                    anno_text,
                    script: segment_script,
                    state,
                    ruby_display,
                },
            );
        }
    }

    segments
}

fn push_lower_completed_segments(
    segments: &mut Vec<LowerTypingSegment>,
    fonts: &Fonts,
    source_segment: &Segment,
    display: (String, Option<String>, FontScript),
    is_correct: bool,
    font_size: f32,
) {
    let (base_text, ruby_text, script) = display;
    if matches!(source_segment, Segment::Plain { .. } | Segment::Anno { .. }) {
        let runs = segment_display_runs(source_segment, &base_text, ruby_text, script);
        let mut widths = runs
            .iter()
            .map(|run| measure_display_run_width(fonts, run, font_size))
            .collect::<Vec<_>>();
        if let Segment::Anno { annotation, .. } = source_segment {
            let run_width_total = widths.iter().copied().sum::<u32>();
            let annotation_width = measure_annotation_width(fonts, script, annotation, font_size);
            if let Some(last_width) = widths.last_mut() {
                if annotation_width > run_width_total {
                    *last_width += annotation_width - run_width_total;
                }
            }
        }
        for (run, width) in runs.into_iter().zip(widths) {
            segments.push(LowerTypingSegment::Completed {
                base_text: run.base_text,
                ruby_text: lower_completed_ruby_text(run.ruby_text, run.script),
                script: run.script,
                is_correct,
                width,
            });
        }
        return;
    }

    let run = crate::font::SegmentScriptRun {
        base_text: base_text.clone(),
        reading_text: segment_reading_text(source_segment),
        ruby_text: ruby_text.clone(),
        script,
    };
    let width = measure_display_run_width(fonts, &run, font_size);
    segments.push(LowerTypingSegment::Completed {
        base_text,
        ruby_text: lower_completed_ruby_text(ruby_text, script),
        script,
        is_correct,
        width,
    });
}

fn lower_completed_ruby_text(ruby_text: Option<String>, script: FontScript) -> Option<String> {
    let ruby_text = ruby_text?;
    if matches!(
        script,
        FontScript::ChineseSimplified | FontScript::TraditionalChinese
    ) {
        return pinyin::numbered_pinyin_to_tone_marks(&ruby_text).or(Some(ruby_text));
    }
    Some(ruby_text)
}

fn push_unconfirmed_input_elements(
    elements: &mut Vec<ActiveLowerElement>,
    source_segment: &Segment,
    text: String,
    context: FontScript,
) {
    let text = if matches!(
        context,
        FontScript::ChineseSimplified | FontScript::TraditionalChinese
    ) {
        pinyin::numbered_pinyin_to_tone_marks(&text).unwrap_or(text)
    } else {
        text
    };

    if matches!(source_segment, Segment::Plain { .. }) && context == FontScript::English {
        for run in plain_base_runs(&text, context) {
            elements.push(ActiveLowerElement::UnconfirmedInput {
                text: run.text,
                script: run.script,
            });
        }
    } else {
        elements.push(ActiveLowerElement::UnconfirmedInput {
            text,
            script: context,
        });
    }
}

fn is_word_correct(word: &TypingCorrectnessWord) -> bool {
    word.segments.iter().all(is_segment_correct)
}

fn is_segment_correct(segment: &TypingCorrectnessSegment) -> bool {
    !segment.chars.contains(&TypingCorrectnessChar::Incorrect)
}

fn measure_line_base_width(line: &crate::model::Line, fonts: &Fonts, font_size: f32) -> u32 {
    let scripts = scripts_for_line(line);
    let mut segment_index = 0usize;
    let mut total = 0;

    line.words
        .iter()
        .flat_map(|word| &word.segments)
        .for_each(|segment| {
            let (base_text, ruby_text, anno_text) = segment_display_parts(segment);
            let script = scripts
                .get(segment_index)
                .copied()
                .unwrap_or_else(|| script_for_segment(segment));
            segment_index += 1;
            let mut segment_width = 0;
            for run in segment_display_runs(segment, &base_text, ruby_text, script) {
                segment_width += measure_display_run_width(fonts, &run, font_size);
            }
            if let Some(anno_text) = anno_text {
                let anno_font_size = fonts.scaled_size_for_ruby_script(script, font_size * 0.3);
                let anno_width = gui_renderer::measure_text(
                    fonts.get_ruby_for_script(script),
                    &anno_text,
                    anno_font_size,
                )
                .0;
                segment_width = segment_width.max(anno_width);
            }
            total += segment_width;
        });

    total
}

fn measure_annotation_width(
    fonts: &Fonts,
    script: FontScript,
    annotation: &str,
    font_size: f32,
) -> u32 {
    let anno_font_size = fonts.scaled_size_for_ruby_script(script, font_size * 0.3);
    gui_renderer::measure_text(
        fonts.get_ruby_for_script(script),
        annotation,
        anno_font_size,
    )
    .0
}

fn measure_display_run_width(
    fonts: &Fonts,
    run: &crate::font::SegmentScriptRun,
    font_size: f32,
) -> u32 {
    let base_font_size = fonts.scaled_size_for_script(run.script, font_size);
    let base_width = gui_renderer::measure_text(
        fonts.get_for_script(run.script),
        &run.base_text,
        base_font_size,
    )
    .0;
    let ruby_width = run.ruby_text.as_deref().map_or(0, |ruby| {
        let ruby_font_size = fonts.scaled_size_for_ruby_script(run.script, font_size * 0.4);
        gui_renderer::measure_text(fonts.get_ruby_for_script(run.script), ruby, ruby_font_size).0
    });

    base_width.max(ruby_width)
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

fn flat_segment_index_at(
    line: &crate::model::Line,
    word_index: usize,
    segment_index: usize,
) -> usize {
    line.words
        .iter()
        .take(word_index)
        .map(|word| word.segments.len())
        .sum::<usize>()
        + segment_index
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
        let title_font_size = FontSize::WindowHeight(TYPING_TITLE_FONT_SIZE_RATIO);
        let title_pixel_font_size = calculate_pixel_font_size(title_font_size, width, height)
            * app.display_settings().scale.multiplier();
        let title_segments = upper_segments_for_line(
            &model.content.title,
            UpperSegmentState::Active,
            UpperRubyDisplay::Presentation,
        );
        let title_line_width =
            measure_line_base_width(&model.content.title, fonts, title_pixel_font_size);
        let title_vertical_metrics =
            upper_typing_vertical_metrics(&title_segments, fonts, title_pixel_font_size);

        let display_scale = app.display_settings().scale.multiplier();
        let base_font_size = FontSize::WindowHeight(BASE_FONT_SIZE_RATIO);
        let base_pixel_font_size =
            calculate_pixel_font_size(base_font_size, width, height) * display_scale;
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
        let line_scripts = scripts_for_line(content_line);
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

                let segment_script = line_scripts
                    .get(segment_index)
                    .copied()
                    .unwrap_or_else(|| script_for_segment(seg));
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
                                    anno_group_run_count: 0,
                                    anno_script: None,
                                    script: if matches!(seg, Segment::Plain { .. }) {
                                        script_for_plain_character(ch, segment_script)
                                    } else {
                                        segment_script
                                    },
                                    state,
                                });
                            }
                            continue;
                        }
                    }
                }
                push_upper_typing_segment(
                    &mut upper_segments,
                    UpperTypingSegmentInput {
                        source_segment: seg,
                        base_text,
                        ruby_text,
                        anno_text,
                        script: segment_script,
                        state,
                        ruby_display: UpperRubyDisplay::InputKeys,
                    },
                );
            }
        }

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

                    if cache_seg.display_runs.len() == 1 {
                        lower_segments.push(LowerTypingSegment::Completed {
                            base_text: cache_seg.display_runs[0].base_text.clone(),
                            ruby_text: lower_completed_ruby_text(
                                cache_seg.display_runs[0].ruby_text.clone(),
                                cache_seg.display_runs[0].script,
                            ),
                            script: cache_seg.display_runs[0].script,
                            is_correct,
                            width: cache_seg.base_width as u32,
                        });
                    } else {
                        let mut run_widths = cache_seg
                            .display_runs
                            .iter()
                            .map(|run| measure_display_run_width(fonts, run, base_pixel_font_size))
                            .collect::<Vec<_>>();
                        let run_width_total = run_widths.iter().copied().sum::<u32>();
                        if let Some(last_width) = run_widths.last_mut() {
                            let segment_width = cache_seg.base_width as u32;
                            if segment_width > run_width_total {
                                *last_width += segment_width - run_width_total;
                            }
                        }
                        for (run, width) in cache_seg.display_runs.iter().zip(run_widths) {
                            lower_segments.push(LowerTypingSegment::Completed {
                                base_text: run.base_text.clone(),
                                ruby_text: lower_completed_ruby_text(
                                    run.ruby_text.clone(),
                                    run.script,
                                ),
                                script: run.script,
                                is_correct,
                                width,
                            });
                        }
                    }
                }
            }

            if let Some(active_word_content) = content_line.words.get(status_word) {
                let active_word_idx = status_word;
                if active_word_idx < correctness_line.words.len() {
                    let active_correctness_word = &correctness_line.words[active_word_idx];
                    if let Some(active_seg_content) =
                        active_word_content.segments.get(status_segment)
                    {
                        let active_script = line_scripts
                            .get(flat_segment_index_at(
                                content_line,
                                status_word,
                                status_segment,
                            ))
                            .copied()
                            .unwrap_or_else(|| script_for_segment(active_seg_content));
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
                                script: script_for_active_character(
                                    active_seg_content,
                                    active_script,
                                    char_idx,
                                    character,
                                ),
                            });
                        }

                        if !status.unconfirmed.is_empty() {
                            let unconfirmed_text: String = status.unconfirmed.iter().collect();
                            push_unconfirmed_input_elements(
                                &mut active_elements,
                                active_seg_content,
                                unconfirmed_text,
                                active_script,
                            );
                        }

                        if let Some(wrong_char) = status.last_wrong_keydown {
                            active_elements.push(ActiveLowerElement::LastIncorrectInput {
                                character: wrong_char,
                                script: script_for_input_feedback_character(
                                    active_seg_content,
                                    active_script,
                                    status.char_.get(),
                                    wrong_char,
                                ),
                            });
                        } else {
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
                    for (seg_idx, seg) in word.segments.iter().enumerate() {
                        let script = line_scripts
                            .get(flat_segment_index_at(content_line, word_idx, seg_idx))
                            .copied()
                            .unwrap_or_else(|| script_for_segment(seg));
                        let (base_text, ruby_text, _) = segment_display_parts(seg);
                        push_lower_completed_segments(
                            &mut lower_segments,
                            fonts,
                            seg,
                            (base_text, ruby_text, script),
                            is_word_correct(correctness_word),
                            base_pixel_font_size,
                        );
                    }
                }
            }

            if let (Some(active_word_content), Some(active_correctness_word)) = (
                content_line.words.get(status_word),
                correctness_line.words.get(status_word),
            ) {
                for seg_idx in 0..status_segment {
                    if let Some(seg) = active_word_content.segments.get(seg_idx) {
                        let script = line_scripts
                            .get(flat_segment_index_at(content_line, status_word, seg_idx))
                            .copied()
                            .unwrap_or_else(|| script_for_segment(seg));
                        let (base_text, ruby_text, _) = segment_display_parts(seg);
                        let is_correct = active_correctness_word
                            .segments
                            .get(seg_idx)
                            .is_some_and(is_segment_correct);
                        push_lower_completed_segments(
                            &mut lower_segments,
                            fonts,
                            seg,
                            (base_text, ruby_text, script),
                            is_correct,
                            base_pixel_font_size,
                        );
                    }
                }

                if let Some(active_seg_content) = active_word_content.segments.get(status_segment) {
                    let active_script = line_scripts
                        .get(flat_segment_index_at(
                            content_line,
                            status_word,
                            status_segment,
                        ))
                        .copied()
                        .unwrap_or_else(|| script_for_segment(active_seg_content));
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
                            script: script_for_active_character(
                                active_seg_content,
                                active_script,
                                char_idx,
                                character,
                            ),
                        });
                    }

                    if !status.unconfirmed.is_empty() {
                        let unconfirmed_text: String = status.unconfirmed.iter().collect();
                        push_unconfirmed_input_elements(
                            &mut active_elements,
                            active_seg_content,
                            unconfirmed_text,
                            active_script,
                        );
                    }

                    if let Some(wrong_char) = status.last_wrong_keydown {
                        active_elements.push(ActiveLowerElement::LastIncorrectInput {
                            character: wrong_char,
                            script: script_for_input_feedback_character(
                                active_seg_content,
                                active_script,
                                status.char_.get(),
                                wrong_char,
                            ),
                        });
                    } else {
                        active_elements.push(ActiveLowerElement::Cursor);
                    }

                    lower_segments.push(LowerTypingSegment::Active {
                        elements: active_elements,
                        script: active_script,
                    });
                }
            }
        }

        let upper_vertical_metrics =
            upper_typing_vertical_metrics(&upper_segments, fonts, base_pixel_font_size);
        let lower_vertical_metrics =
            lower_typing_vertical_metrics(&lower_segments, fonts, base_pixel_font_size);
        let core_layout =
            typing_core_vertical_layout(height, upper_vertical_metrics, lower_vertical_metrics);

        let line_count = model.content.lines.len();
        let context_font_size = FontSize::WindowHeight(TYPING_CONTEXT_FONT_SIZE_RATIO);
        let context_pixel_font_size =
            calculate_pixel_font_size(context_font_size, width, height) * display_scale;
        let float_gap = height.max(1) as f32 * TYPING_FLOAT_MIN_GAP_RATIO;
        let top_margin = height.max(1) as f32 * TYPING_FLOAT_TOP_MARGIN_RATIO;

        let mut previous_context_top = None;
        let mut previous_context_renderable = None;
        let previous_line_to_display = model.status.line.get() as isize - 1;
        if previous_line_to_display >= 0 {
            let line_idx_context = previous_line_to_display as usize;
            if let Some(context_line) = model.content.lines.get(line_idx_context) {
                let segments = upper_segments_for_line(
                    context_line,
                    UpperSegmentState::Muted,
                    UpperRubyDisplay::Presentation,
                );
                let metrics =
                    upper_typing_vertical_metrics(&segments, fonts, context_pixel_font_size);
                let top = core_layout.top
                    - float_gap
                    - metrics.total_height()
                    - TYPING_PREVIOUS_CONTEXT_ROUNDING_GUARD_PX;
                if top >= top_margin {
                    previous_context_top = Some(top);
                    previous_context_renderable = Some(Renderable::TypingUpper {
                        segments,
                        anchor: Anchor::Center,
                        shift: Shift {
                            x: 0.0,
                            y: center_anchor_top_shift_y_for_box_top(top, metrics, height),
                        },
                        align: Align {
                            horizontal: HorizontalAlign::Center,
                            vertical: VerticalAlign::Top,
                        },
                        font_size: context_font_size,
                        line_alignment: TypingLineAlignment::full_line(measure_line_base_width(
                            context_line,
                            fonts,
                            context_pixel_font_size,
                        )),
                    });
                }
            }
        }

        if !title_segments.is_empty() {
            let title_limit = previous_context_top.unwrap_or(core_layout.top);
            let title_height = title_vertical_metrics.total_height();
            let title_top = top_margin.min(title_limit - title_height - 4.0).max(0.0);
            if title_top + title_height + 4.0 <= title_limit {
                render_list.push(Renderable::TypingUpper {
                    segments: title_segments,
                    anchor: Anchor::TopCenter,
                    shift: Shift {
                        x: 0.0,
                        y: top_anchor_shift_y_for_box_top(
                            title_top,
                            title_vertical_metrics,
                            height,
                        ),
                    },
                    align: Align {
                        horizontal: HorizontalAlign::Center,
                        vertical: VerticalAlign::Top,
                    },
                    font_size: title_font_size,
                    line_alignment: TypingLineAlignment::full_line(title_line_width),
                });
            }
        }

        if let Some(renderable) = previous_context_renderable {
            render_list.push(renderable);
        }

        render_list.push(Renderable::TypingUpper {
            segments: upper_segments,
            anchor: Anchor::Center,
            shift: Shift {
                x: line_shift_x,
                y: core_layout.upper_shift_y,
            },
            align: Align {
                horizontal: HorizontalAlign::Left,
                vertical: VerticalAlign::Center,
            },
            font_size: base_font_size,
            line_alignment: TypingLineAlignment::new(full_line_width, upper_visible_start_width),
        });

        render_list.push(Renderable::TypingLower {
            segments: lower_segments,
            anchor: Anchor::Center,
            shift: Shift {
                x: line_shift_x,
                y: core_layout.lower_shift_y,
            },
            align: Align {
                horizontal: HorizontalAlign::Left,
                vertical: VerticalAlign::Top,
            },
            font_size: base_font_size,
            line_alignment: TypingLineAlignment::new(full_line_width, lower_visible_start_width),
        });

        let metrics = typing::calculate_total_metrics(model);
        let status_rows = typing_status_rows(model, line_count, metrics);
        let status_row_step = measured_ui_row_step(
            fonts,
            FontSize::WindowHeight(TYPING_STATUS_ITEM_HEIGHT_RATIO),
            width,
            height,
            display_scale,
            TYPING_STATUS_ITEM_HEIGHT_RATIO,
        );
        let next_line_to_display = model.status.line.get() + 1;
        if let Some(context_line) = model.content.lines.get(next_line_to_display) {
            let segments = upper_segments_for_line(
                context_line,
                UpperSegmentState::Muted,
                UpperRubyDisplay::Presentation,
            );
            let metrics = upper_typing_vertical_metrics(&segments, fonts, context_pixel_font_size);
            let left = status_float_right_edge(fonts, &status_rows, width, height, display_scale);
            if left < width.max(1) as f32 {
                let top = typing_status_text_block_top(height, status_rows.len(), status_row_step);
                render_list.push(Renderable::TypingUpper {
                    segments,
                    anchor: Anchor::TopLeft,
                    shift: Shift {
                        x: left / width.max(1) as f32,
                        y: top_anchor_shift_y_for_box_top(top, metrics, height),
                    },
                    align: Align {
                        horizontal: HorizontalAlign::Left,
                        vertical: VerticalAlign::Top,
                    },
                    font_size: context_font_size,
                    line_alignment: TypingLineAlignment::full_line(measure_line_base_width(
                        context_line,
                        fonts,
                        context_pixel_font_size,
                    )),
                });
            }
        }

        let status_layout = status_table_layout(fonts, &status_rows, width, height, display_scale);
        for (i, row) in status_rows.iter().enumerate() {
            let row_y = -TYPING_STATUS_BOTTOM_MARGIN_RATIO
                - TYPING_STATUS_PROGRESS_BAR_HEIGHT_RATIO
                - ((status_rows.len() - 1 - i) as f32 * status_row_step);
            render_list.push(Renderable::Text {
                text: row.label.to_string(),
                anchor: Anchor::BottomLeft,
                shift: Shift {
                    x: status_layout.label_right / width.max(1) as f32,
                    y: row_y,
                },
                align: Align {
                    horizontal: HorizontalAlign::Right,
                    vertical: VerticalAlign::Bottom,
                },
                font_size: FontSize::WindowHeight(TYPING_STATUS_ITEM_HEIGHT_RATIO),
                color: 0xFF_AAAAAA,
            });
            render_list.push(Renderable::Text {
                text: row.value.clone(),
                anchor: Anchor::BottomLeft,
                shift: Shift {
                    x: status_layout.value_left / width.max(1) as f32,
                    y: row_y,
                },
                align: Align {
                    horizontal: HorizontalAlign::Left,
                    vertical: VerticalAlign::Bottom,
                },
                font_size: FontSize::WindowHeight(TYPING_STATUS_ITEM_HEIGHT_RATIO),
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
            height_ratio: TYPING_STATUS_PROGRESS_BAR_HEIGHT_RATIO,
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
    use crate::app::{App, AppEvent, FontBundle, FontScale, FontTarget, Fonts, SettingsItem};
    use crate::display::DisplayScale;
    use crate::font::FontScript;
    use crate::io::{FontAssetId, FontEntry, FontSource};
    use ab_glyph::FontVec;

    fn test_fonts() -> Fonts {
        fn font() -> FontVec {
            FontVec::try_from_vec(include_bytes!("../fonts/YujiSyuku-Regular.ttf").to_vec())
                .expect("test font should parse")
        }

        Fonts::new(FontBundle {
            ui: font(),
            japanese: font(),
            japanese_ruby: font(),
            japanese_unconfirmed: font(),
            chinese_simplified: font(),
            chinese_simplified_ruby: font(),
            chinese_simplified_unconfirmed: font(),
            traditional_chinese: font(),
            traditional_chinese_ruby: font(),
            traditional_chinese_unconfirmed: font(),
            english: font(),
        })
    }

    fn typing_app(problem: &str) -> App {
        let mut app = App::new(test_fonts());
        app.add_custom_problem("plain".to_string(), problem.to_string(), 0);
        app.on_event(AppEvent::Enter);
        app
    }

    #[test]
    #[ignore = "manual frame-time probe"]
    fn perf_typing_frame_profile() {
        use crate::renderer::{ArgbSurface, RenderCache};
        use std::time::Instant;

        let problem = concat!(
            "#title [春晓/chun1xiao3]\n",
            "[春眠/chun1mian2][不觉/bu4jue2][晓/xiao3]，[处处/chu4chu4][闻/wen2][啼/ti2][鸟/niao3]。\n",
            "[夜来/ye4lai2][风雨/feng1yu3][声/sheng1]，[花落/hua1luo4][知/zhi1][多少/duo1shao3]。\n",
            "[いろは/いろは][にほへと/にほへと][散りぬる/ちりぬる][を/を]\n",
            "[The/The][quick/quick][brown/brown][fox/fox][jumps/jumps][over/over][typingmp/typingmp].\n",
            "[春眠/chun1mian2][不觉/bu4jue2][晓/xiao3]，[处处/chu4chu4][闻/wen2][啼/ti2][鸟/niao3]。\n",
            "[いろは/いろは][にほへと/にほへと][散りぬる/ちりぬる][を/を]\n",
        );
        let mut app = typing_app(problem);
        let width = 1280;
        let height = 720;
        let mut render_cache = RenderCache::new();
        let mut pixels = vec![0u32; width * height];
        app.update(width, height, 16.0);

        let frames = 120;
        let mut update_time = std::time::Duration::ZERO;
        let mut build_time = std::time::Duration::ZERO;
        let mut render_time = std::time::Duration::ZERO;
        let start = Instant::now();
        for _ in 0..frames {
            let update_start = Instant::now();
            app.update(width, height, 16.0);
            update_time += update_start.elapsed();

            let build_start = Instant::now();
            let render_list = build_ui(&app, app.fonts(), width, height);
            build_time += build_start.elapsed();

            let render_start = Instant::now();
            let mut surface =
                ArgbSurface::new(width, height, &mut pixels).expect("surface should be valid");
            surface.render(
                app.fonts(),
                app.display_settings(),
                &render_list,
                &mut render_cache,
            );
            render_time += render_start.elapsed();
        }
        let elapsed = start.elapsed();
        println!(
            "typing frame profile: {} frames in {:?}, {:.3} ms/frame; update {:.3}, build {:.3}, render {:.3} ms/frame",
            frames,
            elapsed,
            elapsed.as_secs_f64() * 1000.0 / f64::from(frames),
            update_time.as_secs_f64() * 1000.0 / f64::from(frames),
            build_time.as_secs_f64() * 1000.0 / f64::from(frames),
            render_time.as_secs_f64() * 1000.0 / f64::from(frames)
        );
    }

    #[test]
    #[ignore = "manual changing-frame probe"]
    fn perf_typing_input_frame_profile() {
        use crate::renderer::{ArgbSurface, RenderCache};
        use std::time::Instant;

        let input = "abcdefghijklmnopqrstuvwxyz".repeat(16);
        let problem = format!("#title Perf\n[{input}/{input}]");
        let mut app = typing_app(&problem);
        let width = 1280;
        let height = 720;
        let mut render_cache = RenderCache::new();
        let mut pixels = vec![0u32; width * height];
        app.update(width, height, 16.0);

        let frames = 180u32;
        let input_chars = input.chars().collect::<Vec<_>>();
        let mut update_time = std::time::Duration::ZERO;
        let mut input_time = std::time::Duration::ZERO;
        let mut build_time = std::time::Duration::ZERO;
        let mut render_time = std::time::Duration::ZERO;
        let start = Instant::now();
        for frame in 0..frames {
            let input_start = Instant::now();
            app.on_event(AppEvent::Char {
                c: input_chars[frame as usize % input_chars.len()],
                timestamp: frame as f64 * 16.0,
            });
            input_time += input_start.elapsed();

            let update_start = Instant::now();
            app.update(width, height, 16.0);
            update_time += update_start.elapsed();

            let build_start = Instant::now();
            let render_list = build_ui(&app, app.fonts(), width, height);
            build_time += build_start.elapsed();

            let render_start = Instant::now();
            let mut surface =
                ArgbSurface::new(width, height, &mut pixels).expect("surface should be valid");
            surface.render(
                app.fonts(),
                app.display_settings(),
                &render_list,
                &mut render_cache,
            );
            render_time += render_start.elapsed();
        }
        let elapsed = start.elapsed();
        println!(
            "typing input profile: {} frames in {:?}, {:.3} ms/frame; input {:.3}, update {:.3}, build {:.3}, render {:.3} ms/frame",
            frames,
            elapsed,
            elapsed.as_secs_f64() * 1000.0 / f64::from(frames),
            input_time.as_secs_f64() * 1000.0 / f64::from(frames),
            update_time.as_secs_f64() * 1000.0 / f64::from(frames),
            build_time.as_secs_f64() * 1000.0 / f64::from(frames),
            render_time.as_secs_f64() * 1000.0 / f64::from(frames)
        );
    }

    fn typing_rows(render_list: &[Renderable]) -> (&[UpperTypingSegment], &[LowerTypingSegment]) {
        let upper_segments = render_list
            .iter()
            .find_map(|item| match item {
                Renderable::TypingUpper {
                    segments,
                    anchor: Anchor::Center,
                    align:
                        Align {
                            horizontal: HorizontalAlign::Left,
                            ..
                        },
                    ..
                } => Some(segments.as_slice()),
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
                Renderable::TypingUpper {
                    anchor: Anchor::Center,
                    align:
                        Align {
                            horizontal: HorizontalAlign::Left,
                            ..
                        },
                    line_alignment,
                    ..
                } => Some(*line_alignment),
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
                Renderable::TypingUpper {
                    anchor: Anchor::Center,
                    align:
                        Align {
                            horizontal: HorizontalAlign::Left,
                            ..
                        },
                    line_alignment,
                    ..
                } => Some(line_alignment.full_line_width),
                _ => None,
            })
            .expect("typing upper renderable should exist")
    }

    fn render_texts(render_list: &[Renderable]) -> Vec<&str> {
        render_list
            .iter()
            .filter_map(|item| match item {
                Renderable::Text { text, .. } | Renderable::BigText { text, .. } => {
                    Some(text.as_str())
                }
                _ => None,
            })
            .collect()
    }

    fn texts_have_prefix(texts: &[&str], prefix: &str) -> bool {
        texts.iter().any(|text| text.starts_with(prefix))
    }

    fn title_segments(render_list: &[Renderable]) -> &[UpperTypingSegment] {
        render_list
            .iter()
            .find_map(|item| match item {
                Renderable::TypingUpper {
                    segments,
                    anchor: Anchor::TopCenter,
                    ..
                } => Some(segments.as_slice()),
                _ => None,
            })
            .expect("typing title renderable should exist")
    }

    fn context_segments(render_list: &[Renderable]) -> Vec<&[UpperTypingSegment]> {
        render_list
            .iter()
            .filter_map(|item| match item {
                Renderable::TypingUpper { segments, .. }
                    if segments
                        .iter()
                        .all(|segment| segment.state == UpperSegmentState::Muted) =>
                {
                    Some(segments.as_slice())
                }
                _ => None,
            })
            .collect()
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

    #[derive(Debug)]
    struct VerticalTextBox {
        name: &'static str,
        top: f32,
        bottom: f32,
    }

    fn upper_renderable_vertical_box(
        item: &Renderable,
        fonts: &Fonts,
        width: usize,
        height: usize,
        display_scale: f32,
        name: &'static str,
    ) -> Option<VerticalTextBox> {
        let Renderable::TypingUpper {
            segments,
            anchor,
            shift,
            align,
            font_size,
            line_alignment,
        } = item
        else {
            return None;
        };

        let pixel_font_size = calculate_pixel_font_size(*font_size, width, height) * display_scale;
        let metrics = upper_typing_vertical_metrics(segments, fonts, pixel_font_size);
        let total_height = metrics.total_height().ceil() as u32;
        let anchor_pos = calculate_anchor_position(*anchor, *shift, width, height);
        let (_, y) = calculate_aligned_position(
            anchor_pos,
            line_alignment.full_line_width,
            total_height,
            *align,
        );
        let base_y = y as f32;
        let top = base_y - metrics.top_extra;
        let bottom = base_y + metrics.base_height + metrics.bottom_extra;

        Some(VerticalTextBox { name, top, bottom })
    }

    fn lower_renderable_vertical_box(
        item: &Renderable,
        fonts: &Fonts,
        width: usize,
        height: usize,
        display_scale: f32,
        name: &'static str,
    ) -> Option<VerticalTextBox> {
        let Renderable::TypingLower {
            segments,
            anchor,
            shift,
            align,
            font_size,
            line_alignment,
        } = item
        else {
            return None;
        };

        let pixel_font_size = calculate_pixel_font_size(*font_size, width, height) * display_scale;
        let metrics = lower_typing_vertical_metrics(segments, fonts, pixel_font_size);
        let total_height = metrics.total_height().ceil() as u32;
        let anchor_pos = calculate_anchor_position(*anchor, *shift, width, height);
        let (_, y) = calculate_aligned_position(
            anchor_pos,
            line_alignment.full_line_width,
            total_height,
            *align,
        );
        let base_y = y as f32;
        let top = base_y - metrics.top_extra;
        let bottom = base_y + metrics.base_height + metrics.bottom_extra;

        Some(VerticalTextBox { name, top, bottom })
    }

    fn play_typing_text_boxes(
        render_list: &[Renderable],
        fonts: &Fonts,
        width: usize,
        height: usize,
        display_scale: f32,
    ) -> Vec<VerticalTextBox> {
        render_list
            .iter()
            .filter_map(|item| match item {
                Renderable::TypingUpper {
                    anchor: Anchor::TopCenter,
                    ..
                } => upper_renderable_vertical_box(
                    item,
                    fonts,
                    width,
                    height,
                    display_scale,
                    "title",
                ),
                Renderable::TypingUpper {
                    anchor: Anchor::Center,
                    shift,
                    align:
                        Align {
                            horizontal: HorizontalAlign::Center,
                            ..
                        },
                    ..
                } if shift.y < 0.0 => upper_renderable_vertical_box(
                    item,
                    fonts,
                    width,
                    height,
                    display_scale,
                    "previous_context",
                ),
                Renderable::TypingUpper {
                    anchor: Anchor::Center,
                    shift,
                    align:
                        Align {
                            horizontal: HorizontalAlign::Center,
                            ..
                        },
                    ..
                } if shift.y > 0.0 => upper_renderable_vertical_box(
                    item,
                    fonts,
                    width,
                    height,
                    display_scale,
                    "next_context",
                ),
                Renderable::TypingUpper {
                    anchor: Anchor::TopLeft,
                    segments,
                    ..
                } if segments
                    .iter()
                    .all(|segment| segment.state == UpperSegmentState::Muted) =>
                {
                    upper_renderable_vertical_box(
                        item,
                        fonts,
                        width,
                        height,
                        display_scale,
                        "next_context",
                    )
                }
                Renderable::TypingUpper {
                    anchor: Anchor::Center,
                    align:
                        Align {
                            horizontal: HorizontalAlign::Left,
                            ..
                        },
                    ..
                } => upper_renderable_vertical_box(
                    item,
                    fonts,
                    width,
                    height,
                    display_scale,
                    "current_upper",
                ),
                Renderable::TypingLower { .. } => lower_renderable_vertical_box(
                    item,
                    fonts,
                    width,
                    height,
                    display_scale,
                    "current_lower",
                ),
                _ => None,
            })
            .collect()
    }

    fn find_box<'a>(boxes: &'a [VerticalTextBox], name: &str) -> &'a VerticalTextBox {
        boxes
            .iter()
            .find(|text_box| text_box.name == name)
            .unwrap_or_else(|| panic!("{name} vertical text box should exist: {boxes:#?}"))
    }

    fn find_optional_box<'a>(
        boxes: &'a [VerticalTextBox],
        name: &str,
    ) -> Option<&'a VerticalTextBox> {
        boxes.iter().find(|text_box| text_box.name == name)
    }

    fn assert_no_vertical_text_collisions(boxes: &[VerticalTextBox], min_gap: f32) {
        let mut sorted: Vec<&VerticalTextBox> = boxes
            .iter()
            .filter(|box_| box_.name != "next_context")
            .collect();
        sorted.sort_by(|a, b| a.top.total_cmp(&b.top));

        for pair in sorted.windows(2) {
            let upper = pair[0];
            let lower = pair[1];
            assert!(
                upper.bottom + min_gap <= lower.top,
                "text boxes overlap or are too close: upper={upper:?}, lower={lower:?}, all boxes={boxes:#?}"
            );
        }
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
                    is_correct: true,
                    ..
                },
                ActiveLowerElement::Typed {
                    character: 'は',
                    is_correct: true,
                    ..
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
        let app = typing_app("#title Test\n[色/いろ][字/zi4][字/ㄗˋ]");

        let render_list = build_ui(&app, app.fonts(), 800, 500);
        let (upper_segments, lower_segments) = typing_rows(&render_list);

        assert_eq!(upper_segments[0].script, FontScript::Japanese);
        assert_eq!(upper_segments[1].script, FontScript::ChineseSimplified);
        assert_eq!(upper_segments[2].script, FontScript::TraditionalChinese);

        let LowerTypingSegment::Active { script, .. } = &lower_segments[0] else {
            panic!("first lower segment should be active");
        };
        assert_eq!(*script, FontScript::Japanese);
    }

    #[test]
    fn unconfirmed_latin_feedback_uses_problem_language_context() {
        let japanese_segment = Segment::Plain {
            text: "\u{65e5}\u{672c}\u{8a9e}".to_string(),
        };
        let english_segment = Segment::Plain {
            text: "English Letter".to_string(),
        };

        let mut elements = Vec::new();
        push_unconfirmed_input_elements(
            &mut elements,
            &japanese_segment,
            "mikakuteimojiretu".to_string(),
            FontScript::Japanese,
        );
        match elements.as_slice() {
            [ActiveLowerElement::UnconfirmedInput { text, script }] => {
                assert_eq!(text, "mikakuteimojiretu");
                assert_eq!(*script, FontScript::Japanese);
            }
            _ => panic!("Japanese unconfirmed feedback should be a single Japanese-font run"),
        }

        elements.clear();
        push_unconfirmed_input_elements(
            &mut elements,
            &english_segment,
            "English".to_string(),
            FontScript::English,
        );
        match elements.as_slice() {
            [ActiveLowerElement::UnconfirmedInput { text, script }] => {
                assert_eq!(text, "English");
                assert_eq!(*script, FontScript::English);
            }
            _ => panic!("English unconfirmed feedback should remain English-font text"),
        }

        assert_eq!(
            script_for_input_feedback_character(&japanese_segment, FontScript::Japanese, 0, 'm'),
            FontScript::Japanese
        );
    }

    #[test]
    fn typing_title_preserves_ruby_base_and_script() {
        let app = typing_app("#title [\u{6625}\u{6653}/chun1xiao3]\n[\u{6625}\u{7720}/chun1mian2]");

        let render_list = build_ui(&app, app.fonts(), 800, 500);
        let title = title_segments(&render_list);

        assert_eq!(title[0].base_text, "\u{6625}\u{6653}");
        assert_eq!(
            title[0].ruby_text.as_deref(),
            Some("ch\u{016b}nxi\u{01ce}o")
        );
        assert_eq!(title[0].script, FontScript::ChineseSimplified);
    }

    #[test]
    fn active_chinese_upper_keeps_numbered_pinyin_keys() {
        let app = typing_app("#title Test\n[\u{6709}/you3]");

        let render_list = build_ui(&app, app.fonts(), 800, 500);
        let (upper_segments, _) = typing_rows(&render_list);

        assert_eq!(upper_segments[0].base_text, "\u{6709}");
        assert_eq!(upper_segments[0].ruby_text.as_deref(), Some("you3"));
        assert_eq!(upper_segments[0].script, FontScript::ChineseSimplified);
    }

    #[test]
    fn unconfirmed_chinese_pinyin_feedback_uses_tone_marks() {
        let segment = Segment::Annotated {
            base: "\u{6709}".to_string(),
            reading: "you3".to_string(),
        };
        let mut elements = Vec::new();

        push_unconfirmed_input_elements(
            &mut elements,
            &segment,
            "you3".to_string(),
            FontScript::ChineseSimplified,
        );

        match elements.as_slice() {
            [ActiveLowerElement::UnconfirmedInput { text, script }] => {
                assert_eq!(text, "y\u{01d2}u");
                assert_eq!(*script, FontScript::ChineseSimplified);
            }
            _ => panic!("Chinese unconfirmed feedback should be a tone-marked Chinese run"),
        }
    }

    #[test]
    fn active_chinese_pinyin_typed_feedback_uses_unconfirmed_font_role() {
        let mut app = typing_app("#title Test\n[\u{6709}/you3]");
        app.on_event(AppEvent::Char {
            c: 'y',
            timestamp: 1.0,
        });

        let render_list = build_ui(&app, app.fonts(), 800, 500);
        let (_, lower_segments) = typing_rows(&render_list);
        let LowerTypingSegment::Active { elements, .. } = &lower_segments[0] else {
            panic!("lower row should contain the active Chinese pinyin segment");
        };

        match elements.as_slice() {
            [ActiveLowerElement::Typed {
                character: 'y',
                script: FontScript::ChineseSimplified,
                ..
            }, ActiveLowerElement::Cursor] => {
                assert!(active_lower_element_uses_unconfirmed_font(&elements[0]));
            }
            _ => panic!("typed Chinese pinyin feedback should remain visible before the cursor"),
        }
    }

    #[test]
    fn typing_upper_preserves_anno_annotation() {
        let app = typing_app("#title Test\n{[\u{60b2}/\u{304b}\u{306a}]/sad}");

        let render_list = build_ui(&app, app.fonts(), 800, 500);
        let (upper_segments, _) = typing_rows(&render_list);

        assert!(upper_segments
            .iter()
            .any(|segment| segment.anno_text.as_deref() == Some("sad")));
    }

    #[test]
    fn typing_upper_groups_multi_run_anno_annotation() {
        let app = typing_app(
            "#title Test\n{[\u{5fae}/\u{3073}\u{3076}\u{3093}][\u{4fc2}/\u{3051}\u{3044}\u{3059}\u{3046}]/coefficientcoefficient}",
        );

        let render_list = build_ui(&app, app.fonts(), 800, 500);
        let (upper_segments, _) = typing_rows(&render_list);
        let (upper_alignment, _) = typing_alignment(&render_list);

        assert_eq!(upper_segments.len(), 2);
        assert_eq!(
            upper_segments[1].anno_text.as_deref(),
            Some("coefficientcoefficient")
        );
        assert_eq!(upper_segments[1].anno_group_run_count, 2);

        let annotation_width = gui_renderer::measure_text(
            app.fonts().get_ruby_for_script(FontScript::Japanese),
            "coefficientcoefficient",
            app.fonts().scaled_size_for_ruby_script(
                FontScript::Japanese,
                calculate_pixel_font_size(FontSize::WindowHeight(BASE_FONT_SIZE_RATIO), 800, 500)
                    * 0.3,
            ),
        )
        .0;
        assert!(
            upper_alignment.full_line_width >= annotation_width,
            "upper line width {} should reserve grouped annotation width {}",
            upper_alignment.full_line_width,
            annotation_width
        );
    }

    #[test]
    fn wrong_key_feedback_keeps_unconfirmed_prefix_visible() {
        let mut app = typing_app("#title Test\n[\u{8272}/\u{3057}]\u{3042}");
        app.on_event(AppEvent::Char {
            c: 's',
            timestamp: 1.0,
        });
        app.on_event(AppEvent::Char {
            c: 'x',
            timestamp: 2.0,
        });

        let render_list = build_ui(&app, app.fonts(), 800, 500);
        let (_, lower_segments) = typing_rows(&render_list);

        let LowerTypingSegment::Active { elements, .. } = &lower_segments[0] else {
            panic!("first lower segment should be active");
        };
        match elements.as_slice() {
            [ActiveLowerElement::UnconfirmedInput { text, script }, ActiveLowerElement::LastIncorrectInput {
                character,
                script: wrong_script,
            }] => {
                assert_eq!(text, "s");
                assert_eq!(*script, FontScript::Japanese);
                assert_eq!(*character, 'x');
                assert_eq!(*wrong_script, FontScript::Japanese);
            }
            _ => panic!("wrong feedback should keep the accepted romaji prefix visible"),
        }
    }

    #[test]
    fn completed_chinese_lower_ruby_uses_tone_marks() {
        let mut app = typing_app("#title Test\n[\u{6709}/you3][\u{65e0}/wu2]");
        for (index, c) in "you3".chars().enumerate() {
            app.on_event(AppEvent::Char {
                c,
                timestamp: index as f64,
            });
        }
        app.update(800, 500, 16.0);

        let render_list = build_ui(&app, app.fonts(), 800, 500);
        let (_, lower_segments) = typing_rows(&render_list);
        let completed = lower_segments
            .iter()
            .find_map(|segment| match segment {
                LowerTypingSegment::Completed {
                    base_text,
                    ruby_text,
                    script,
                    ..
                } if base_text == "\u{6709}" => Some((ruby_text.as_deref(), *script)),
                _ => None,
            })
            .expect("completed Chinese lower segment should exist");

        assert_eq!(
            completed,
            (Some("y\u{01d2}u"), FontScript::ChineseSimplified)
        );
    }

    #[test]
    fn uncached_completed_chinese_lower_ruby_uses_tone_marks() {
        let mut app = typing_app("#title Test\n[\u{6709}/you3][\u{65e0}/wu2]");
        for (index, c) in "you3".chars().enumerate() {
            app.on_event(AppEvent::Char {
                c,
                timestamp: index as f64,
            });
        }

        let render_list = build_ui(&app, app.fonts(), 800, 500);
        let (_, lower_segments) = typing_rows(&render_list);
        let completed = lower_segments
            .iter()
            .find_map(|segment| match segment {
                LowerTypingSegment::Completed {
                    base_text,
                    ruby_text,
                    script,
                    ..
                } if base_text == "\u{6709}" => Some((ruby_text.as_deref(), *script)),
                _ => None,
            })
            .expect("uncached completed Chinese lower segment should exist");

        assert_eq!(
            completed,
            (Some("y\u{01d2}u"), FontScript::ChineseSimplified)
        );
    }

    #[test]
    fn problem_selection_title_uses_typing_font_renderable() {
        let mut app = App::new(test_fonts());
        app.add_custom_problem(
            "plain".to_string(),
            "#title [\u{6625}\u{6653}/chun1xiao3]\n[\u{6625}\u{7720}/chun1mian2]".to_string(),
            0,
        );

        let render_list = build_ui(&app, app.fonts(), 800, 500);
        let menu_title = render_list
            .iter()
            .find_map(|item| match item {
                Renderable::TypingUpper { segments, .. }
                    if segments
                        .iter()
                        .any(|segment| segment.base_text == "\u{6625}\u{6653}") =>
                {
                    Some(segments.as_slice())
                }
                _ => None,
            })
            .expect("problem menu title should render through TypingUpper");

        assert_eq!(menu_title[0].base_text, "\u{6625}\u{6653}");
        assert_eq!(
            menu_title[0].ruby_text.as_deref(),
            Some("ch\u{016b}nxi\u{01ce}o")
        );
        assert_eq!(menu_title[0].script, FontScript::ChineseSimplified);
        assert!(!render_texts(&render_list)
            .iter()
            .any(|text| text.contains("\u{6625}\u{6653}")));
    }

    #[test]
    fn typing_context_lines_preserve_problem_ruby() {
        let mut app = typing_app("#title Test\n[前/まえ]\n[今/いま]\n[次/つぎ]");
        app.fonts.set_scale_for_target(
            FontTarget::Script(FontScript::Japanese),
            FontScale::Percent50,
        );
        app.fonts
            .set_scale_for_target(FontTarget::Ruby(FontScript::Japanese), FontScale::Percent50);
        app.on_event(AppEvent::Char {
            c: 'm',
            timestamp: 1.0,
        });
        app.on_event(AppEvent::Char {
            c: 'a',
            timestamp: 2.0,
        });
        app.on_event(AppEvent::Char {
            c: 'e',
            timestamp: 3.0,
        });

        let render_list = build_ui(&app, app.fonts(), 800, 500);
        let context = context_segments(&render_list);

        assert!(!context.is_empty());
        assert_eq!(context[0][0].base_text, "前");
        assert_eq!(context[0][0].ruby_text.as_deref(), Some("まえ"));
        if context.len() > 1 {
            assert_eq!(context[1][0].base_text, "次");
            assert_eq!(context[1][0].ruby_text.as_deref(), Some("つぎ"));
        }
        assert!(context
            .iter()
            .flat_map(|segments| segments.iter())
            .all(|segment| segment.state == UpperSegmentState::Muted));
    }

    #[test]
    fn gui_play_title_and_previous_context_ruby_do_not_overlap() {
        let mut app = typing_app("#title [春暁/しゅんぎょう]\n[前/まえ]\n[今/いま]\n[次/つぎ]");
        for (index, c) in "mae".chars().enumerate() {
            app.on_event(AppEvent::Char {
                c,
                timestamp: index as f64,
            });
        }

        let width = 800;
        let height = 500;
        for scale in [
            DisplayScale::Percent75,
            DisplayScale::Percent100,
            DisplayScale::Percent125,
            DisplayScale::Percent150,
            DisplayScale::Percent200,
        ] {
            app.display_settings.scale = scale;
            let render_list = build_ui(&app, app.fonts(), width, height);
            let boxes = play_typing_text_boxes(
                &render_list,
                app.fonts(),
                width,
                height,
                scale.multiplier(),
            );
            if scale == DisplayScale::Percent100 {
                let title = find_box(&boxes, "title");
                let previous_context = find_box(&boxes, "previous_context");
                assert!(
                    title.bottom + 4.0 <= previous_context.top,
                    "at scale {} title box {title:?} should leave room before previous context ruby box {previous_context:?}; all boxes: {boxes:#?}",
                    scale.label()
                );
            } else if let (Some(title), Some(previous_context)) = (
                find_optional_box(&boxes, "title"),
                find_optional_box(&boxes, "previous_context"),
            ) {
                assert!(
                    title.bottom + 4.0 <= previous_context.top,
                    "at scale {} title box {title:?} should leave room before previous context ruby box {previous_context:?}; all boxes: {boxes:#?}",
                    scale.label()
                );
            }
        }
    }

    #[test]
    fn gui_play_major_typing_rows_do_not_overlap() {
        let mut app = typing_app("#title [春暁/しゅんぎょう]\n[前/まえ]\n[今/いま]\n[次/つぎ]");
        for (index, c) in "mae".chars().enumerate() {
            app.on_event(AppEvent::Char {
                c,
                timestamp: index as f64,
            });
        }

        let width = 800;
        let height = 500;
        for scale in [
            DisplayScale::Percent75,
            DisplayScale::Percent100,
            DisplayScale::Percent125,
            DisplayScale::Percent150,
            DisplayScale::Percent200,
        ] {
            app.display_settings.scale = scale;
            let render_list = build_ui(&app, app.fonts(), width, height);
            let boxes = play_typing_text_boxes(
                &render_list,
                app.fonts(),
                width,
                height,
                scale.multiplier(),
            );

            assert_no_vertical_text_collisions(&boxes, 4.0);
        }
    }

    #[test]
    fn next_context_is_left_aligned_next_to_status_float() {
        let app =
            typing_app("#title Test\n[\u{4eca}/\u{3044}\u{307e}]\n[\u{6b21}/\u{3064}\u{304e}]");
        let width = 800;
        let height = 500;
        let display_scale = app.display_settings.scale.multiplier();
        let render_list = build_ui(&app, app.fonts(), width, height);
        let model = app.typing_model().expect("typing model should exist");
        let metrics = typing::calculate_total_metrics(model);
        let status_rows = typing_status_rows(model, model.content.lines.len(), metrics);
        let expected_left =
            status_float_right_edge(app.fonts(), &status_rows, width, height, display_scale);

        let next_context = render_list
            .iter()
            .find_map(|item| match item {
                Renderable::TypingUpper {
                    segments,
                    anchor,
                    shift,
                    align,
                    ..
                } if segments
                    .iter()
                    .any(|segment| segment.base_text == "\u{6b21}") =>
                {
                    Some((segments.as_slice(), *anchor, *shift, *align))
                }
                _ => None,
            })
            .expect("next context line should be rendered next to the status float");

        assert!(next_context
            .0
            .iter()
            .all(|segment| segment.state == UpperSegmentState::Muted));
        assert!(matches!(next_context.1, Anchor::TopLeft));
        assert!(matches!(
            next_context.3,
            Align {
                horizontal: HorizontalAlign::Left,
                vertical: VerticalAlign::Top,
            }
        ));
        assert!(
            ((next_context.2.x * width as f32) - expected_left).abs() <= 1.0,
            "next context x={} should start at measured status right edge {expected_left}",
            next_context.2.x * width as f32
        );
    }

    #[test]
    fn typing_status_uses_two_column_table_layout() {
        let app =
            typing_app("#title Test\n[\u{4eca}/\u{3044}\u{307e}]\n[\u{6b21}/\u{3064}\u{304e}]");
        let width = 800;
        let height = 500;
        let render_list = build_ui(&app, app.fonts(), width, height);
        let model = app.typing_model().expect("typing model should exist");
        let metrics = typing::calculate_total_metrics(model);
        let rows = typing_status_rows(model, model.content.lines.len(), metrics);
        let layout = status_table_layout(
            app.fonts(),
            &rows,
            width,
            height,
            app.display_settings.scale.multiplier(),
        );

        for row in &rows {
            let label = render_list
                .iter()
                .find_map(|item| match item {
                    Renderable::Text {
                        text, shift, align, ..
                    } if text == row.label => Some((*shift, *align)),
                    _ => None,
                })
                .expect("status label should be rendered");
            let value = render_list
                .iter()
                .find_map(|item| match item {
                    Renderable::Text {
                        text, shift, align, ..
                    } if text == &row.value => Some((*shift, *align)),
                    _ => None,
                })
                .expect("status value should be rendered");

            assert!(
                (label.0.x * width as f32 - layout.label_right).abs() <= 1.0,
                "label '{}' should align to the label column right edge",
                row.label
            );
            assert!(matches!(
                label.1,
                Align {
                    horizontal: HorizontalAlign::Right,
                    vertical: VerticalAlign::Bottom,
                }
            ));
            assert!(
                (value.0.x * width as f32 - layout.value_left).abs() <= 1.0,
                "value '{}' should align to the value column left edge",
                row.value
            );
            assert!(matches!(
                value.1,
                Align {
                    horizontal: HorizontalAlign::Left,
                    vertical: VerticalAlign::Bottom,
                }
            ));
            assert!(
                (label.0.y - value.0.y).abs() <= f32::EPSILON,
                "label and value should share a row baseline"
            );
        }
    }

    #[test]
    fn status_region_reservation_grows_with_ui_font_scale() {
        let mut fonts = test_fonts();
        let normal_row_step = measured_ui_row_step(
            &fonts,
            FontSize::WindowHeight(TYPING_STATUS_ITEM_HEIGHT_RATIO),
            320,
            240,
            DisplayScale::Percent100.multiplier(),
            TYPING_STATUS_ITEM_HEIGHT_RATIO,
        );
        let normal_top = typing_status_region_top(240, 5, normal_row_step);

        fonts.set_scale_for_target(FontTarget::Ui, FontScale::Percent200);
        let scaled_row_step = measured_ui_row_step(
            &fonts,
            FontSize::WindowHeight(TYPING_STATUS_ITEM_HEIGHT_RATIO),
            320,
            240,
            DisplayScale::Percent100.multiplier(),
            TYPING_STATUS_ITEM_HEIGHT_RATIO,
        );
        let scaled_top = typing_status_region_top(240, 5, scaled_row_step);

        assert!(scaled_row_step > normal_row_step);
        assert!(
            scaled_top < normal_top,
            "larger UI Font Scale should reserve more vertical status area: normal={normal_top}, scaled={scaled_top}"
        );
    }

    #[test]
    fn lower_active_metrics_include_typed_element_script_scale() {
        let mut fonts = test_fonts();
        let segments = [LowerTypingSegment::Active {
            elements: vec![ActiveLowerElement::Typed {
                character: 'A',
                is_correct: true,
                script: FontScript::English,
            }],
            script: FontScript::Japanese,
        }];

        let normal = lower_typing_vertical_metrics(&segments, &fonts, 48.0).base_height;
        fonts.set_scale_for_target(
            FontTarget::Script(FontScript::English),
            FontScale::Percent200,
        );
        let scaled = lower_typing_vertical_metrics(&segments, &fonts, 48.0).base_height;

        assert!(
            scaled > normal * 1.5,
            "active lower metrics should grow with typed element script scale: normal={normal}, scaled={scaled}"
        );
    }

    #[test]
    fn settings_row_step_grows_with_ui_font_scale() {
        let mut fonts = test_fonts();
        let normal = measured_ui_row_step(
            &fonts,
            FontSize::WindowHeight(0.042),
            320,
            240,
            DisplayScale::Percent100.multiplier(),
            0.058,
        );
        fonts.set_scale_for_target(FontTarget::Ui, FontScale::Percent200);
        let scaled = measured_ui_row_step(
            &fonts,
            FontSize::WindowHeight(0.042),
            320,
            240,
            DisplayScale::Percent100.multiplier(),
            0.058,
        );

        assert!(
            scaled > normal * 1.5,
            "settings row step should grow with UI Font Scale: normal={normal}, scaled={scaled}"
        );
    }

    #[test]
    fn settings_font_picker_lists_available_font_files() {
        let mut app = App::new(test_fonts());
        app.state = AppState::Settings;
        app.settings_picking_font = true;
        app.set_available_fonts(vec![
            FontEntry {
                id: FontAssetId(0),
                name: "YujiSyuku-Regular".to_string(),
                source: FontSource::Bundled,
            },
            FontEntry {
                id: FontAssetId(1),
                name: "MaShanZheng-Regular".to_string(),
                source: FontSource::Bundled,
            },
            FontEntry {
                id: FontAssetId(2),
                name: "Kalam-Regular".to_string(),
                source: FontSource::Bundled,
            },
        ]);

        let render_list = build_ui(&app, app.fonts(), 800, 500);
        let texts = render_texts(&render_list);

        assert!(texts.iter().any(|text| text.contains("YujiSyuku-Regular")));
        assert!(texts
            .iter()
            .any(|text| text.contains("MaShanZheng-Regular")));
        assert!(texts.iter().any(|text| text.contains("Kalam-Regular")));
    }

    #[test]
    fn settings_table_lists_ui_font_and_keeps_selected_row_visible() {
        let mut app = App::new(test_fonts());
        app.state = AppState::Settings;

        let render_list = build_ui(&app, app.fonts(), 320, 240);
        let texts = render_texts(&render_list);
        assert!(texts.contains(&"UI Font"));
        assert!(texts.contains(&"Japanese Ruby Font"));
        assert!(texts_have_prefix(&texts, "Japanese Unconfirmed"));
        assert!(texts.contains(&"UI Font Scale"));
        assert!(!texts.contains(&"Chinese Simplified Font"));

        app.selected_settings_item =
            SettingsItem::FontFamily(FontTarget::Unconfirmed(FontScript::ChineseSimplified));
        let render_list = build_ui(&app, app.fonts(), 320, 240);
        let texts = render_texts(&render_list);
        assert!(texts.contains(&"Simplified Chinese Font"));
        assert!(texts_have_prefix(&texts, "Simplified Chinese Ruby"));
        assert!(texts_have_prefix(&texts, "Simplified Chinese Unco"));

        app.selected_settings_item =
            SettingsItem::FontFamily(FontTarget::Unconfirmed(FontScript::TraditionalChinese));
        let render_list = build_ui(&app, app.fonts(), 320, 240);
        let texts = render_texts(&render_list);
        assert!(texts.contains(&"Traditional Chinese Font"));
        assert!(texts_have_prefix(&texts, "Traditional Chinese Ru"));
        assert!(texts_have_prefix(&texts, "Traditional Chinese Un"));

        app.selected_settings_item = SettingsItem::ImeInput;
        let render_list = build_ui(&app, app.fonts(), 320, 240);
        let texts = render_texts(&render_list);
        assert!(texts.contains(&"IME Input"));
        assert!(texts.contains(&"Disabled"));

        app.selected_settings_item = SettingsItem::DisplayScale;
        app.display_settings.scale = DisplayScale::Percent200;

        let render_list = build_ui(&app, app.fonts(), 320, 240);
        let texts = render_texts(&render_list);

        assert!(texts.contains(&"Display Scale"));
        assert!(texts
            .iter()
            .any(|text| text.contains(app.display_settings.scale.label())));
    }

    #[test]
    fn display_scale_is_applied_to_typing_line_measurement() {
        let mut app = typing_app("#title Test\n[色/いろ][字/zi4][字/ㄗˋ]");

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
            .position(|segment| {
                segment
                    .display_runs
                    .first()
                    .is_some_and(|run| &run.base_text == first_upper_text)
            })
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
            .position(|segment| {
                segment
                    .display_runs
                    .first()
                    .is_some_and(|run| &run.base_text == first_completed_text)
            })
            .expect("visible lower segment should come from the scroll cache");
        assert_eq!(
            lower_alignment.visible_start_width,
            cache.current.segment_prefix_width[first_visible_segment] as u32
        );
    }

    #[test]
    fn cached_lower_multi_run_segments_use_ruby_widths() {
        let mut app = typing_app(
            "#title Test\n{[\u{5fae}/\u{3073}\u{3076}\u{3093}][\u{4fc2}/\u{3051}\u{3044}\u{3059}\u{3046}]/coefficient}[\u{6b21}/\u{3064}\u{304e}]",
        );
        for (index, c) in "\u{3073}\u{3076}\u{3093}\u{3051}\u{3044}\u{3059}\u{3046}"
            .chars()
            .enumerate()
        {
            app.on_event(AppEvent::Char {
                c,
                timestamp: index as f64,
            });
        }
        app.update(800, 500, 100.0);

        let render_list = build_ui(&app, app.fonts(), 800, 500);
        let (_, lower_segments) = typing_rows(&render_list);
        let completed = lower_segments
            .iter()
            .find_map(|segment| match segment {
                LowerTypingSegment::Completed {
                    base_text,
                    ruby_text,
                    width,
                    ..
                } if base_text == "\u{5fae}" => Some((ruby_text.as_deref(), *width)),
                _ => None,
            })
            .expect("cached multi-run completed segment should be present");

        let expected_ruby_width = gui_renderer::measure_text(
            app.fonts().get_ruby_for_script(FontScript::Japanese),
            "\u{3073}\u{3076}\u{3093}",
            app.fonts().scaled_size_for_ruby_script(
                FontScript::Japanese,
                calculate_pixel_font_size(FontSize::WindowHeight(BASE_FONT_SIZE_RATIO), 800, 500)
                    * 0.4,
            ),
        )
        .0;

        assert_eq!(completed.0, Some("\u{3073}\u{3076}\u{3093}"));
        assert!(
            completed.1 >= expected_ruby_width,
            "cached lower width {} should include ruby width {}",
            completed.1,
            expected_ruby_width
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
                    anchor: Anchor::Center,
                    shift,
                    align:
                        Align {
                            horizontal: HorizontalAlign::Left,
                            ..
                        },
                    line_alignment,
                    ..
                } => {
                    assert_line_left_matches_scroll_offset(
                        Anchor::Center,
                        *shift,
                        Align {
                            horizontal: HorizontalAlign::Left,
                            vertical: VerticalAlign::Center,
                        },
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
