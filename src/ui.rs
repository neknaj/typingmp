// src/ui.rs

// uefi feature縺梧怏蜉ｹ縺ｪ蝣ｴ蜷医∵ｨ呎ｺ悶・alloc繧ｯ繝ｬ繝ｼ繝医ｒ繧､繝ｳ繝昴・繝・#[cfg(feature = "uefi")]
extern crate alloc;

// uefi 縺ｧ f64::floor() 繧剃ｽｿ縺・◆繧√↓蠢・ｦ・#[cfg(feature = "uefi")]
use core_maths::CoreFloat;

// uefi 縺ｨ std 縺ｧ菴ｿ逕ｨ縺吶ｋ Vec 縺ｨ vec! 繧貞・繧頑崛縺医ｋ
#[cfg(feature = "uefi")]
use alloc::{vec, vec::Vec};
#[cfg(not(feature = "uefi"))]
use std::vec::Vec;

// uefi 縺ｨ std 縺ｧ菴ｿ逕ｨ縺吶ｋ String 縺ｨ format! 繧貞・繧頑崛縺医ｋ
#[cfg(feature = "uefi")]
use alloc::{format, string::{String, ToString}};
#[cfg(not(feature = "uefi"))]
use std::string::{String, ToString};

use crate::app::{App, AppState, ScrollCache, Script};
use crate::model::{Segment, TypingCorrectnessChar, TypingCorrectnessSegment, TypingCorrectnessWord};
use crate::renderer::{calculate_pixel_font_size, gui_renderer};
use crate::typing; // For calculate_total_metrics
use ab_glyph::FontVec;

/// 逕ｻ髱｢荳翫・謠冗判蝓ｺ貅也せ繧貞ｮ夂ｾｩ縺吶ｋenum
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

/// Anchor縺九ｉ縺ｮ繧ｪ繝輔そ繝・ヨ・育ｧｻ蜍暮㍼・峨ｒ螳夂ｾｩ縺吶ｋ讒矩菴・#[derive(Clone, Copy)]
pub struct Shift {
    pub x: f32,
    pub y: f32,
}

/// 豌ｴ蟷ｳ譁ｹ蜷代・謠・∴
#[derive(Clone, Copy)]
pub enum HorizontalAlign {
    Left,
    Center,
    Right,
}

/// 蝙ら峩譁ｹ蜷代・謠・∴
#[derive(Clone, Copy)]
pub enum VerticalAlign {
    Top,
    Center,
    Bottom,
}

/// 繝・く繧ｹ繝医・謠・∴譁ｹ繧貞ｮ夂ｾｩ縺吶ｋ讒矩菴・#[derive(Clone, Copy)]
pub struct Align {
    pub horizontal: HorizontalAlign,
    pub vertical: VerticalAlign,
}

/// 繝輔か繝ｳ繝医し繧､繧ｺ縺ｮ蝓ｺ貅悶ｒ螳夂ｾｩ縺吶ｋenum
#[derive(Clone, Copy)]
pub enum FontSize {
    /// 繧ｦ繧｣繝ｳ繝峨え縺ｮ鬮倥＆縺ｫ蟇ｾ縺吶ｋ豈皮紫
    WindowHeight(f32),
    /// 繧ｦ繧｣繝ｳ繝峨え縺ｮ髱｢遨阪・蟷ｳ譁ｹ譬ｹ縺ｫ蟇ｾ縺吶ｋ豈皮紫
    WindowAreaSqrt(f32),
}

/// 繧ｰ繝ｩ繝・・繧ｷ繝ｧ繝ｳ縺ｮ螳夂ｾｩ
#[derive(Clone, Copy)]
pub struct Gradient {
    pub start_color: u32,
    pub end_color: u32,
}

/// 荳頑ｮｵ・育岼讓吶ユ繧ｭ繧ｹ繝茨ｼ峨・繧ｻ繧ｰ繝｡繝ｳ繝医・迥ｶ諷・pub enum UpperSegmentState {
    /// 螳御ｺ・ｸ医∩・域ｭ｣縺励￥繧ｿ繧､繝励＆繧後◆・・    Correct,
    /// 螳御ｺ・ｸ医∩・磯俣驕輔＞繧貞性繧薙〒縺・◆・・    Incorrect,
    /// 譛ｪ蜈･蜉・    Pending,
    /// 迴ｾ蝨ｨ蜈･蜉帑ｸｭ縺ｮ繧｢繧ｯ繝・ぅ繝悶↑繧ｻ繧ｰ繝｡繝ｳ繝・    Active,
}

