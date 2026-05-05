// src/ui.rs

// uefi feature驍ｵ・ｺ隴ｴ・ｧ隲､蜑ｰ諤上・・ｹ驍ｵ・ｺ繝ｻ・ｪ髯懶ｽ｣繝ｻ・ｴ髯ｷ・ｷ陋ｹ・ｻ・つ遶擾ｽｵ繝ｻ・ｨ陷ｻ雜｣・ｽ・ｺ隰費ｽｶ郢晢ｽｻalloc驛｢・ｧ繝ｻ・ｯ驛｢譎｢・ｽ・ｬ驛｢譎｢・ｽ・ｼ驛｢譎冗樟繝ｻ蝣､・ｹ・ｧ繝ｻ・､驛｢譎｢・ｽ・ｳ驛｢譎・ｺ｢郢晢ｽｻ驛｢譏ｴ繝ｻ
#[cfg(feature = "uefi")]
extern crate alloc;

// uefi 驍ｵ・ｺ繝ｻ・ｧ f64::floor() 驛｢・ｧ陷代・・ｽ・ｽ繝ｻ・ｿ驍ｵ・ｺ郢晢ｽｻ隨ｳ繝ｻ・ｹ・ｧ遶丞｣ｺ繝ｻ髯滂ｽ｢郢晢ｽｻ繝ｻ・ｦ郢晢ｽｻ
#[cfg(feature = "uefi")]
use core_maths::CoreFloat;

// uefi 驍ｵ・ｺ繝ｻ・ｨ std 驍ｵ・ｺ繝ｻ・ｧ髣厄ｽｴ繝ｻ・ｿ鬨ｾ蛹・ｽｽ・ｨ驍ｵ・ｺ陷ｷ・ｶ繝ｻ繝ｻVec 驍ｵ・ｺ繝ｻ・ｨ vec! 驛｢・ｧ髮区ｧｭ繝ｻ驛｢・ｧ鬯・､ｧ・ｴ蟶ｷ・ｸ・ｺ陋ｹ・ｻ繝ｻ繝ｻ
#[cfg(feature = "uefi")]
use alloc::vec::Vec;
#[cfg(not(feature = "uefi"))]
use std::vec::Vec;

// uefi 驍ｵ・ｺ繝ｻ・ｨ std 驍ｵ・ｺ繝ｻ・ｧ髣厄ｽｴ繝ｻ・ｿ鬨ｾ蛹・ｽｽ・ｨ驍ｵ・ｺ陷ｷ・ｶ繝ｻ繝ｻString 驍ｵ・ｺ繝ｻ・ｨ format! 驛｢・ｧ髮区ｧｭ繝ｻ驛｢・ｧ鬯・､ｧ・ｴ蟶ｷ・ｸ・ｺ陋ｹ・ｻ繝ｻ繝ｻ
#[cfg(feature = "uefi")]
use alloc::{
    format,
    string::{String, ToString},
};
#[cfg(not(feature = "uefi"))]
use std::string::{String, ToString};

use crate::app::{typing_line_scroll_offset, App, AppSnapshot, AppState, Script, ScrollCache};
use crate::model::{
    Segment, TypingCorrectnessChar, TypingCorrectnessSegment, TypingCorrectnessWord,
};
use crate::renderer::{calculate_pixel_font_size, gui_renderer};
use crate::typing; // For calculate_total_metrics
use ab_glyph::FontVec;

/// 鬨ｾ蛹・ｽｽ・ｻ鬯ｮ・ｱ繝ｻ・｢髣包ｽｳ驗呻ｽｫ郢晢ｽｻ髫ｰ・ｰ陷諤懈・髯憺屮・ｽ・ｺ髮九・・ｹ貅倪雷驛｢・ｧ髮区ｩｸ・ｽ・ｮ陞溘ｑ・ｽ・ｾ繝ｻ・ｩ驍ｵ・ｺ陷ｷ・ｶ繝ｻ闖穫um
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

/// Anchor驍ｵ・ｺ闕ｵ譎｢・ｽ閾･・ｸ・ｺ繝ｻ・ｮ驛｢・ｧ繝ｻ・ｪ驛｢譎・ｽｼ譁絶落驛｢譏ｴ繝ｻ郢晢ｽｨ郢晢ｽｻ髢ｧ・ｲ繝ｻ・ｧ繝ｻ・ｻ髯ｷ閧ｴ蝮ｩ郤・ｽｼ郢晢ｽｻ陝ｲ・ｨ繝ｻ螳壽･懆棔繧托ｽｽ・ｾ繝ｻ・ｩ驍ｵ・ｺ陷ｷ・ｶ繝ｻ邇厄ｽｮ蝣､豢ｸ・つ繝ｻ・ｰ髣厄ｽｴ郢晢ｽｻ
#[derive(Clone, Copy)]
pub struct Shift {
    pub x: f32,
    pub y: f32,
}

/// 髮朱ｯ会ｽｽ・ｴ髯晢ｽｷ繝ｻ・ｳ髫ｴ繝ｻ・ｽ・ｹ髯ｷ・ｷ闔会ｽ｣郢晢ｽｻ髫ｰ・ｰ郢晢ｽｻ遶擾ｽｴ
#[derive(Clone, Copy)]
pub enum HorizontalAlign {
    Left,
    Center,
    Right,
}

/// 髯懷生・芽浚・ｩ髫ｴ繝ｻ・ｽ・ｹ髯ｷ・ｷ闔会ｽ｣郢晢ｽｻ髫ｰ・ｰ郢晢ｽｻ遶擾ｽｴ
#[derive(Clone, Copy)]
pub enum VerticalAlign {
    Top,
    Center,
    Bottom,
}

/// 驛｢譏ｴ繝ｻ邵ｺ蜀暦ｽｹ・ｧ繝ｻ・ｹ驛｢譎冗樟郢晢ｽｻ髫ｰ・ｰ郢晢ｽｻ遶擾ｽｴ髫ｴ繝ｻ・ｽ・ｹ驛｢・ｧ髮区ｩｸ・ｽ・ｮ陞溘ｑ・ｽ・ｾ繝ｻ・ｩ驍ｵ・ｺ陷ｷ・ｶ繝ｻ邇厄ｽｮ蝣､豢ｸ・つ繝ｻ・ｰ髣厄ｽｴ郢晢ｽｻ
#[derive(Clone, Copy)]
pub struct Align {
    pub horizontal: HorizontalAlign,
    pub vertical: VerticalAlign,
}

