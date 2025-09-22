// src/typing_renderer.rs

#[cfg(feature = "uefi")]
extern crate alloc;

#[cfg(feature = "uefi")]
use alloc::{string::{String, ToString}, vec::Vec};
#[cfg(not(feature = "uefi"))]
use std::string::{String, ToString};
#[cfg(not(feature = "uefi"))]
use std::vec::Vec;

use crate::model::{Line, Segment, TypingCorrectnessChar, TypingCorrectnessLine, TypingCorrectnessSegment, TypingStatus};
use crate::renderer::gui_renderer;
use crate::ui::{Align, Anchor, FontSize, HorizontalAlign, Renderable, Shift, VerticalAlign};
use ab_glyph::FontRef;

// --- Define Colors based on old source ---
const CORRECT_COLOR: u32 = 0xFF_9097FF;
const INCORRECT_COLOR: u32 = 0xFF_FF9898;
const PENDING_COLOR: u32 = 0xFF_999999;
const WRONG_KEY_COLOR: u32 = 0xFF_F55252;
const CURSOR_COLOR: u32 = 0xFF_FFFFFF;

// --- Layout Control Constants ---
const UPPER_ROW_Y_OFFSET_FACTOR: f32 = 1.2;
const LOWER_ROW_Y_OFFSET_FACTOR: f32 = 0.5;
const RUBY_Y_OFFSET_FACTOR: f32 = 1.0;

/// Helper to check if all characters in a segment were typed correctly.
fn is_segment_correct(segment: &TypingCorrectnessSegment) -> bool {
    !segment.chars.iter().any(|c| *c == TypingCorrectnessChar::Incorrect)
}