/// 荳頑ｮｵ・育岼讓吶ユ繧ｭ繧ｹ繝茨ｼ峨ｒ讒区・縺吶ｋ縲√Ν繝謎ｻ倥″縺ｮ1繧ｻ繧ｰ繝｡繝ｳ繝・pub struct UpperTypingSegment {
    pub base_text: String,
    pub ruby_text: Option<String>,
    /// anno險俶ｳ輔・豕ｨ驥医ユ繧ｭ繧ｹ繝茨ｼ医・繝ｼ繧ｹ繝・く繧ｹ繝医・荳九↓陦ｨ遉ｺ・・    pub anno_text: Option<String>,
    pub state: UpperSegmentState,
}

/// 荳区ｮｵ・亥・蜉帙ユ繧ｭ繧ｹ繝茨ｼ峨・繧｢繧ｯ繝・ぅ繝厄ｼ育樟蝨ｨ蜈･蜉帑ｸｭ・峨そ繧ｰ繝｡繝ｳ繝医ｒ讒区・縺吶ｋ隕∫ｴ
pub enum ActiveLowerElement {
    /// 繧ｿ繧､繝玲ｸ医∩縺ｮ譁・ｭ暦ｼ域ｭ｣隱､諠・ｱ莉倥″・・    Typed { character: char, is_correct: bool },
    /// 繧ｫ繝ｼ繧ｽ繝ｫ
    Cursor,
    /// 譛ｪ遒ｺ螳壹・繝ｭ繝ｼ繝槫ｭ怜・蜉・(萓・ "k", "ky")
    UnconfirmedInput(String),
    /// 逶ｴ蜑阪・隱､蜈･蜉帙く繝ｼ
    LastIncorrectInput(char),
}

/// 荳区ｮｵ・亥・蜉帙ユ繧ｭ繧ｹ繝茨ｼ峨ｒ讒区・縺吶ｋ繧ｻ繧ｰ繝｡繝ｳ繝・pub enum LowerTypingSegment {
    /// 螳御ｺ・ｸ医∩縺ｮ繧ｻ繧ｰ繝｡繝ｳ繝・    Completed {
        base_text: String,
        ruby_text: Option<String>,
        is_correct: bool,
        width: u32,
    },
    /// 迴ｾ蝨ｨ蜈･蜉帑ｸｭ縺ｮ繧｢繧ｯ繝・ぅ繝悶↑繧ｻ繧ｰ繝｡繝ｳ繝・    Active { elements: Vec<ActiveLowerElement> },
}

/// 逕ｻ髱｢縺ｫ謠冗判縺吶∋縺崎ｦ∫ｴ縺ｮ遞ｮ鬘槭→繝ｬ繧､繧｢繧ｦ繝域ュ蝣ｱ繧貞ｮ夂ｾｩ縺吶ｋenum
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
    /// 荳頑ｮｵ縺ｮ逶ｮ讓吶ユ繧ｭ繧ｹ繝郁｡悟・菴薙ｒ陦ｨ縺吝梛
    TypingUpper {
        segments: Vec<UpperTypingSegment>,
        anchor: Anchor,
        shift: Shift,
        align: Align,
        font_size: FontSize, // 繝吶・繧ｹ繝・く繧ｹ繝医・繝輔か繝ｳ繝医し繧､繧ｺ
    },
    /// 荳区ｮｵ縺ｮ蜈･蜉帙ユ繧ｭ繧ｹ繝郁｡悟・菴薙ｒ陦ｨ縺吝梛
    TypingLower {
        segments: Vec<LowerTypingSegment>,
        anchor: Anchor,
        shift: Shift,
        align: Align,
        font_size: FontSize, // 蜈･蜉帙ユ繧ｭ繧ｹ繝医・繝輔か繝ｳ繝医し繧､繧ｺ
        target_line_total_width: u32,
    },
    ProgressBar {
        anchor: Anchor,
        shift: Shift,
        width_ratio: f32, // 逕ｻ髱｢蟷・↓蟇ｾ縺吶ｋ豈皮紫
        height_ratio: f32, // 逕ｻ髱｢鬮倥＆縺ｫ蟇ｾ縺吶ｋ豈皮紫
        progress: f32, // 0.0 to 1.0
        bg_color: u32,
        fg_color: u32,
    },
}