/// 驛｢譎・ｽｼ譁青ｰ驛｢譎｢・ｽ・ｳ驛｢譎冗樟邵ｺ遉ｼ・ｹ・ｧ繝ｻ・､驛｢・ｧ繝ｻ・ｺ驍ｵ・ｺ繝ｻ・ｮ髯憺屮・ｽ・ｺ髮九・縺倥・螳壽･懆棔繧托ｽｽ・ｾ繝ｻ・ｩ驍ｵ・ｺ陷ｷ・ｶ繝ｻ闖穫um
#[derive(Clone, Copy)]
pub enum FontSize {
    /// 驛｢・ｧ繝ｻ・ｦ驛｢・ｧ繝ｻ・｣驛｢譎｢・ｽ・ｳ驛｢譎擾ｽｳ・ｨ邵ｺ閧ｲ・ｸ・ｺ繝ｻ・ｮ鬯ｯ・ｮ陋滂ｽ･繝ｻ繝ｻ・ｸ・ｺ繝ｻ・ｫ髯昴・・ｽ・ｾ驍ｵ・ｺ陷ｷ・ｶ繝ｻ邇厄ｽｱ閧ｲ蝮ｩ驍擾ｽｫ
    WindowHeight(f32),
    /// 驛｢・ｧ繝ｻ・ｦ驛｢・ｧ繝ｻ・｣驛｢譎｢・ｽ・ｳ驛｢譎擾ｽｳ・ｨ邵ｺ閧ｲ・ｸ・ｺ繝ｻ・ｮ鬯ｮ・ｱ繝ｻ・｢鬩包ｽｨ鬮ｦ・ｪ郢晢ｽｻ髯晢ｽｷ繝ｻ・ｳ髫ｴ繝ｻ・ｽ・ｹ髫ｴ・ｬ繝ｻ・ｹ驍ｵ・ｺ繝ｻ・ｫ髯昴・・ｽ・ｾ驍ｵ・ｺ陷ｷ・ｶ繝ｻ邇厄ｽｱ閧ｲ蝮ｩ驍擾ｽｫ
    WindowAreaSqrt(f32),
}

/// 驛｢・ｧ繝ｻ・ｰ驛｢譎｢・ｽ・ｩ驛｢譏ｴ繝ｻ郢晢ｽｻ驛｢・ｧ繝ｻ・ｷ驛｢譎｢・ｽ・ｧ驛｢譎｢・ｽ・ｳ驍ｵ・ｺ繝ｻ・ｮ髯橸ｽｳ陞溘ｑ・ｽ・ｾ繝ｻ・ｩ
#[derive(Clone, Copy)]
pub struct Gradient {
    pub start_color: u32,
    pub end_color: u32,
}

/// 髣包ｽｳ鬯・汚・ｽ・ｮ繝ｻ・ｵ郢晢ｽｻ髢ｧ・ｲ陝ｯ・ｼ髫ｶ轣倡函郢晢ｽｦ驛｢・ｧ繝ｻ・ｭ驛｢・ｧ繝ｻ・ｹ驛｢譎∬か繝ｻ・ｼ陝ｲ・ｨ郢晢ｽｻ驛｢・ｧ繝ｻ・ｻ驛｢・ｧ繝ｻ・ｰ驛｢譎｢・ｽ・｡驛｢譎｢・ｽ・ｳ驛｢譎冗樟郢晢ｽｻ髴托ｽ･繝ｻ・ｶ髫ｲ・ｷ郢晢ｽｻ
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

/// 髣包ｽｳ鬯・汚・ｽ・ｮ繝ｻ・ｵ郢晢ｽｻ髢ｧ・ｲ陝ｯ・ｼ髫ｶ轣倡函郢晢ｽｦ驛｢・ｧ繝ｻ・ｭ驛｢・ｧ繝ｻ・ｹ驛｢譎∬か繝ｻ・ｼ陝ｲ・ｨ繝ｻ螳夲ｽｮ雋樒私郢晢ｽｻ驍ｵ・ｺ陷ｷ・ｶ繝ｻ迢暦ｽｸ・ｲ遶丞｢・刮・ｹ譎・ｽｬ雜｣・ｽ・ｻ陋滂ｽ･遯ｶ・ｳ驍ｵ・ｺ繝ｻ・ｮ1驛｢・ｧ繝ｻ・ｻ驛｢・ｧ繝ｻ・ｰ驛｢譎｢・ｽ・｡驛｢譎｢・ｽ・ｳ驛｢譏ｴ繝ｻ
pub struct UpperTypingSegment {
    pub base_text: String,
    pub ruby_text: Option<String>,
    /// anno鬮ｫ・ｪ闖ｫ・ｶ繝ｻ・ｳ髴域鱒繝ｻ髮主桁・ｽ・ｨ鬯ｩ・･陋ｹ・ｻ郢晢ｽｦ驛｢・ｧ繝ｻ・ｭ驛｢・ｧ繝ｻ・ｹ驛｢譎∬か繝ｻ・ｼ陋ｹ・ｻ郢晢ｽｻ驛｢譎｢・ｽ・ｼ驛｢・ｧ繝ｻ・ｹ驛｢譏ｴ繝ｻ邵ｺ蜀暦ｽｹ・ｧ繝ｻ・ｹ驛｢譎冗樟郢晢ｽｻ髣包ｽｳ闕ｵ譏ｶ繝ｻ鬮ｯ・ｦ繝ｻ・ｨ鬩穂ｼ夲ｽｽ・ｺ郢晢ｽｻ郢晢ｽｻ
    pub anno_text: Option<String>,
    pub state: UpperSegmentState,
}

/// 髣包ｽｳ陋ｹ・ｺ繝ｻ・ｮ繝ｻ・ｵ郢晢ｽｻ闔・･郢晢ｽｻ髯ｷ迚呻ｽｸ蜷ｶﾎ倬Δ・ｧ繝ｻ・ｭ驛｢・ｧ繝ｻ・ｹ驛｢譎∬か繝ｻ・ｼ陝ｲ・ｨ郢晢ｽｻ驛｢・ｧ繝ｻ・｢驛｢・ｧ繝ｻ・ｯ驛｢譏ｴ繝ｻ邵ｺ繝ｻ・ｹ譎冗ｧ√・・ｼ髢ｧ・ｲ隶捺ｺｯ闊峨・・ｨ髯ｷ闌ｨ・ｽ・･髯ｷ迚呻ｽｸ謇假ｽｽ・ｸ繝ｻ・ｭ郢晢ｽｻ陝ｲ・ｨ邵ｺ譎会ｽｹ・ｧ繝ｻ・ｰ驛｢譎｢・ｽ・｡驛｢譎｢・ｽ・ｳ驛｢譎冗樟繝ｻ螳夲ｽｮ雋樒私郢晢ｽｻ驍ｵ・ｺ陷ｷ・ｶ繝ｻ遏ｩ蝗守ｫ擾ｽｫ繝ｻ・ｴ繝ｻ・ｰ
pub enum ActiveLowerElement {
    /// 驛｢・ｧ繝ｻ・ｿ驛｢・ｧ繝ｻ・､驛｢譎芽ｻｸ繝ｻ・ｸ陋ｹ・ｻ遶擾ｽｩ驍ｵ・ｺ繝ｻ・ｮ髫ｴ竏壹・繝ｻ・ｭ隴会ｽｦ繝ｻ・ｼ陜捺ｻゑｽｽ・ｭ繝ｻ・｣鬮ｫ・ｱ繝ｻ・､髫ｲ・ｰ郢晢ｽｻ繝ｻ・ｰ繝ｻ・ｱ髣皮甥ﾂ・･遯ｶ・ｳ郢晢ｽｻ郢晢ｽｻ
    Typed { character: char, is_correct: bool },
    /// 驛｢・ｧ繝ｻ・ｫ驛｢譎｢・ｽ・ｼ驛｢・ｧ繝ｻ・ｽ驛｢譎｢・ｽ・ｫ
    Cursor,
    /// 髫ｴ蟷｢・ｽ・ｪ鬩墓慣・ｽ・ｺ髯橸ｽｳ陞｢・ｹ郢晢ｽｻ驛｢譎｢・ｽ・ｭ驛｢譎｢・ｽ・ｼ驛｢譎・ｽｧ・ｫ繝ｻ・ｭ隲､諛翫・髯ｷ蟲ｨ繝ｻ(髣懆侭繝ｻ "k", "ky")
    UnconfirmedInput(String),
    /// 鬨ｾ・ｶ繝ｻ・ｴ髯ｷ鮃ｹ莠らｹ晢ｽｻ鬮ｫ・ｱ繝ｻ・､髯ｷ闌ｨ・ｽ・･髯ｷ迚呻ｽｸ蜷ｶ・･驛｢譎｢・ｽ・ｼ
    LastIncorrectInput(char),
}