/// Builds the complex list of renderables for the main typing view.
pub fn build_typing_renderables(
    content_line: &Line,
    correctness_line: &TypingCorrectnessLine,
    status: &TypingStatus,
    font: &FontRef,
    width: usize,
    height: usize,
) -> Vec<Renderable> {
    let mut renderables = Vec::new();

    let base_font_size = FontSize::WindowHeight(0.125);
    let base_pixel_font_size = crate::renderer::calculate_pixel_font_size(base_font_size, width, height);
    let ruby_pixel_font_size = base_pixel_font_size * 0.4;
    let small_ruby_pixel_font_size = base_pixel_font_size * 0.3;

    // --- UPPER ROW (TARGET TEXT) ---
    let upper_y = (height as f32 / 2.0) - base_pixel_font_size * UPPER_ROW_Y_OFFSET_FACTOR;
    let total_upper_width = content_line.segments.iter().map(|seg| {
        let text = match seg { Segment::Plain { text } => text, Segment::Annotated { base, .. } => base };
        gui_renderer::measure_text(font, text, base_pixel_font_size).0 as f32
    }).sum::<f32>();
    let mut upper_pen_x = (width as f32 - total_upper_width) / 2.0;

    for (seg_idx, seg) in content_line.segments.iter().enumerate() {
        let is_typed_segment = seg_idx < status.segment as usize;
        let color = if is_typed_segment {
            if is_segment_correct(&correctness_line.segments[seg_idx]) { CORRECT_COLOR } else { INCORRECT_COLOR }
        } else {
            PENDING_COLOR
        };
        
        match seg {
            Segment::Plain { text } => {
                let (seg_w, ..) = gui_renderer::measure_text(font, text, base_pixel_font_size);
                renderables.push(Renderable::BigText { text: text.clone(), anchor: Anchor::TopLeft, shift: Shift {x: upper_pen_x / width as f32, y: upper_y / height as f32}, align: Align {horizontal: HorizontalAlign::Left, vertical: VerticalAlign::Top}, font_size: base_font_size, color });
                upper_pen_x += seg_w as f32;
            }
            Segment::Annotated { base, reading } => {
                 let (base_w, ..) = gui_renderer::measure_text(font, base, base_pixel_font_size);
                 let (reading_w, ..) = gui_renderer::measure_text(font, reading, ruby_pixel_font_size);
                 let ruby_x = upper_pen_x + (base_w as f32 - reading_w as f32) / 2.0;
                 let ruby_y = upper_y - ruby_pixel_font_size * RUBY_Y_OFFSET_FACTOR;

                renderables.push(Renderable::BigText { text: base.clone(), anchor: Anchor::TopLeft, shift: Shift {x: upper_pen_x / width as f32, y: upper_y / height as f32}, align: Align {horizontal: HorizontalAlign::Left, vertical: VerticalAlign::Top}, font_size: base_font_size, color });
                renderables.push(Renderable::Text { text: reading.clone(), anchor: Anchor::TopLeft, shift: Shift {x: ruby_x / width as f32, y: ruby_y / height as f32}, align: Align {horizontal: HorizontalAlign::Left, vertical: VerticalAlign::Top}, font_size: FontSize::WindowHeight(0.125 * 0.4), color });
                upper_pen_x += base_w as f32;
            }
        }
    }

    // --- LOWER ROW (USER INPUT) ---
    let lower_y = (height as f32 / 2.0) + base_pixel_font_size * LOWER_ROW_Y_OFFSET_FACTOR;
    let total_lower_width = {
        let mut temp_width = 0.0;
        for i in 0..status.segment as usize {
            let text = match &content_line.segments[i] {
                Segment::Plain { text } => text,
                Segment::Annotated { base, .. } => base,
            };
            temp_width += gui_renderer::measure_text(font, text, base_pixel_font_size).0 as f32;
        }
        if let Some(seg) = content_line.segments.get(status.segment as usize) {
            let reading = match seg { Segment::Plain { text } => text, Segment::Annotated { reading, .. } => reading };
            let typed_part = reading.chars().take(status.char_ as usize).collect::<String>();
            temp_width += gui_renderer::measure_text(font, &typed_part, base_pixel_font_size).0 as f32;
        }
        temp_width
    };
    let mut lower_pen_x = (width as f32 - total_lower_width) / 2.0;

    // Draw completed segments for lower row
    for (seg_idx, seg) in content_line.segments.iter().enumerate().take(status.segment as usize) {
        let color = if is_segment_correct(&correctness_line.segments[seg_idx]) { CORRECT_COLOR } else { INCORRECT_COLOR };
        match seg {
            Segment::Plain { text } => {
                let (seg_w, ..) = gui_renderer::measure_text(font, text, base_pixel_font_size);
                renderables.push(Renderable::BigText { text: text.clone(), anchor:Anchor::TopLeft, shift: Shift { x: lower_pen_x / width as f32, y: lower_y / height as f32 }, align: Align {horizontal: HorizontalAlign::Left, vertical: VerticalAlign::Top}, font_size: base_font_size, color });
                lower_pen_x += seg_w as f32;
            }
            Segment::Annotated { base, reading } => {
                let (base_w, ..) = gui_renderer::measure_text(font, base, base_pixel_font_size);
                let (reading_w, ..) = gui_renderer::measure_text(font, reading, small_ruby_pixel_font_size);
                let ruby_x = lower_pen_x + (base_w as f32 - reading_w as f32) / 2.0;
                let ruby_y = lower_y - small_ruby_pixel_font_size * RUBY_Y_OFFSET_FACTOR;
                
                renderables.push(Renderable::BigText { text: base.clone(), anchor:Anchor::TopLeft, shift: Shift { x: lower_pen_x / width as f32, y: lower_y / height as f32 }, align: Align {horizontal: HorizontalAlign::Left, vertical: VerticalAlign::Top}, font_size: base_font_size, color });
                renderables.push(Renderable::Text { text: reading.clone(), anchor: Anchor::TopLeft, shift: Shift { x: ruby_x / width as f32, y: ruby_y / height as f32 }, align: Align {horizontal: HorizontalAlign::Left, vertical: VerticalAlign::Top}, font_size: FontSize::WindowHeight(0.125 * 0.3), color });
                lower_pen_x += base_w as f32;
            }
        }
    }

    // Draw current segment (typed part) for lower row
    if let Some(seg) = content_line.segments.get(status.segment as usize) {
        let reading = match seg { Segment::Plain { text } => text, Segment::Annotated { reading, .. } => reading };
        for (char_idx, character) in reading.chars().enumerate().take(status.char_ as usize) {
            let char_str = character.to_string();
            let color = match correctness_line.segments[status.segment as usize].chars[char_idx] {
                TypingCorrectnessChar::Correct => CORRECT_COLOR,
                _ => INCORRECT_COLOR,
            };
            let (char_w, ..) = gui_renderer::measure_text(font, &char_str, base_pixel_font_size);
            
            if let Segment::Annotated{..} = seg {
                let (reading_w, ..) = gui_renderer::measure_text(font, &char_str, small_ruby_pixel_font_size);
                let ruby_x = lower_pen_x + (char_w as f32 - reading_w as f32) / 2.0;
                let ruby_y = lower_y - small_ruby_pixel_font_size * RUBY_Y_OFFSET_FACTOR;
                renderables.push(Renderable::Text { text: char_str.clone(), anchor: Anchor::TopLeft, shift: Shift { x: ruby_x / width as f32, y: ruby_y / height as f32 }, align: Align {horizontal: HorizontalAlign::Left, vertical: VerticalAlign::Top}, font_size: FontSize::WindowHeight(0.125 * 0.3), color });
            }
            renderables.push(Renderable::BigText { text: char_str, anchor: Anchor::TopLeft, shift: Shift { x: lower_pen_x/width as f32, y: lower_y/ height as f32}, align: Align {horizontal: HorizontalAlign::Left, vertical: VerticalAlign::Top}, font_size: base_font_size, color });
            lower_pen_x += char_w as f32;
        }
    }
    
    // --- Draw Cursor & Extras ---
    let cursor_y = lower_y;
    renderables.push(Renderable::BigText {text: "|".to_string(), anchor: Anchor::TopLeft, shift: Shift {x: lower_pen_x / width as f32, y: cursor_y / height as f32}, align: Align {horizontal: HorizontalAlign::Left, vertical: VerticalAlign::Top}, font_size: base_font_size, color: CURSOR_COLOR});
    
    let extras_x = lower_pen_x + gui_renderer::measure_text(font, "|", base_pixel_font_size).0 as f32 * 0.5;
    if !status.unconfirmed.is_empty() {
        let unconfirmed_text: String = status.unconfirmed.iter().collect();
        renderables.push(Renderable::Text {text: unconfirmed_text, anchor: Anchor::TopLeft, shift: Shift {x: extras_x / width as f32, y: lower_y / height as f32}, align: Align {horizontal: HorizontalAlign::Left, vertical: VerticalAlign::Top}, font_size: base_font_size, color: PENDING_COLOR});
    } else if let Some(wrong_char) = status.last_wrong_keydown {
        let wrong_text = wrong_char.to_string();
        renderables.push(Renderable::Text {text: wrong_text, anchor: Anchor::TopLeft, shift: Shift {x: extras_x / width as f32, y: lower_y / height as f32}, align: Align {horizontal: HorizontalAlign::Left, vertical: VerticalAlign::Top}, font_size: base_font_size, color: WRONG_KEY_COLOR});
    }

    renderables
}