/// How to Use 逕ｻ髱｢縺ｫ陦ｨ遉ｺ縺吶ｋ蜷・｡・ (繝・く繧ｹ繝・ 濶ｲ) 縺ｮ螳夂ｾｩ縲・/// app.rs 縺九ｉ陦梧焚蜿ら・縺ｮ縺溘ａ縺ｫ pub(crate) 縺ｧ蜈ｬ髢九☆繧九・pub(crate) const HOW_TO_USE_CONTENT: &[(&str, u32)] = &[
    // 笏笏笏 蝓ｺ譛ｬ謫堺ｽ・笏笏笏
    ("[ 蝓ｺ譛ｬ謫堺ｽ・]",                              0xFF_FFDD88),
    ("  竊・/ 竊・  : 鬆・岼繧帝∈謚・,                   0xFF_CCCCCC),
    ("  Enter    : 驕ｸ謚・/ 豎ｺ螳・,                  0xFF_CCCCCC),
    ("  Esc      : 蜑阪・逕ｻ髱｢縺ｫ謌ｻ繧・,               0xFF_CCCCCC),
    ("",                                          0xFF_000000),
    // 笏笏笏 繧ｿ繧､繝斐Φ繧ｰ 笏笏笏
    ("[ 繧ｿ繧､繝斐Φ繧ｰ ]",                            0xFF_FFDD88),
    ("  繝ｭ繝ｼ繝槫ｭ励〒縺ｲ繧峨′縺ｪ / 繧ｫ繧ｿ繧ｫ繝翫ｒ蜈･蜉・,     0xFF_CCCCCC),
    ("  萓・ 縺銀・ka  縺坂・ki  縺娯・ga  縺ｯ竊檀a",        0xFF_888888),
    ("  髱定牡 : 豁｣縺励￥繧ｿ繧､繝励＆繧後◆譁・ｭ・,           0xFF_9097FF),
    ("  襍､濶ｲ : 髢馴＆縺・ｒ蜷ｫ繧薙□譁・ｭ・,              0xFF_FF9898),
    ("  逋ｽ濶ｲ : 迴ｾ蝨ｨ縺ｮ蜈･蜉帑ｽ咲ｽｮ",                  0xFF_FFFFFF),
    ("  Backspace : 譛ｪ遒ｺ螳壹Ο繝ｼ繝槫ｭ・/ 隱､繧雁・蜉帙ｒ豸医☆", 0xFF_CCCCCC),
    ("",                                          0xFF_000000),
    // 笏笏笏 蝠城｡碁∈謚・笏笏笏
    ("[ 蝠城｡碁∈謚・]",                              0xFF_FFDD88),
    ("  Enter : 繧ｿ繧､繝斐Φ繧ｰ髢句ｧ・,                  0xFF_CCCCCC),
    ("  V     : 繧ｽ繝ｼ繧ｹ繝輔ぃ繧､繝ｫ繧堤｢ｺ隱・,            0xFF_CCCCCC),
    ("  X     : 繧ｫ繧ｹ繧ｿ繝蝠城｡後ｒ蜑企勁",              0xFF_CCCCCC),
    ("  U / D : 繧ｫ繧ｹ繧ｿ繝蝠城｡後・鬆・ｺ上ｒ螟画峩",        0xFF_CCCCCC),
    ("",                                          0xFF_000000),
    // 笏笏笏 繝輔Μ繝・け蜈･蜉・笏笏笏
    ("[ 繝輔Μ繝・け蜈･蜉・(繧ｿ繝・メ繝・ヰ繧､繧ｹ) ]",         0xFF_FFDD88),
    ("  繧ｿ繧､繝斐Φ繧ｰ荳ｭ縺ｫ逕ｻ髱｢荳矩Κ縺ｫ繧ｭ繝ｼ繝懊・繝峨′陦ｨ遉ｺ", 0xFF_CCCCCC),
    ("  荳ｭ螟ｮ繧ｿ繝・・ : 縺よｮｵ  (萓・ 縺・竊・縺・",        0xFF_CCCCCC),
    ("  荳翫ヵ繝ｪ繝・け : 縺・ｮｵ  (萓・ 縺・竊・縺・",        0xFF_CCCCCC),
    ("  蟾ｦ繝輔Μ繝・け : 縺・ｮｵ  (萓・ 縺・竊・縺・",        0xFF_CCCCCC),
    ("  蜿ｳ繝輔Μ繝・け : 縺域ｮｵ  (萓・ 縺・竊・縺・",        0xFF_CCCCCC),
    ("  荳九ヵ繝ｪ繝・け : 縺頑ｮｵ  (萓・ 縺・竊・縺・",        0xFF_CCCCCC),
    ("  螟ｧ竍泌ｰ上く繝ｼ : 逶ｴ蜑阪・隱､繧雁・蜉帙ｒ螟画鋤",       0xFF_CCCCCC),
    ("           縺銀・縺娯・縺・ 縺ｯ竊偵・竊偵・竊偵・  縺ｪ縺ｩ", 0xFF_888888),
    ("           (逶ｴ蜑阪′隱､繧翫・縺ｨ縺阪・縺ｿ菴懷虚)",     0xFF_888888),
    ("  繧・/ 繧・ : 縲後＞縲・縲後∴縲阪〒莉｣譖ｿ蜈･蜉帛庄",   0xFF_AAAAAA),
    ("",                                          0xFF_000000),
    // 笏笏笏 繧ｫ繧ｹ繧ｿ繝蝠城｡・笏笏笏
    ("[ 繧ｫ繧ｹ繧ｿ繝蝠城｡・(.ntq 蠖｢蠑・ ]",              0xFF_FFDD88),
    ("  #title 繧ｿ繧､繝医Ν蜷・,                       0xFF_99DDAA),
    ("  [貍｢蟄・繧医∩縺後↑]  : 繝ｫ繝謎ｻ倥″繝・く繧ｹ繝・,    0xFF_99DDAA),
    ("  {蜀・ｮｹ/豕ｨ驥・      : 豕ｨ驥井ｻ倥″繝・く繧ｹ繝・,    0xFF_99DDAA),
    ("  遨ｺ逋ｽ / 繧ｹ繝ｩ繝・す繝･ : 蜊倩ｪ槭・蛹ｺ蛻・ｊ",        0xFF_99DDAA),
    ("  Web迚・ [Open File...] 縺九ｉ繧｢繝・・繝ｭ繝ｼ繝・,  0xFF_CCCCCC),
];

#[cfg(target_arch = "wasm32")]
const MENU_ITEMS: [&str; 3] = ["Start Typing", "How to Use", "Settings"];

#[cfg(not(target_arch = "wasm32"))]
const MENU_ITEMS: [&str; 4] = ["Start Typing", "How to Use", "Settings", "Quit"];

// --- 繧ｿ繧､繝斐Φ繧ｰ逕ｻ髱｢縺ｮ繝ｬ繧､繧｢繧ｦ繝亥ｮ壽焚 ---
pub const BASE_FONT_SIZE_RATIO: f32 = 0.2;
const UPPER_ROW_Y_OFFSET_FACTOR: f32 = 1.3;
const LOWER_ROW_Y_OFFSET_FACTOR: f32 = 0.2;

// --- 濶ｲ螳夂ｾｩ ---
pub const CORRECT_COLOR: u32 = 0xFF_9097FF;
pub const INCORRECT_COLOR: u32 = 0xFF_FF9898;
pub const PENDING_COLOR: u32 = 0xFF_999999;
pub const ACTIVE_COLOR: u32 = 0xFF_FFFFFF;
pub const WRONG_KEY_COLOR: u32 = 0xFF_F55252;
pub const CURSOR_COLOR: u32 = 0xFF_FFFFFF;
pub const UNCONFIRMED_COLOR: u32 = 0xFF_CCCCCC;

/// App縺ｮ迥ｶ諷九ｒ蜿励￠蜿悶ｊ縲∵緒逕ｻ繝ｪ繧ｹ繝茨ｼ・I繝ｬ繧､繧｢繧ｦ繝茨ｼ峨ｒ讒狗ｯ峨☆繧・pub fn build_ui(app: &App, font: &FontVec, width: usize, height: usize) -> Vec<Renderable> {
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

    // --- 逕ｻ髱｢蜿ｳ荳翫・FPS陦ｨ遉ｺ ---
    let fps_text = format!("FPS: {:.1}", app.fps);
    render_list.push(Renderable::Text {
        text: fps_text,
        anchor: Anchor::TopRight,
        shift: Shift { x: -0.01, y: 0.01 },
        align: Align { horizontal: HorizontalAlign::Right, vertical: VerticalAlign::Top },
        font_size: FontSize::WindowHeight(0.04),
        color: 0xFF_00FF00, // 邱題牡
    });

    // --- 逕ｻ髱｢荳矩Κ縺ｮ蜈ｱ騾啅I ---
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
        (Script::Japanese, "Japanese"),
        (Script::TraditionalChinese, "Traditional Chinese"),
        (Script::SimplifiedChinese, "Simplified Chinese"),
    ];

    for (i, (font_choice, name)) in fonts.iter().enumerate() {
        let is_selected = i == app.selected_settings_item;
        let is_active = *font_choice == app.settings_script;

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
        // 繧ｽ繝ｼ繧ｹ遞ｮ蛻･繝舌ャ繧ｸ繧剃ｻ倅ｸ・ [B]=builtin, [W]=web(wasm), [F]=file(desktop), [+]=open-file
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
        render_list.push(Renderable::Text { text: "笆ｲ".to_string(), anchor: Anchor::TopCenter, shift: Shift { x: 0.0, y: list_y_start - item_height },
            align: Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Center }, font_size: FontSize::WindowHeight(0.04), color: 0xFF_AAAAAA });
    }
    if end_index < app.problem_count() {
        render_list.push(Renderable::Text { text: "笆ｼ".to_string(), anchor: Anchor::TopCenter, shift: Shift { x: 0.0, y: list_y_start + list_height },
            align: Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Center }, font_size: FontSize::WindowHeight(0.04), color: 0xFF_AAAAAA });
    }
}

/// 蝠城｡後ヵ繧｡繧､繝ｫ縺ｮ繧ｽ繝ｼ繧ｹ繧ｳ繝ｼ繝峨ｒ髢ｲ隕ｧ縺吶ｋ繧ｷ繝ｼ繝ｳ繧呈緒逕ｻ縺吶ｋ
fn build_problem_source_ui(app: &App, render_list: &mut Vec<Renderable>, gradient: Gradient) {
    render_list.push(Renderable::Background { gradient });

    let idx = app.selected_problem_item;
    let label = app.problem_source_label(idx);
    let name = app.problem_name_at(idx);

    // 繝倥ャ繝繝ｼ: "[遞ｮ蛻･] 蝠城｡悟錐"
    render_list.push(Renderable::BigText {
        text: format!("[{}] {}", label, name),
        anchor: Anchor::TopCenter,
        shift: Shift { x: 0.0, y: 0.05 },
        align: Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Top },
        font_size: FontSize::WindowHeight(0.09),
        color: 0xFF_AADDFF,
    });

    // 繧ｽ繝ｼ繧ｹ繧ｳ繝ｳ繝・Φ繝・ｼ・陦後★縺､謠冗判・・    let line_h: f32 = 0.046;
    let content_y: f32 = 0.21;
    // 荳矩Κ縺ｮ status_text / instructions_text 鬆伜沺繧帝∩縺代ｋ縺溘ａ 0.12 繧剃ｽ咏區縺ｨ縺励※遒ｺ菫・    let max_lines = ((1.0f32 - content_y - 0.12) / line_h).floor() as usize;

    if let Some(content) = app.get_problem_source(idx) {
        let total_lines = content.lines().count();

        for (i, line) in content.lines().skip(app.source_scroll).take(max_lines).enumerate() {
            // 髟ｷ縺・｡後・60譁・ｭ励〒蛻・ｊ隧ｰ繧・ｼ医ヰ繧､繝亥｢・阜縺ｧ縺ｯ縺ｪ縺乗枚蟄怜｢・阜縺ｧ・・            let ch_count = line.chars().count();
            let display = if ch_count > 60 {
                let truncated: String = line.chars().take(60).collect();
                format!("{}窶ｦ", truncated)
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

        // 繧ｹ繧ｯ繝ｭ繝ｼ繝ｫ菴咲ｽｮ繧､繝ｳ繧ｸ繧ｱ繝ｼ繧ｿ (蜿ｳ荳・
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

        // 荳贋ｸ九せ繧ｯ繝ｭ繝ｼ繝ｫ蜿ｯ閭ｽ縺ｧ縺ゅｋ縺薙→繧堤､ｺ縺咏泙蜊ｰ
        if app.source_scroll > 0 {
            render_list.push(Renderable::Text {
                text: "笆ｲ".to_string(),
                anchor: Anchor::TopCenter,
                shift: Shift { x: 0.45, y: content_y - line_h },
                align: Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Top },
                font_size: FontSize::WindowHeight(0.035),
                color: 0xFF_888888,
            });
        }
        if app.source_scroll + max_lines < total_lines {
            render_list.push(Renderable::Text {
                text: "笆ｼ".to_string(),
                anchor: Anchor::TopCenter,
                shift: Shift { x: 0.45, y: content_y + max_lines as f32 * line_h },
                align: Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Top },
                font_size: FontSize::WindowHeight(0.035),
                color: 0xFF_888888,
            });
        }
    }
}

/// How to Use 逕ｻ髱｢繧呈緒逕ｻ縺吶ｋ縲・/// 繧ｳ繝ｳ繝・Φ繝・・ HOW_TO_USE_CONTENT 螳壽焚縺ｧ螳夂ｾｩ縺励√せ繧ｯ繝ｭ繝ｼ繝ｫ縺ｫ蟇ｾ蠢懊☆繧九・fn build_how_to_use_ui(app: &App, render_list: &mut Vec<Renderable>, gradient: Gradient) {
    render_list.push(Renderable::Background { gradient });

    render_list.push(Renderable::BigText {
        text: "How to Use".to_string(),
        anchor: Anchor::TopCenter,
        shift: Shift { x: 0.0, y: 0.05 },
        align: Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Top },
        font_size: FontSize::WindowHeight(0.09),
        color: 0xFF_AADDFF,
    });

    // ProblemSource 縺ｨ蜷後§繝ｬ繧､繧｢繧ｦ繝亥ｮ壽焚繧剃ｽｿ逕ｨ
    let line_h: f32 = 0.046;
    let content_y: f32 = 0.21;
    // 荳矩Κ縺ｮ status_text / instructions_text 鬆伜沺 (0.12) 繧帝勁縺・◆譛螟ｧ陦ｨ遉ｺ陦梧焚
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

    // 繧ｹ繧ｯ繝ｭ繝ｼ繝ｫ菴咲ｽｮ繧､繝ｳ繧ｸ繧ｱ繝ｼ繧ｿ (蜿ｳ荳・
    let scroll_text = format!("{}/{}", scroll + 1, total_lines.max(1));
    render_list.push(Renderable::Text {
        text: scroll_text,
        anchor: Anchor::TopRight,
        shift: Shift { x: -0.01, y: 0.14 },
        align: Align { horizontal: HorizontalAlign::Right, vertical: VerticalAlign::Top },
        font_size: FontSize::WindowHeight(0.035),
        color: 0xFF_666688,
    });

    // 荳翫せ繧ｯ繝ｭ繝ｼ繝ｫ蜿ｯ閭ｽ縺ｪ蝣ｴ蜷・笆ｲ 繧定｡ｨ遉ｺ
    if scroll > 0 {
        render_list.push(Renderable::Text {
            text: "笆ｲ".to_string(),
            anchor: Anchor::TopCenter,
            shift: Shift { x: 0.45, y: content_y - line_h },
            align: Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Top },
            font_size: FontSize::WindowHeight(0.035),
            color: 0xFF_888888,
        });
    }

    // 荳九せ繧ｯ繝ｭ繝ｼ繝ｫ蜿ｯ閭ｽ縺ｪ蝣ｴ蜷・笆ｼ 繧定｡ｨ遉ｺ
    if scroll + max_lines < total_lines {
        render_list.push(Renderable::Text {
            text: "笆ｼ".to_string(),
            anchor: Anchor::TopCenter,
            shift: Shift { x: 0.45, y: content_y + max_lines as f32 * line_h },
            align: Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Top },
            font_size: FontSize::WindowHeight(0.035),
            color: 0xFF_888888,
        });
    }
}

// Segment 縺ｮ base 繝・く繧ｹ繝医ｒ譁・ｭ怜・縺ｨ縺励※霑斐☆・・nno 縺ｯ inner 縺ｮ base 繧帝｣邨撰ｼ・fn segment_base_text(seg: &Segment) -> String {
    match seg {
        Segment::Plain { text } => text.clone(),
        Segment::Annotated { base, .. } => base.clone(),
        Segment::Anno { inner, .. } => inner.iter().map(|s| segment_base_text(s)).collect(),
    }
}

// Segment 縺ｮ reading 繝・く繧ｹ繝医ｒ譁・ｭ怜・縺ｨ縺励※霑斐☆・・nno 縺ｯ inner 縺ｮ reading 繧帝｣邨撰ｼ・fn segment_reading_text(seg: &Segment) -> String {
    match seg {
        Segment::Plain { text } => text.clone(),
        Segment::Annotated { reading, .. } => reading.clone(),
        Segment::Anno { inner, .. } => inner.iter().map(|s| segment_reading_text(s)).collect(),
    }
}