/// 髣包ｽｳ陋ｹ・ｺ繝ｻ・ｮ繝ｻ・ｵ郢晢ｽｻ闔・･郢晢ｽｻ髯ｷ迚呻ｽｸ蜷ｶﾎ倬Δ・ｧ繝ｻ・ｭ驛｢・ｧ繝ｻ・ｹ驛｢譎∬か繝ｻ・ｼ陝ｲ・ｨ繝ｻ螳夲ｽｮ雋樒私郢晢ｽｻ驍ｵ・ｺ陷ｷ・ｶ繝ｻ迢暦ｽｹ・ｧ繝ｻ・ｻ驛｢・ｧ繝ｻ・ｰ驛｢譎｢・ｽ・｡驛｢譎｢・ｽ・ｳ驛｢譏ｴ繝ｻ
pub enum LowerTypingSegment {
    /// 髯橸ｽｳ陟包ｽ｡繝ｻ・ｺ郢晢ｽｻ繝ｻ・ｸ陋ｹ・ｻ遶擾ｽｩ驍ｵ・ｺ繝ｻ・ｮ驛｢・ｧ繝ｻ・ｻ驛｢・ｧ繝ｻ・ｰ驛｢譎｢・ｽ・｡驛｢譎｢・ｽ・ｳ驛｢譏ｴ繝ｻ
    Completed {
        base_text: String,
        ruby_text: Option<String>,
        is_correct: bool,
        width: u32,
    },
    /// 髴托ｽｴ繝ｻ・ｾ髯懶ｽｨ繝ｻ・ｨ髯ｷ闌ｨ・ｽ・･髯ｷ迚呻ｽｸ謇假ｽｽ・ｸ繝ｻ・ｭ驍ｵ・ｺ繝ｻ・ｮ驛｢・ｧ繝ｻ・｢驛｢・ｧ繝ｻ・ｯ驛｢譏ｴ繝ｻ邵ｺ繝ｻ・ｹ譎・§遶企・・ｹ・ｧ繝ｻ・ｻ驛｢・ｧ繝ｻ・ｰ驛｢譎｢・ｽ・｡驛｢譎｢・ｽ・ｳ驛｢譏ｴ繝ｻ
    Active { elements: Vec<ActiveLowerElement> },
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

/// 鬨ｾ蛹・ｽｽ・ｻ鬯ｮ・ｱ繝ｻ・｢驍ｵ・ｺ繝ｻ・ｫ髫ｰ・ｰ陷諤懈・驍ｵ・ｺ陷ｷ・ｶ遶冗距・ｸ・ｺ陝雜｣・ｽ・ｦ遶擾ｽｫ繝ｻ・ｴ繝ｻ・ｰ驍ｵ・ｺ繝ｻ・ｮ鬩墓ｩｸ・ｽ・ｮ鬯ｯ菫ｶ・ｧ・ｭ遶雁､・ｹ譎｢・ｽ・ｬ驛｢・ｧ繝ｻ・､驛｢・ｧ繝ｻ・｢驛｢・ｧ繝ｻ・ｦ驛｢譎乗ｲｺ郢晢ｽ･髯懶ｽ｣繝ｻ・ｱ驛｢・ｧ髮区ｩｸ・ｽ・ｮ陞溘ｑ・ｽ・ｾ繝ｻ・ｩ驍ｵ・ｺ陷ｷ・ｶ繝ｻ闖穫um
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
    /// 髣包ｽｳ鬯・汚・ｽ・ｮ繝ｻ・ｵ驍ｵ・ｺ繝ｻ・ｮ鬨ｾ・ｶ繝ｻ・ｮ髫ｶ轣倡函郢晢ｽｦ驛｢・ｧ繝ｻ・ｭ驛｢・ｧ繝ｻ・ｹ驛｢譎槭Γ繝ｻ・｡隰疲ｺ倥・髣厄ｽｴ髦ｮ蜻ｻ・ｽ蟶晏距繝ｻ・ｨ驍ｵ・ｺ陷ｷ譎・ｽ｢繝ｻ
    TypingUpper {
        segments: Vec<UpperTypingSegment>,
        anchor: Anchor,
        shift: Shift,
        align: Align,
        font_size: FontSize, // 驛｢譎冗函郢晢ｽｻ驛｢・ｧ繝ｻ・ｹ驛｢譏ｴ繝ｻ邵ｺ蜀暦ｽｹ・ｧ繝ｻ・ｹ驛｢譎冗樟郢晢ｽｻ驛｢譎・ｽｼ譁青ｰ驛｢譎｢・ｽ・ｳ驛｢譎冗樟邵ｺ遉ｼ・ｹ・ｧ繝ｻ・､驛｢・ｧ繝ｻ・ｺ
        line_width: u32,
    },
    /// 髣包ｽｳ陋ｹ・ｺ繝ｻ・ｮ繝ｻ・ｵ驍ｵ・ｺ繝ｻ・ｮ髯ｷ闌ｨ・ｽ・･髯ｷ迚呻ｽｸ蜷ｶﾎ倬Δ・ｧ繝ｻ・ｭ驛｢・ｧ繝ｻ・ｹ驛｢譎槭Γ繝ｻ・｡隰疲ｺ倥・髣厄ｽｴ髦ｮ蜻ｻ・ｽ蟶晏距繝ｻ・ｨ驍ｵ・ｺ陷ｷ譎・ｽ｢繝ｻ
    TypingLower {
        segments: Vec<LowerTypingSegment>,
        anchor: Anchor,
        shift: Shift,
        align: Align,
        font_size: FontSize, // 髯ｷ闌ｨ・ｽ・･髯ｷ迚呻ｽｸ蜷ｶﾎ倬Δ・ｧ繝ｻ・ｭ驛｢・ｧ繝ｻ・ｹ驛｢譎冗樟郢晢ｽｻ驛｢譎・ｽｼ譁青ｰ驛｢譎｢・ｽ・ｳ驛｢譎冗樟邵ｺ遉ｼ・ｹ・ｧ繝ｻ・､驛｢・ｧ繝ｻ・ｺ
        line_alignment: TypingLineAlignment,
    },
    ProgressBar {
        anchor: Anchor,
        shift: Shift,
        width_ratio: f32, // 鬨ｾ蛹・ｽｽ・ｻ鬯ｮ・ｱ繝ｻ・｢髯晢ｽｷ郢晢ｽｻ遶頑･｢豎槭・・ｾ驍ｵ・ｺ陷ｷ・ｶ繝ｻ邇厄ｽｱ閧ｲ蝮ｩ驍擾ｽｫ
        height_ratio: f32, // 鬨ｾ蛹・ｽｽ・ｻ鬯ｮ・ｱ繝ｻ・｢鬯ｯ・ｮ陋滂ｽ･繝ｻ繝ｻ・ｸ・ｺ繝ｻ・ｫ髯昴・・ｽ・ｾ驍ｵ・ｺ陷ｷ・ｶ繝ｻ邇厄ｽｱ閧ｲ蝮ｩ驍擾ｽｫ
        progress: f32,     // 0.0 to 1.0
        bg_color: u32,
        fg_color: u32,
    },
}

