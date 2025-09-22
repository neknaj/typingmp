// uefi featureが有効な場合、標準のallocクレートをインポート
#[cfg(feature = "uefi")]
extern crate alloc;

// uefi と std で使用する Vec と vec! を切り替える
#[cfg(feature = "uefi")]
use alloc::vec::Vec;
#[cfg(feature = "uefi")]
use alloc::vec;
#[cfg(not(feature = "uefi"))]
use std::vec::Vec;

#[cfg(feature = "uefi")]
use core_maths::CoreFloat;

use ab_glyph::{point, Font, FontRef, OutlinedGlyph, PxScale, ScaleFont};
use crate::ui::FontSize;

/// テキストの描画色
pub const TEXT_COLOR: u32 = 0x00_FFFFFF;
/// 背景の描画色
pub const BG_COLOR: u32 = 0x00_101010;

/// Calculates the actual pixel font size based on the FontSize enum and window dimensions.
pub fn calculate_pixel_font_size(font_size: FontSize, width: usize, height: usize) -> f32 {
    match font_size {
        FontSize::WindowHeight(ratio) => height as f32 * ratio,
        FontSize::WindowAreaSqrt(ratio) => {
            let area = (width * height) as f32;
            area.sqrt() * ratio
        }
    }
}

/// GUI/WASMバックエンド用のピクセルベースレンダラ
pub mod gui_renderer {
    use super::*;

    /// 指定されたピクセルバッファの指定位置にテキストを描画する
    pub fn draw_text(
        buffer: &mut [u32], stride: usize, font: &FontRef, text: &str,
        pos: (f32, f32), font_size: f32,
    ) {
        let scale = PxScale::from(font_size);
        let scaled_font = font.as_scaled(scale);
        let ascent = scaled_font.ascent();
        let mut pen_x = pos.0;
        let pen_y = pos.1 + ascent;

        let mut last_glyph = None;
        for character in text.chars() {
            let glyph_id = font.glyph_id(character);
            if let Some(last) = last_glyph {
                pen_x += scaled_font.kern(last, glyph_id);
            }
            let glyph = glyph_id.with_scale_and_position(scale, point(pen_x, pen_y));
            if let Some(outlined) = font.outline_glyph(glyph) {
                draw_glyph_to_pixel_buffer(buffer, stride, &outlined);
            }
            pen_x += scaled_font.h_advance(glyph_id);
            last_glyph = Some(glyph_id);
        }
    }
    
    /// アウトライン化されたグリフをピクセルバッファに描画する（内部関数）
    fn draw_glyph_to_pixel_buffer(buffer: &mut [u32], stride: usize, outlined: &OutlinedGlyph) {
        let bounds = outlined.px_bounds();
        outlined.draw(|x, y, c| {
            let buffer_x = bounds.min.x as i32 + x as i32;
            let buffer_y = bounds.min.y as i32 + y as i32;
            let height = buffer.len() / stride;
            if buffer_x >= 0 && buffer_x < stride as i32 && buffer_y >= 0 && buffer_y < height as i32 {
                let index = (buffer_y as usize) * stride + (buffer_x as usize);
                let text_r = ((TEXT_COLOR >> 16) & 0xFF) as f32;
                let text_g = ((TEXT_COLOR >> 8) & 0xFF) as f32;
                let text_b = (TEXT_COLOR & 0xFF) as f32;
                let bg_r = ((buffer[index] >> 16) & 0xFF) as f32;
                let bg_g = ((buffer[index] >> 8) & 0xFF) as f32;
                let bg_b = (buffer[index] & 0xFF) as f32;
                let r = (text_r * c + bg_r * (1.0 - c)) as u32;
                let g = (text_g * c + bg_g * (1.0 - c)) as u32;
                let b = (text_b * c + bg_b * (1.0 - c)) as u32;
                buffer[index] = (r << 16) | (g << 8) | b;
            }
        });
    }

    /// テキストの描画サイズ（幅と高さ）を計算する
    pub fn measure_text(font: &FontRef, text: &str, size: f32) -> (u32, u32, f32) {
        let scale = PxScale::from(size);
        let scaled_font = font.as_scaled(scale);
        let mut total_width = 0.0;

        let mut last_glyph_id = None;
        for c in text.chars() {
            if c == '\n' {
                continue;
            }
            let glyph = font.glyph_id(c);
            if let Some(last_id) = last_glyph_id {
                total_width += scaled_font.kern(last_id, glyph);
            }
            total_width += scaled_font.h_advance(glyph);
            last_glyph_id = Some(glyph);
        }
        let height = scaled_font.ascent() - scaled_font.descent();
        (total_width as u32, height as u32, scaled_font.ascent())
    }
}

/// TUIバックエンド用の文字ベースレンダラ
pub mod tui_renderer {
    use super::*;
    
    /// 全画面に大きなテキストを描画し、文字バッファを返す
    pub fn render(font: &FontRef, text: &str, width: usize, height: usize, ui_font_size: FontSize) -> Vec<char> {
        let mut buffer = vec![' '; width * height];
        let font_size = super::calculate_pixel_font_size(ui_font_size, width, height);
        let scale = font_size / (font.ascent_unscaled() - font.descent_unscaled());
        let mut pen_x = 2.0;
        let pen_y = height as f32 * 0.7;

        for character in text.chars() {
            let glyph = font.glyph_id(character).with_scale_and_position(font_size, point(pen_x, pen_y));
            if let Some(outlined) = font.outline_glyph(glyph) {
                draw_glyph_to_char_buffer(&mut buffer, width, &outlined);
            }
            pen_x += font.h_advance_unscaled(font.glyph_id(character)) * scale;
        }
        buffer
    }
    
    /// アウトライン化されたグリフを文字バッファに描画する（内部関数）
    fn draw_glyph_to_char_buffer(buffer: &mut Vec<char>, width: usize, outlined: &OutlinedGlyph) {
        let bounds = outlined.px_bounds();
        outlined.draw(|x, y, c| {
            let buffer_x = bounds.min.x as usize + x as usize;
            let buffer_y = bounds.min.y as usize + y as usize;
            let height = buffer.len() / width;
            if buffer_x < width && buffer_y < height {
                let index = buffer_y * width + buffer_x;
                let coverage_char = match (c * 4.0).round() as u8 {
                    0 => ' ',
                    1 => '.',
                    2 => '*',
                    3 => '#',
                    _ => '@',
                };
                if buffer[index] == ' ' { buffer[index] = coverage_char; }
            }
        });
    }

    /// テキストの描画サイズ（幅と高さ）を計算する
    pub fn measure_text(text: &str) -> (u32, u32) {
        (text.chars().count() as u32, 1)
    }
}