// 陦ｨ遉ｺ逕ｨ縺ｮ (base_text, ruby_text, anno_text) 繧定ｿ斐☆
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
        // --- 繝ｬ繧､繧｢繧ｦ繝郁ｪｿ謨ｴ蛟､ ---
        let v_offset = 0.08; // UI蜈ｨ菴薙ｒ荳九↓縺壹ｉ縺吝牡蜷・
        // --- 蝠城｡後ち繧､繝医Ν陦ｨ遉ｺ ---
        render_list.push(Renderable::BigText {
            text: model.content.title.to_string(),
            anchor: Anchor::TopCenter,
            shift: Shift { x: 0.0, y: 0.01 },
            align: Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Top },
            font_size: FontSize::WindowHeight(0.12),
            color: ACTIVE_COLOR,
        });

        let base_font_size = FontSize::WindowHeight(BASE_FONT_SIZE_RATIO);
        let base_pixel_font_size = calculate_pixel_font_size(base_font_size, width, height);
        let line_idx = model.status.line as usize;
        let content_line = if let Some(line) = model.content.lines.get(line_idx) { line } else { return; };
        let correctness_line = if let Some(line) = model.typing_correctness.lines.get(line_idx) { line } else { return; };
        let status = &model.status;
        let cached_cache = match app.scroll_cache.as_ref() {
            Some(ScrollCache::Ready(state)) if state.current.line == status.line => Some(state),
            _ => None,
        };
        let line_origin = cached_cache.map(|state| state.line_origin as f64).unwrap_or(0.0);
        let target_line_total_width = match cached_cache {
            Some(state) => (state.current.total_width + state.gap_width) as u32,
            None => content_line
                .words
                .iter()
                .flat_map(|w| &w.segments)
                .map(|seg| {
                    let text = segment_base_text(seg);
                    gui_renderer::measure_text(font, &text, base_pixel_font_size).0
                })
                .sum::<u32>() + width as u32,
        };
        let scroll_offset = if cached_cache.is_some() {
            (model.scroll.scroll - line_origin) as f32
        } else {
            model.scroll.scroll as f32
        };
        // --- 荳頑ｮｵ・育岼讓吶ユ繧ｭ繧ｹ繝茨ｼ峨・讒狗ｯ・---
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
                        // 縺薙・繝ｯ繝ｼ繝牙・縺ｮ螳御ｺ・ｸ医∩繧ｻ繧ｰ繝｡繝ｳ繝・                        if correctness_line.words.get(word_idx)
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

        // --- 荳区ｮｵ・亥・蜉帙ユ繧ｭ繧ｹ繝茨ｼ峨・讒狗ｯ・---
        let mut lower_segments = Vec::new();
            let status_word = usize::try_from(status.word).ok();
        let status_segment = usize::try_from(status.segment).ok().unwrap_or(0);
        let status_segment_opt = usize::try_from(status.segment).ok();

        if let Some(state) = cached_cache {
            let current_cache = &state.current;
            let status_word_idx = status_word.unwrap_or(usize::MAX);
            let cursor_x = state.cursor_in_line.max(0.0);
            let viewport = width as f32;
            let left_bound = (cursor_x - viewport).max(0.0);
            let right_bound = cursor_x + viewport;

            let active_segment_idx = match (status_word, status_segment_opt) {
                (Some(word_idx), Some(segment_idx)) if word_idx < current_cache.word_segment_starts.len() => {
                    let word_start = current_cache
                        .word_segment_starts
                        .get(word_idx)
                        .copied()
                        .unwrap_or(current_cache.segments.len());
                    let word_end = current_cache
                        .word_segment_starts
                        .get(word_idx + 1)
                        .copied()
                        .unwrap_or(current_cache.segments.len());
                    let segment_count = word_end.saturating_sub(word_start);
                    let status_offset = segment_idx.min(segment_count);
                    word_start + status_offset
                }
                _ => current_cache.segments.len(),
            };

            let mut visible_start = current_cache
                .segment_prefix_width
                .partition_point(|value| *value < left_bound);
            let mut visible_end = current_cache
                .segment_prefix_width
                .partition_point(|value| *value <= right_bound);
            visible_start = visible_start.min(active_segment_idx);
            visible_end = visible_end.min(active_segment_idx).min(current_cache.segments.len());
            if visible_end < visible_start {
                visible_end = visible_start;
            }

            if active_segment_idx < visible_start {
                visible_start = active_segment_idx;
            }
            if active_segment_idx > visible_end {
                visible_end = active_segment_idx;
            }

            for cache_index in visible_start..visible_end {
                if let Some(cache_seg) = current_cache.segments.get(cache_index) {
                    let is_correct = match (status_word, correctness_line.words.get(cache_seg.word_index)) {
                        (Some(current_status_word), Some(correctness_word))
                            if cache_seg.word_index < current_status_word =>
                        {
                            is_word_correct(correctness_word)
                        }
                        (_, Some(correctness_word))
                            if cache_seg.word_index == status_word_idx =>
                        {
                            correctness_word
                                .segments
                                .get(cache_seg.segment_index)
                                .map_or(false, is_segment_correct)
                        }
                        _ => false,
                    };

                    lower_segments.push(LowerTypingSegment::Completed {
                        base_text: cache_seg.base_text.clone(),
                        ruby_text: cache_seg.ruby_text.clone(),
                        is_correct,
                        width: cache_seg.base_width as u32,
                    });
                }
            }

            if let Some(active_word_content) = content_line.words.get(status_word.unwrap_or(usize::MAX)) {
                let active_word_idx = status_word.unwrap_or(0);
                if active_word_idx < correctness_line.words.len() {
                    let active_correctness_word = &correctness_line.words[active_word_idx];
                    if let Some(active_seg_content) = active_word_content.segments.get(status_segment) {
                        let reading_text = segment_reading_text(active_seg_content);
                        let mut active_elements = Vec::new();

                        let correctness_seg = &active_correctness_word.segments[status_segment];
                        for (char_idx, character) in reading_text
                            .chars()
                            .enumerate()
                            .take(status.char_ as usize)
                        {
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
            }
        } else {
            for word_idx in 0..status_word.unwrap_or(0) {
                if let (Some(word), Some(correctness_word)) = (
                    content_line.words.get(word_idx),
                    correctness_line.words.get(word_idx),
                ) {
                    for seg in &word.segments {
                        let (base_text, ruby_text, _) = segment_display_parts(seg);
                        let seg_width = gui_renderer::measure_text(font, &base_text, base_pixel_font_size).0;
                        lower_segments.push(LowerTypingSegment::Completed {
                            base_text,
                            ruby_text,
                            is_correct: is_word_correct(correctness_word),
                            width: seg_width,
                        });
                    }
                }
            }

            if let (Some(active_word_content), Some(active_correctness_word)) = (
                status_word.and_then(|word_idx| content_line.words.get(word_idx)),
                status_word.and_then(|word_idx| correctness_line.words.get(word_idx)),
            ) {
                for seg_idx in 0..status_segment {
                    if let Some(seg) = active_word_content.segments.get(seg_idx) {
                        let (base_text, ruby_text, _) = segment_display_parts(seg);
                        let is_correct = active_correctness_word
                            .segments
                            .get(seg_idx)
                            .map_or(false, is_segment_correct);
                        lower_segments.push(LowerTypingSegment::Completed {
                            base_text,
                            ruby_text,
                            is_correct,
                            width: gui_renderer::measure_text(font, &base_text, base_pixel_font_size).0,
                        });
                    }
                }

                if let Some(active_seg_content) = active_word_content.segments.get(status_segment) {
                    let reading_text = segment_reading_text(active_seg_content);
                    let mut active_elements = Vec::new();

                    let correctness_seg = &active_correctness_word.segments[status_segment];
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

        // --- 繧ｳ繝ｳ繝・く繧ｹ繝郁｡鯉ｼ亥燕蠕後・陦鯉ｼ峨ｒ謠冗判 ---
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
        
        // --- 繧ｹ繝・・繧ｿ繧ｹ繝代ロ繝ｫ ---
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

        // --- 騾ｲ謐励ヰ繝ｼ ---
        let char_progress_in_line = model.status.word as f32 / content_line.words.len().max(1) as f32;
        let detailed_progress_ratio = if line_count > 0 {
            (model.status.line as f32 + char_progress_in_line) / (line_count as f32)
        } else {
            0.0
        };

        render_list.push(Renderable::ProgressBar {
            anchor: Anchor::BottomLeft,
            shift: Shift { x: 0.0, y: -0.005 }, // 蟆代＠縺縺大ｺ輔°繧画ｵｮ縺九○繧・            width_ratio: 1.0,
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


/// Anchor縺ｨShift縺九ｉ縲∝渕貅悶→縺ｪ繧句ｺｧ讓・x, y)繧定ｨ育ｮ励☆繧・pub fn calculate_anchor_position(
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

/// 蝓ｺ貅也せ縲√ユ繧ｭ繧ｹ繝医・蟇ｸ豕輔∵純縺域婿縺九ｉ縲∵怙邨ら噪縺ｪ謠冗判髢句ｧ句ｺｧ讓呻ｼ亥ｷｦ荳奇ｼ峨ｒ險育ｮ励☆繧・pub fn calculate_aligned_position(
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