/// How to Use 鬨ｾ蛹・ｽｽ・ｻ鬯ｮ・ｱ繝ｻ・｢驍ｵ・ｺ繝ｻ・ｫ鬮ｯ・ｦ繝ｻ・ｨ鬩穂ｼ夲ｽｽ・ｺ驍ｵ・ｺ陷ｷ・ｶ繝ｻ邇匁・郢晢ｽｻ繝ｻ・｡郢晢ｽｻ (驛｢譏ｴ繝ｻ邵ｺ蜀暦ｽｹ・ｧ繝ｻ・ｹ驛｢譏ｴ繝ｻ 雎ｼ・ｶ繝ｻ・ｲ) 驍ｵ・ｺ繝ｻ・ｮ髯橸ｽｳ陞溘ｑ・ｽ・ｾ繝ｻ・ｩ驍ｵ・ｲ郢晢ｽｻ/// app.rs 驍ｵ・ｺ闕ｵ譎｢・ｽ陋ｾ蜍苓ｭｴ・ｧ霎溷､頑╂郢ｧ蟲ｨ繝ｻ驍ｵ・ｺ繝ｻ・ｮ驍ｵ・ｺ雋・∞・ｽ竏ｫ・ｸ・ｺ繝ｻ・ｫ pub(crate) 驍ｵ・ｺ繝ｻ・ｧ髯ｷ闌ｨ・ｽ・ｬ鬯ｮ・｢闕ｵ譏ｶ繝ｻ驛｢・ｧ闕ｵ謨鳴郢晢ｽｻ
pub(crate) const HOW_TO_USE_CONTENT: &[(&str, u32)] = &[
    // 髫ｨ貂可髫ｨ貂可髫ｨ貂可 髯憺屮・ｽ・ｺ髫ｴ蟷｢・ｽ・ｬ髫ｰ・ｫ陜｣・ｺ繝ｻ・ｽ郢晢ｽｻ髫ｨ貂可髫ｨ貂可髫ｨ貂可
    ("[ 髯憺屮・ｽ・ｺ髫ｴ蟷｢・ｽ・ｬ髫ｰ・ｫ陜｣・ｺ繝ｻ・ｽ郢晢ｽｻ]",                              0xFF_FFDD88),
    ("",                                          0xFF_000000),
    // 髫ｨ貂可髫ｨ貂可髫ｨ貂可 驛｢・ｧ繝ｻ・ｿ驛｢・ｧ繝ｻ・､驛｢譎・ｱ抵ｾ趣ｽｦ驛｢・ｧ繝ｻ・ｰ 髫ｨ貂可髫ｨ貂可髫ｨ貂可
    ("[ 驛｢・ｧ繝ｻ・ｿ驛｢・ｧ繝ｻ・､驛｢譎・ｱ抵ｾ趣ｽｦ驛｢・ｧ繝ｻ・ｰ ]",                            0xFF_FFDD88),
    ("  髣懆侭繝ｻ 驍ｵ・ｺ鬩ｫﾂ郢晢ｽｻka  驍ｵ・ｺ陜ｮ繧・・ki  驍ｵ・ｺ陞ｽ・ｯ郢晢ｽｻga  驍ｵ・ｺ繝ｻ・ｯ驕ｶ鬆托ｽｪﾂa",        0xFF_888888),
    ("  鬨ｾ蜈ｷ・ｽ・ｽ雎ｼ・ｶ繝ｻ・ｲ : 髴托ｽｴ繝ｻ・ｾ髯懶ｽｨ繝ｻ・ｨ驍ｵ・ｺ繝ｻ・ｮ髯ｷ闌ｨ・ｽ・･髯ｷ迚呻ｽｸ謇假ｽｽ・ｽ陷･・ｲ繝ｻ・ｽ繝ｻ・ｮ",                  0xFF_FFFFFF),
    ("",                                          0xFF_000000),
    // 髫ｨ貂可髫ｨ貂可髫ｨ貂可 髯懶｣ｰ陜楢ｶ｣・ｽ・｡驕停沖繝ｻ髫ｰ螢ｹ繝ｻ髫ｨ貂可髫ｨ貂可髫ｨ貂可
    ("[ 髯懶｣ｰ陜楢ｶ｣・ｽ・｡驕停沖繝ｻ髫ｰ螢ｹ繝ｻ]",                              0xFF_FFDD88),
    ("  X     : 驛｢・ｧ繝ｻ・ｫ驛｢・ｧ繝ｻ・ｹ驛｢・ｧ繝ｻ・ｿ驛｢譎｢・｣・ｰ髯懶｣ｰ陜楢ｶ｣・ｽ・｡陟暮ｯ会ｽｽ螳壽≧闔ｨ竏晄ｱ・",              0xFF_CCCCCC),
    ("  U / D : 驛｢・ｧ繝ｻ・ｫ驛｢・ｧ繝ｻ・ｹ驛｢・ｧ繝ｻ・ｿ驛｢譎｢・｣・ｰ髯懶｣ｰ陜楢ｶ｣・ｽ・｡陟募ｾ後・鬯ｯ繝ｻ繝ｻ繝ｻ・ｺ闕ｳ螂・ｽｽ螳壽｣秘包ｽｻ陝ｲ・ｩ",        0xFF_CCCCCC),
    ("",                                          0xFF_000000),
    // 髫ｨ貂可髫ｨ貂可髫ｨ貂可 驛｢譎・ｽｼ驥・㏍・ｹ譏ｴ繝ｻ邵ｺ鬘梧ｦ繝ｻ・･髯ｷ蟲ｨ繝ｻ髫ｨ貂可髫ｨ貂可髫ｨ貂可
    ("[ 驛｢譎・ｽｼ驥・㏍・ｹ譏ｴ繝ｻ邵ｺ鬘梧ｦ繝ｻ・･髯ｷ蟲ｨ繝ｻ(驛｢・ｧ繝ｻ・ｿ驛｢譏ｴ繝ｻ郢晢ｽ｡驛｢譏ｴ繝ｻ郢晢ｽｰ驛｢・ｧ繝ｻ・､驛｢・ｧ繝ｻ・ｹ) ]",         0xFF_FFDD88),
    ("  驛｢・ｧ繝ｻ・ｿ驛｢・ｧ繝ｻ・､驛｢譎・ｱ抵ｾ趣ｽｦ驛｢・ｧ繝ｻ・ｰ髣包ｽｳ繝ｻ・ｭ驍ｵ・ｺ繝ｻ・ｫ鬨ｾ蛹・ｽｽ・ｻ鬯ｮ・ｱ繝ｻ・｢髣包ｽｳ驕擾ｽｩ・主､ゑｽｸ・ｺ繝ｻ・ｫ驛｢・ｧ繝ｻ・ｭ驛｢譎｢・ｽ・ｼ驛｢譎・鯵郢晢ｽｻ驛｢譎擾ｽｳ・ｨ遯ｶ・ｲ鬮ｯ・ｦ繝ｻ・ｨ鬩穂ｼ夲ｽｽ・ｺ", 0xFF_CCCCCC),
    ("  髣包ｽｳ繝ｻ・ｭ髯樊ｻゑｽｽ・ｮ驛｢・ｧ繝ｻ・ｿ驛｢譏ｴ繝ｻ郢晢ｽｻ : 驍ｵ・ｺ郢ｧ闌ｨ・ｽ・ｮ繝ｻ・ｵ  (髣懆侭繝ｻ 驍ｵ・ｺ郢晢ｽｻ驕ｶ鄙ｫ繝ｻ驍ｵ・ｺ郢晢ｽｻ",        0xFF_CCCCCC),
    ("  髣包ｽｳ驗呻ｽｫ郢晢ｽｵ驛｢譎｢・ｽ・ｪ驛｢譏ｴ繝ｻ邵ｺ繝ｻ: 驍ｵ・ｺ郢晢ｽｻ繝ｻ・ｮ繝ｻ・ｵ  (髣懆侭繝ｻ 驍ｵ・ｺ郢晢ｽｻ驕ｶ鄙ｫ繝ｻ驍ｵ・ｺ郢晢ｽｻ",        0xFF_CCCCCC),
    ("  髯晢ｽｾ繝ｻ・ｦ驛｢譎・ｽｼ驥・㏍・ｹ譏ｴ繝ｻ邵ｺ繝ｻ: 驍ｵ・ｺ郢晢ｽｻ繝ｻ・ｮ繝ｻ・ｵ  (髣懆侭繝ｻ 驍ｵ・ｺ郢晢ｽｻ驕ｶ鄙ｫ繝ｻ驍ｵ・ｺ郢晢ｽｻ",        0xFF_CCCCCC),
    ("  髯ｷ・ｿ繝ｻ・ｳ驛｢譎・ｽｼ驥・㏍・ｹ譏ｴ繝ｻ邵ｺ繝ｻ: 驍ｵ・ｺ陜捺ｻゑｽｽ・ｮ繝ｻ・ｵ  (髣懆侭繝ｻ 驍ｵ・ｺ郢晢ｽｻ驕ｶ鄙ｫ繝ｻ驍ｵ・ｺ郢晢ｽｻ",        0xFF_CCCCCC),
    ("  髣包ｽｳ闕ｵ譏ｴﾎｨ驛｢譎｢・ｽ・ｪ驛｢譏ｴ繝ｻ邵ｺ繝ｻ: 驍ｵ・ｺ鬯・汚・ｽ・ｮ繝ｻ・ｵ  (髣懆侭繝ｻ 驍ｵ・ｺ郢晢ｽｻ驕ｶ鄙ｫ繝ｻ驍ｵ・ｺ郢晢ｽｻ",        0xFF_CCCCCC),
    ("  髯樊ｻゑｽｽ・ｧ驕ｶ閧ｴ・ｳ魃会ｽｽ・ｰ闕ｳ鄙ｫ・･驛｢譎｢・ｽ・ｼ : 鬨ｾ・ｶ繝ｻ・ｴ髯ｷ鮃ｹ莠らｹ晢ｽｻ鬮ｫ・ｱ繝ｻ・､驛｢・ｧ鬮ｮ竏壹・髯ｷ迚呻ｽｸ蜻ｻ・ｽ螳壽｣秘包ｽｻ鬩ｪ・､",       0xFF_CCCCCC),
    ("           驍ｵ・ｺ鬩ｫﾂ郢晢ｽｻ驍ｵ・ｺ陞ｽ・ｯ郢晢ｽｻ驍ｵ・ｺ郢晢ｽｻ 驍ｵ・ｺ繝ｻ・ｯ驕ｶ髮√・郢晢ｽｻ驕ｶ髮√・郢晢ｽｻ驕ｶ髮√・郢晢ｽｻ  驍ｵ・ｺ繝ｻ・ｪ驍ｵ・ｺ繝ｻ・ｩ", 0xFF_888888),
    ("           (鬨ｾ・ｶ繝ｻ・ｴ髯ｷ鮃ｹ莠らｪｶ・ｲ鬮ｫ・ｱ繝ｻ・､驛｢・ｧ驗呻ｽｫ郢晢ｽｻ驍ｵ・ｺ繝ｻ・ｨ驍ｵ・ｺ鬮ｦ・ｪ郢晢ｽｻ驍ｵ・ｺ繝ｻ・ｿ髣厄ｽｴ隲幢ｽｷ髯後・",     0xFF_888888),
    ("",                                          0xFF_000000),
    // 髫ｨ貂可髫ｨ貂可髫ｨ貂可 驛｢・ｧ繝ｻ・ｫ驛｢・ｧ繝ｻ・ｹ驛｢・ｧ繝ｻ・ｿ驛｢譎｢・｣・ｰ髯懶｣ｰ陜楢ｶ｣・ｽ・｡郢晢ｽｻ髫ｨ貂可髫ｨ貂可髫ｨ貂可
    ("[ 驛｢・ｧ繝ｻ・ｫ驛｢・ｧ繝ｻ・ｹ驛｢・ｧ繝ｻ・ｿ驛｢譎｢・｣・ｰ髯懶｣ｰ陜楢ｶ｣・ｽ・｡郢晢ｽｻ(.ntq 髯溷私・ｽ・｢髯滉ｻ｣繝ｻ ]",              0xFF_FFDD88),
];

