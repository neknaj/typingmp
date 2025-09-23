// src/typing_renderer.rs

#[cfg(feature = "uefi")]
extern crate alloc;

#[cfg(feature = "uefi")]
use alloc::{string::{String, ToString}, vec::Vec};
#[cfg(not(feature = "uefi"))]
use std::string::{String, ToString};
#[cfg(not(feature = "uefi"))]
use std::vec::Vec;

use crate::model::{TypingModel, Segment, TypingCorrectnessChar, TypingCorrectnessSegment};
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
pub const BASE_FONT_SIZE_RATIO: f32 = 0.125;
const UPPER_ROW_Y_OFFSET_FACTOR: f32 = 1.2;
const LOWER_ROW_Y_OFFSET_FACTOR: f32 = 0.5;
const RUBY_Y_OFFSET_FACTOR: f32 = 1.0;

/// Helper to check if all characters in a segment were typed correctly.
fn is_segment_correct(segment: &TypingCorrectnessSegment) -> bool {
    !segment.chars.iter().any(|c| *c == TypingCorrectnessChar::Incorrect)
}

/// Builds the complex list of renderables for the main typing view.
pub fn build_typing_renderables(
    model: &TypingModel,
    font: &FontRef,
    width: usize,
    height: usize,
) -> Vec<Renderable> {
    let mut renderables = Vec::new();

    let line_idx = model.status.line as usize;
    let content_line = if let Some(line) = model.content.lines.get(line_idx) { line } else { return renderables; };
    let correctness_line = if let Some(line) = model.typing_correctness.lines.get(line_idx) { line } else { return renderables; };
    let status = &model.status;
    let scroll_offset = model.scroll.scroll as f32;

    let base_font_size = FontSize::WindowHeight(BASE_FONT_SIZE_RATIO);
    let base_pixel_font_size = crate::renderer::calculate_pixel_font_size(base_font_size, width, height);
    let ruby_pixel_font_size = base_pixel_font_size * 0.4;
    let small_ruby_pixel_font_size = base_pixel_font_size * 0.3;

    // --- Layout Calculation --- // The entire layout is now driven by the width of the 'base' text.
    let total_layout_width = content_line.segments.iter().map(|seg| {
        let text = match seg {
            Segment::Plain { text } => text.as_str(),
            Segment::Annotated { base, .. } => base.as_str(),
        };
        gui_renderer::measure_text(font, text, base_pixel_font_size).0 as f32
    }).sum::<f32>();

    // This is the starting X for the entire line block, for both rows.
    let block_start_x = (width as f32 - total_layout_width) / 2.0 - scroll_offset;

    // --- Render Upper Row (Target Text) ---
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
        
        // Render the base text at the current pen position to ensure left alignment.
        renderables.push(Renderable::BigText { text: base_text.to_string(), anchor: Anchor::TopLeft, shift: Shift {x: upper_pen_x / width as f32, y: upper_y / height as f32}, align: Align {horizontal: HorizontalAlign::Left, vertical: VerticalAlign::Top}, font_size: base_font_size, color });
        
        // Also render ruby text if it's annotated
        if let Segment::Annotated { base, reading, .. } = seg {
            let (base_w, ..) = gui_renderer::measure_text(font, base, base_pixel_font_size);
            let (ruby_w, ..) = gui_renderer::measure_text(font, reading, ruby_pixel_font_size);
            // Center the ruby above the base text we just drew.
            let ruby_x = upper_pen_x + (base_w as f32 - ruby_w as f32) / 2.0;
            let ruby_y = upper_y - ruby_pixel_font_size * RUBY_Y_OFFSET_FACTOR;
            renderables.push(Renderable::Text {
                text: reading.clone(),
                anchor: Anchor::TopLeft,
                shift: Shift {x: ruby_x / width as f32, y: ruby_y / height as f32},
                align: Align {horizontal: HorizontalAlign::Left, vertical: VerticalAlign::Top},
                font_size: FontSize::WindowHeight(BASE_FONT_SIZE_RATIO * 0.4),
                color
            });
        }

        // IMPORTANT: Advance the pen by the BASE width.
        let (base_w, ..) = gui_renderer::measure_text(font, base_text, base_pixel_font_size);
        upper_pen_x += base_w as f32;
    }

    // --- Render Lower Row (User Input) ---
    let lower_y = (height as f32 / 2.0) + base_pixel_font_size * LOWER_ROW_Y_OFFSET_FACTOR;
    let mut lower_pen_x = block_start_x;
    
    // Draw completed segments
    for seg_idx in 0..(status.segment as usize) {
        let seg = &content_line.segments[seg_idx];
        let color = if is_segment_correct(&correctness_line.segments[seg_idx]) { CORRECT_COLOR } else { INCORRECT_COLOR };
        
        let base_text = match seg {
            Segment::Plain { text } => text,
            Segment::Annotated { base, .. } => base,
        };

        // Render the base text at the current pen position.
        renderables.push(Renderable::BigText { text: base_text.to_string(), anchor:Anchor::TopLeft, shift: Shift { x: lower_pen_x / width as f32, y: lower_y / height as f32 }, align: Align {horizontal: HorizontalAlign::Left, vertical: VerticalAlign::Top}, font_size: base_font_size, color });
        
        if let Segment::Annotated { base, reading, .. } = seg {
            let (base_w, ..) = gui_renderer::measure_text(font, base, base_pixel_font_size);
            let (small_reading_w, ..) = gui_renderer::measure_text(font, reading, small_ruby_pixel_font_size);
            // Center the ruby above the base text we just drew.
            let ruby_x = lower_pen_x + (base_w as f32 - small_reading_w as f32) / 2.0;
            let ruby_y = lower_y - small_ruby_pixel_font_size * RUBY_Y_OFFSET_FACTOR;
            renderables.push(Renderable::Text {
                text: reading.clone(),
                anchor: Anchor::TopLeft,
                shift: Shift { x: ruby_x / width as f32, y: ruby_y / height as f32 },
                align: Align {horizontal: HorizontalAlign::Left, vertical: VerticalAlign::Top},
                font_size: FontSize::WindowHeight(BASE_FONT_SIZE_RATIO * 0.3),
                color
            });
        }
        
        // IMPORTANT: Advance the pen by the BASE width.
        let (base_w, ..) = gui_renderer::measure_text(font, base_text, base_pixel_font_size);
        lower_pen_x += base_w as f32;
    }


    // Draw current segment (typed part)
    if let Some(seg) = content_line.segments.get(status.segment as usize) {
        let reading_text = match seg {
            Segment::Plain { text } => text.as_str(),
            Segment::Annotated { base: _, reading } => reading.as_str(),
        };

        let mut reading_width_before: u32 = 0;

        for (char_idx, character) in reading_text.chars().enumerate().take(status.char_ as usize) {
            let char_str = character.to_string();
            let color = match correctness_line.segments[status.segment as usize].chars[char_idx] {
                TypingCorrectnessChar::Correct => CORRECT_COLOR,
                _ => INCORRECT_COLOR,
            };

            // Render the typed character (from reading text) at the current pen position.
            renderables.push(Renderable::BigText {
                text: char_str.clone(),
                anchor: Anchor::TopLeft,
                shift: Shift { x: lower_pen_x / width as f32, y: lower_y / height as f32 },
                align: Align { horizontal: HorizontalAlign::Left, vertical: VerticalAlign::Top },
                font_size: base_font_size,
                color,
            });

            // Calculate the actual advance width for this character, including kerning,
            // by measuring the difference in substring widths.
            let reading_part_up_to_char = reading_text.chars().take(char_idx + 1).collect::<String>();
            let (reading_width_up_to_char, ..) = gui_renderer::measure_text(font, &reading_part_up_to_char, base_pixel_font_size);
            let char_advance_width = (reading_width_up_to_char - reading_width_before) as f32;

            // DELETED: The redundant drawing of a smaller ruby on top of the large, unconfirmed ruby text has been removed.

            // Advance the pen by the calculated width of the character we just drew.
            lower_pen_x += char_advance_width;
            reading_width_before = reading_width_up_to_char;
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