#[cfg(target_arch = "wasm32")]
const MENU_ITEMS: [&str; 3] = ["Start Typing", "How to Use", "Settings"];

#[cfg(not(target_arch = "wasm32"))]
const MENU_ITEMS: [&str; 4] = ["Start Typing", "How to Use", "Settings", "Quit"];

// --- 驛｢・ｧ繝ｻ・ｿ驛｢・ｧ繝ｻ・､驛｢譎・ｱ抵ｾ趣ｽｦ驛｢・ｧ繝ｻ・ｰ鬨ｾ蛹・ｽｽ・ｻ鬯ｮ・ｱ繝ｻ・｢驍ｵ・ｺ繝ｻ・ｮ驛｢譎｢・ｽ・ｬ驛｢・ｧ繝ｻ・､驛｢・ｧ繝ｻ・｢驛｢・ｧ繝ｻ・ｦ驛｢譏懶ｽｺ・･繝ｻ・ｮ陞｢・ｽ霎溘・---
pub const BASE_FONT_SIZE_RATIO: f32 = 0.2;
const UPPER_ROW_Y_OFFSET_FACTOR: f32 = 1.3;
const LOWER_ROW_Y_OFFSET_FACTOR: f32 = 0.2;

// --- 雎ｼ・ｶ繝ｻ・ｲ髯橸ｽｳ陞溘ｑ・ｽ・ｾ繝ｻ・ｩ ---
pub const CORRECT_COLOR: u32 = 0xFF_9097FF;
pub const INCORRECT_COLOR: u32 = 0xFF_FF9898;
pub const PENDING_COLOR: u32 = 0xFF_999999;
pub const ACTIVE_COLOR: u32 = 0xFF_FFFFFF;
pub const WRONG_KEY_COLOR: u32 = 0xFF_F55252;
pub const CURSOR_COLOR: u32 = 0xFF_FFFFFF;
pub const UNCONFIRMED_COLOR: u32 = 0xFF_CCCCCC;

/// App驍ｵ・ｺ繝ｻ・ｮ髴托ｽ･繝ｻ・ｶ髫ｲ・ｷ闕ｵ譎｢・ｽ螳壽╂陷会ｽｱ繝ｻ・ｰ髯ｷ・ｿ隰費ｽｶ繝ｻ鬘費ｽｸ・ｲ遶擾ｽｵ驍ｱ蟶敖蛹・ｽｽ・ｻ驛｢譎｢・ｽ・ｪ驛｢・ｧ繝ｻ・ｹ驛｢譎∬か繝ｻ・ｼ郢晢ｽｻI驛｢譎｢・ｽ・ｬ驛｢・ｧ繝ｻ・､驛｢・ｧ繝ｻ・｢驛｢・ｧ繝ｻ・ｦ驛｢譎∬か繝ｻ・ｼ陝ｲ・ｨ繝ｻ螳夲ｽｮ蝣､霍昴・・ｯ陝ｲ・ｨ隨倥・・ｹ・ｧ郢晢ｽｻ
pub fn build_ui(app: &App, font: &FontVec, width: usize, height: usize) -> Vec<Renderable> {
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
            build_typing_ui(app, &mut render_list, typing_gradient, font, width, height)
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

    // --- 鬨ｾ蛹・ｽｽ・ｻ鬯ｮ・ｱ繝ｻ・｢髯ｷ・ｿ繝ｻ・ｳ髣包ｽｳ驗呻ｽｫ郢晢ｽｻFPS鬮ｯ・ｦ繝ｻ・ｨ鬩穂ｼ夲ｽｽ・ｺ ---
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
        color: 0xFF_00FF00, // 鬩搾ｽｱ鬯俶ｪ取ｨｪ
    });

    // --- 鬨ｾ蛹・ｽｽ・ｻ鬯ｮ・ｱ繝ｻ・｢髣包ｽｳ驕擾ｽｩ・主､ゑｽｸ・ｺ繝ｻ・ｮ髯ｷ闌ｨ・ｽ・ｱ鬯ｨ・ｾ陜繝ｻ ---
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
        (Script::Japanese, "Japanese"),
        (Script::TraditionalChinese, "Traditional Chinese"),
        (Script::SimplifiedChinese, "Simplified Chinese"),
    ];

    for (i, (font_choice, name)) in fonts.iter().enumerate() {
        let is_selected = i == snapshot.selected_settings_item.index();
        let is_active = *font_choice == snapshot.settings_script;

        let mut display_text = if is_selected {
            format!("> {}", name)
        } else {
            format!("  {}", name)
        };

        if is_active {
            display_text.push_str(" *");
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
        // 驛｢・ｧ繝ｻ・ｽ驛｢譎｢・ｽ・ｼ驛｢・ｧ繝ｻ・ｹ鬩墓ｩｸ・ｽ・ｮ髯具ｽｻ繝ｻ・･驛｢譎√・郢晢ｽ｣驛｢・ｧ繝ｻ・ｸ驛｢・ｧ陷代・・ｽ・ｻ陋溘・・ｽ・ｸ郢晢ｽｻ [B]=builtin, [W]=web(wasm), [F]=file(desktop), [+]=open-file
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
            text: "髫ｨ繝ｻ・ｽ・ｲ".to_string(),
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
            text: "髫ｨ繝ｻ・ｽ・ｼ".to_string(),
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

/// 髯懶｣ｰ陜楢ｶ｣・ｽ・｡陟募ｾ湖ｨ驛｢・ｧ繝ｻ・｡驛｢・ｧ繝ｻ・､驛｢譎｢・ｽ・ｫ驍ｵ・ｺ繝ｻ・ｮ驛｢・ｧ繝ｻ・ｽ驛｢譎｢・ｽ・ｼ驛｢・ｧ繝ｻ・ｹ驛｢・ｧ繝ｻ・ｳ驛｢譎｢・ｽ・ｼ驛｢譎擾ｽｳ・ｨ繝ｻ蟶晢ｽｫ・｢繝ｻ・ｲ鬮ｫ蛹・ｽｽ・ｧ驍ｵ・ｺ陷ｷ・ｶ繝ｻ迢暦ｽｹ・ｧ繝ｻ・ｷ驛｢譎｢・ｽ・ｼ驛｢譎｢・ｽ・ｳ驛｢・ｧ陷ｻ閧ｲ・ｷ蟶敖蛹・ｽｽ・ｻ驍ｵ・ｺ陷ｷ・ｶ繝ｻ繝ｻ
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

    // ProblemSource 驍ｵ・ｺ繝ｻ・ｨ髯ｷ・ｷ陟募具ｽｧ驛｢譎｢・ｽ・ｬ驛｢・ｧ繝ｻ・､驛｢・ｧ繝ｻ・｢驛｢・ｧ繝ｻ・ｦ驛｢譏懶ｽｺ・･繝ｻ・ｮ陞｢・ｽ霎溷､ゑｽｹ・ｧ陷代・・ｽ・ｽ繝ｻ・ｿ鬨ｾ蛹・ｽｽ・ｨ
    let line_h: f32 = 0.046;
    let content_y: f32 = 0.21;
    // 髣包ｽｳ驕擾ｽｩ・主､ゑｽｸ・ｺ繝ｻ・ｮ status_text / instructions_text 鬯ｯ繝ｻ・ｼ諛茨ｽｲ・ｺ (0.12) 驛｢・ｧ陝ｶ譎乗ｱるし・ｺ郢晢ｽｻ隨ｳ繝ｻ・ｭ蟠｢ﾂ髯樊ｻゑｽｽ・ｧ鬮ｯ・ｦ繝ｻ・ｨ鬩穂ｼ夲ｽｽ・ｺ鬮ｯ・ｦ隴ｴ・ｧ霎溘・
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

    // 驛｢・ｧ繝ｻ・ｹ驛｢・ｧ繝ｻ・ｯ驛｢譎｢・ｽ・ｭ驛｢譎｢・ｽ・ｼ驛｢譎｢・ｽ・ｫ髣厄ｽｴ陷･・ｲ繝ｻ・ｽ繝ｻ・ｮ驛｢・ｧ繝ｻ・､驛｢譎｢・ｽ・ｳ驛｢・ｧ繝ｻ・ｸ驛｢・ｧ繝ｻ・ｱ驛｢譎｢・ｽ・ｼ驛｢・ｧ繝ｻ・ｿ (髯ｷ・ｿ繝ｻ・ｳ髣包ｽｳ郢晢ｽｻ
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

    // 髣包ｽｳ驗呻ｽｫ邵ｺ蟶ｷ・ｹ・ｧ繝ｻ・ｯ驛｢譎｢・ｽ・ｭ驛｢譎｢・ｽ・ｼ驛｢譎｢・ｽ・ｫ髯ｷ・ｿ繝ｻ・ｯ鬮｢・ｭ繝ｻ・ｽ驍ｵ・ｺ繝ｻ・ｪ髯懶ｽ｣繝ｻ・ｴ髯ｷ・ｷ郢晢ｽｻ髫ｨ繝ｻ・ｽ・ｲ 驛｢・ｧ陞ｳ螟ｲ・ｽ・｡繝ｻ・ｨ鬩穂ｼ夲ｽｽ・ｺ
    if scroll > 0 {
        render_list.push(Renderable::Text {
            text: "髫ｨ繝ｻ・ｽ・ｲ".to_string(),
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

    // 髣包ｽｳ闕ｵ譏ｴ笳矩Δ・ｧ繝ｻ・ｯ驛｢譎｢・ｽ・ｭ驛｢譎｢・ｽ・ｼ驛｢譎｢・ｽ・ｫ髯ｷ・ｿ繝ｻ・ｯ鬮｢・ｭ繝ｻ・ｽ驍ｵ・ｺ繝ｻ・ｪ髯懶ｽ｣繝ｻ・ｴ髯ｷ・ｷ郢晢ｽｻ髫ｨ繝ｻ・ｽ・ｼ 驛｢・ｧ陞ｳ螟ｲ・ｽ・｡繝ｻ・ｨ鬩穂ｼ夲ｽｽ・ｺ
    if scroll + max_lines < total_lines {
        render_list.push(Renderable::Text {
            text: "髫ｨ繝ｻ・ｽ・ｼ".to_string(),
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

// Segment 驍ｵ・ｺ繝ｻ・ｮ base 驛｢譏ｴ繝ｻ邵ｺ蜀暦ｽｹ・ｧ繝ｻ・ｹ驛｢譎冗樟繝ｻ螳夲ｽｭ竏壹・繝ｻ・ｭ隲､諛翫・驍ｵ・ｺ繝ｻ・ｨ驍ｵ・ｺ陷会ｽｱ遯ｶ・ｻ鬮ｴ隨ｬ魍堤ｬ倥・繝ｻ郢晢ｽｻnno 驍ｵ・ｺ繝ｻ・ｯ inner 驍ｵ・ｺ繝ｻ・ｮ base 驛｢・ｧ陝ｶ謨鳴繝ｻ・｣鬩搾ｽｨ隰ｦ・ｰ繝ｻ・ｼ郢晢ｽｻ
fn segment_base_text(seg: &Segment) -> String {
    match seg {
        Segment::Plain { text } => text.clone(),
        Segment::Annotated { base, .. } => base.clone(),
        Segment::Anno { inner, .. } => inner.iter().map(segment_base_text).collect(),
    }
}

// Segment 驍ｵ・ｺ繝ｻ・ｮ reading 驛｢譏ｴ繝ｻ邵ｺ蜀暦ｽｹ・ｧ繝ｻ・ｹ驛｢譎冗樟繝ｻ螳夲ｽｭ竏壹・繝ｻ・ｭ隲､諛翫・驍ｵ・ｺ繝ｻ・ｨ驍ｵ・ｺ陷会ｽｱ遯ｶ・ｻ鬮ｴ隨ｬ魍堤ｬ倥・繝ｻ郢晢ｽｻnno 驍ｵ・ｺ繝ｻ・ｯ inner 驍ｵ・ｺ繝ｻ・ｮ reading 驛｢・ｧ陝ｶ謨鳴繝ｻ・｣鬩搾ｽｨ隰ｦ・ｰ繝ｻ・ｼ郢晢ｽｻ
fn segment_reading_text(seg: &Segment) -> String {
    match seg {
        Segment::Plain { text } => text.clone(),
        Segment::Annotated { reading, .. } => reading.clone(),
        Segment::Anno { inner, .. } => inner.iter().map(segment_reading_text).collect(),
    }
}

// 鬮ｯ・ｦ繝ｻ・ｨ鬩穂ｼ夲ｽｽ・ｺ鬨ｾ蛹・ｽｽ・ｨ驍ｵ・ｺ繝ｻ・ｮ (base_text, ruby_text, anno_text) 驛｢・ｧ陞ｳ螟ｲ・ｽ・ｿ隴∫ｵｶ繝ｻ
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

fn measure_line_base_width(line: &crate::model::Line, font: &FontVec, font_size: f32) -> u32 {
    line.words
        .iter()
        .flat_map(|word| &word.segments)
        .map(|segment| {
            let text = segment_base_text(segment);
            gui_renderer::measure_text(font, &text, font_size).0
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

fn build_typing_ui(
    app: &App,
    render_list: &mut Vec<Renderable>,
    gradient: Gradient,
    font: &FontVec,
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
        let base_pixel_font_size = calculate_pixel_font_size(base_font_size, width, height);
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
            None => measure_line_base_width(content_line, font, base_pixel_font_size),
        };
        let scroll_offset = if cached_cache.is_some() {
            (model.scroll.scroll - line_origin) as f32
        } else {
            typing_line_scroll_offset(full_line_width as f32, 0.0, width)
        };
        let line_shift_x = typing_line_shift_x(scroll_offset, width);
        // --- 髣包ｽｳ鬯・汚・ｽ・ｮ繝ｻ・ｵ郢晢ｽｻ髢ｧ・ｲ陝ｯ・ｼ髫ｶ轣倡函郢晢ｽｦ驛｢・ｧ繝ｻ・ｭ驛｢・ｧ繝ｻ・ｹ驛｢譎∬か繝ｻ・ｼ陝ｲ・ｨ郢晢ｽｻ髫ｶ蝣､霍昴・・ｯ郢晢ｽｻ---
        let mut upper_segments = Vec::new();
        for (word_idx, word) in content_line.words.iter().enumerate() {
            for (seg_idx, seg) in word.segments.iter().enumerate() {
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
            line_width: full_line_width,
        });

        // --- 髣包ｽｳ陋ｹ・ｺ繝ｻ・ｮ繝ｻ・ｵ郢晢ｽｻ闔・･郢晢ｽｻ髯ｷ迚呻ｽｸ蜷ｶﾎ倬Δ・ｧ繝ｻ・ｭ驛｢・ｧ繝ｻ・ｹ驛｢譎∬か繝ｻ・ｼ陝ｲ・ｨ郢晢ｽｻ髫ｶ蝣､霍昴・・ｯ郢晢ｽｻ---
        let mut lower_segments = Vec::new();
        let mut lower_visible_start_width = 0;
        let status_word = status.word.get();
        let status_segment = status.segment.get();

        if let Some(state) = cached_cache {
            let current_cache = &state.current;
            let status_word_idx = status_word;
            let (left_bound, right_bound) = typing_visible_line_bounds(scroll_offset, width);

            let active_segment_idx = if status_word < current_cache.word_segment_starts.len() {
                let word_start = current_cache
                    .word_segment_starts
                    .get(status_word)
                    .copied()
                    .unwrap_or(current_cache.segments.len());
                let word_end = current_cache
                    .word_segment_starts
                    .get(status_word + 1)
                    .copied()
                    .unwrap_or(current_cache.segments.len());
                let segment_count = word_end.saturating_sub(word_start);
                let status_offset = status_segment.min(segment_count);
                word_start + status_offset
            } else {
                current_cache.segments.len()
            };

            let mut visible_start = current_cache
                .segment_prefix_width
                .partition_point(|value| *value <= left_bound)
                .saturating_sub(1)
                .min(current_cache.segments.len());
            let mut visible_end = current_cache
                .segment_prefix_width
                .partition_point(|value| *value <= right_bound);
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
                        let (base_text, ruby_text, _) = segment_display_parts(seg);
                        let seg_width =
                            gui_renderer::measure_text(font, &base_text, base_pixel_font_size).0;
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
                content_line.words.get(status_word),
                correctness_line.words.get(status_word),
            ) {
                for seg_idx in 0..status_segment {
                    if let Some(seg) = active_word_content.segments.get(seg_idx) {
                        let (base_text, ruby_text, _) = segment_display_parts(seg);
                        let is_correct = active_correctness_word
                            .segments
                            .get(seg_idx)
                            .is_some_and(is_segment_correct);
                        let width =
                            gui_renderer::measure_text(font, &base_text, base_pixel_font_size).0;
                        lower_segments.push(LowerTypingSegment::Completed {
                            base_text,
                            ruby_text,
                            is_correct,
                            width,
                        });
                    }
                }

                if let Some(active_seg_content) = active_word_content.segments.get(status_segment) {
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

        // --- 驛｢・ｧ繝ｻ・ｳ驛｢譎｢・ｽ・ｳ驛｢譏ｴ繝ｻ邵ｺ蜀暦ｽｹ・ｧ繝ｻ・ｹ驛｢譎槭Γ繝ｻ・｡鬲・ｼ夲ｽｽ・ｼ闔・･霎ｯ謌奇｣ｰ蜍滂ｽｾ蠕後・鬮ｯ・ｦ鬲・ｼ夲ｽｽ・ｼ陝ｲ・ｨ繝ｻ螳夲ｽｬ・ｰ陷諤懈・ ---
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

        // --- 驛｢・ｧ繝ｻ・ｹ驛｢譏ｴ繝ｻ郢晢ｽｻ驛｢・ｧ繝ｻ・ｿ驛｢・ｧ繝ｻ・ｹ驛｢譏懶ｽｻ・｣郢晢ｽｭ驛｢譎｢・ｽ・ｫ ---
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

        // --- 鬯ｨ・ｾ繝ｻ・ｲ髫ｰ莉吝ｹｲ郢晢ｽｰ驛｢譎｢・ｽ・ｼ ---
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

/// Anchor驍ｵ・ｺ繝ｻ・ｨShift驍ｵ・ｺ闕ｵ譎｢・ｽ閾･・ｸ・ｲ遶乗刋・ｸ謌奇ｽｲ繝ｻ縺倡ｫ雁､・ｸ・ｺ繝ｻ・ｪ驛｢・ｧ陷ｿ・･繝ｻ・ｺ繝ｻ・ｧ髫ｶ阮吶・x, y)驛｢・ｧ陞ｳ螟ｲ・ｽ・ｨ髢ｧ・ｲ繝ｻ・ｮ陷会ｽｱ隨倥・・ｹ・ｧ郢晢ｽｻ
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

/// 髯憺屮・ｽ・ｺ髮九・・ｹ貅倪雷驍ｵ・ｲ遶丞｣ｹﾎ倬Δ・ｧ繝ｻ・ｭ驛｢・ｧ繝ｻ・ｹ驛｢譎冗樟郢晢ｽｻ髯昴・・ｽ・ｸ髮取・・ｼ譚ｿﾂ遶擾ｽｵ驍冗坩・ｸ・ｺ陜捺ｺｷ・ｩ・ｿ驍ｵ・ｺ闕ｵ譎｢・ｽ閾･・ｸ・ｲ遶擾ｽｵ隲､蜥弱♀郢ｧ迚咎｣ｭ驍ｵ・ｺ繝ｻ・ｪ髫ｰ・ｰ陷諤懈・鬯ｮ・｢陷ｿ・･繝ｻ・ｧ陷ｿ・･繝ｻ・ｺ繝ｻ・ｧ髫ｶ轣倬●繝ｻ・ｼ闔・･繝ｻ・ｷ繝ｻ・ｦ髣包ｽｳ陞ゅ・・ｽ・ｼ陝ｲ・ｨ繝ｻ蟶晏搦髢ｧ・ｲ繝ｻ・ｮ陷会ｽｱ隨倥・・ｹ・ｧ郢晢ｽｻ
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

    fn test_fonts() -> Fonts {
        let japanese =
            FontVec::try_from_vec(include_bytes!("../fonts/YujiSyuku-Regular.ttf").to_vec())
                .expect("test font should parse");
        Fonts {
            japanese,
            traditional_chinese: None,
            simplified_chinese: None,
        }
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

    fn typing_alignment(render_list: &[Renderable]) -> (u32, TypingLineAlignment) {
        let upper_width = render_list
            .iter()
            .find_map(|item| match item {
                Renderable::TypingUpper { line_width, .. } => Some(*line_width),
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
        (upper_width, lower_alignment)
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

        let render_list = build_ui(&app, app.get_current_font(), 800, 500);
        let (upper_segments, lower_segments) = typing_rows(&render_list);
        assert_eq!(upper_segments[0].base_text, "色");
        assert_eq!(upper_segments[0].state, UpperSegmentState::Correct);
        assert_eq!(upper_segments[1].base_text, "は");
        assert_eq!(upper_segments[1].state, UpperSegmentState::Correct);
        assert_eq!(upper_segments[2].base_text, "句");
        assert_eq!(upper_segments[2].state, UpperSegmentState::Active);

        let LowerTypingSegment::Active { elements } = &lower_segments[0] else {
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

        let (upper_width, lower_alignment) = typing_alignment(&render_list);
        assert_eq!(lower_alignment, TypingLineAlignment::full_line(upper_width));
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

        let render_list = build_ui(&app, app.get_current_font(), 240, 500);
        let (upper_width, lower_alignment) = typing_alignment(&render_list);
        let (_, lower_segments) = typing_rows(&render_list);
        let Some(ScrollCache::Ready(cache)) = app.scroll_cache() else {
            panic!("scroll cache should be ready");
        };

        assert_eq!(upper_width, cache.current.total_width as u32);
        assert_eq!(lower_alignment.full_line_width, upper_width);
        assert!(
            lower_alignment.visible_start_width > 0,
            "long cached rows should preserve a non-zero clipped prefix"
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

        let render_list = build_ui(&app, app.get_current_font(), 240, 500);
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
                    line_width,
                    ..
                } => {
                    assert_line_left_matches_scroll_offset(
                        *anchor,
                        *shift,
                        *align,
                        *line_width,
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
