// -----------------------------------------------------------------------------
// モジュール：renderer - ab_glyphを使った共通レンダリングロジック
// -----------------------------------------------------------------------------
use ab_glyph::{point, Font, FontRef, OutlinedGlyph};

/// GUIバックエンドおよびWASMバックエンド用のレンダラ
pub mod gui_renderer {
    use super::*;
    const TEXT_COLOR: u32 = 0x00_FFFFFF;
    const BG_COLOR: u32 = 0x00_101010;
    
    pub fn render(font: &FontRef, text: &str, width: usize, height: usize) -> Vec<u32> {
        let mut buffer = vec![BG_COLOR; width * height];

        let font_size = 48.0;
        let scale = font_size / (font.ascent_unscaled() - font.descent_unscaled());
        let mut pen_x = 15.0;
        let pen_y = (height as f32) / 2.0 + font_size / 3.0; // 中央に寄せる

        for character in text.chars() {
            let glyph = font.glyph_id(character).with_scale_and_position(font_size, point(pen_x, pen_y));
            if let Some(outlined) = font.outline_glyph(glyph) {
                draw_glyph_to_pixel_buffer(&mut buffer, width, &outlined);
            }
            pen_x += font.h_advance_unscaled(font.glyph_id(character)) * scale;
        }
        buffer
    }
    
    fn draw_glyph_to_pixel_buffer(buffer: &mut Vec<u32>, width: usize, outlined: &OutlinedGlyph) {
        let bounds = outlined.px_bounds();
        outlined.draw(|x, y, c| {
            let buffer_x = bounds.min.x as i32 + x as i32;
            let buffer_y = bounds.min.y as i32 + y as i32;
            let height = buffer.len() / width;

            if buffer_x >= 0 && buffer_x < width as i32 && buffer_y >= 0 && buffer_y < height as i32 {
                let index = (buffer_y as usize) * width + (buffer_x as usize);
                let text_r = ((TEXT_COLOR >> 16) & 0xFF) as f32;
                let text_g = ((TEXT_COLOR >> 8) & 0xFF) as f32;
                let text_b = (TEXT_COLOR & 0xFF) as f32;
                let bg_r = ((BG_COLOR >> 16) & 0xFF) as f32;
                let bg_g = ((BG_COLOR >> 8) & 0xFF) as f32;
                let bg_b = (BG_COLOR & 0xFF) as f32;
                let r = (text_r * c + bg_r * (1.0 - c)) as u32;
                let g = (text_g * c + bg_g * (1.0 - c)) as u32;
                let b = (text_b * c + bg_b * (1.0 - c)) as u32;
                buffer[index] = (r << 16) | (g << 8) | b;
            }
        });
    }
}

/// TUIバックエンド用のレンダラ
pub mod tui_renderer {
    use super::*;
    
    pub fn render(font: &FontRef, text: &str, width: usize, height: usize) -> Vec<char> {
        let mut buffer = vec![' '; width * height];
        
        let font_size = height as f32 * 0.8;
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
    
    fn draw_glyph_to_char_buffer(buffer: &mut Vec<char>, width: usize, outlined: &OutlinedGlyph) {
        let bounds = outlined.px_bounds();
        outlined.draw(|x, y, c| {
            let buffer_x = bounds.min.x as usize + x as usize;
            let buffer_y = bounds.min.y as usize + y as usize;
            let height = buffer.len() / width;
            if buffer_x < width && buffer_y < height {
                let index = buffer_y * width + buffer_x;
                let coverage_char = match (c * 4.0).round() as u8 {
                    0 => ' ', 1 => '.', 2 => '*', 3 => '#', _ => '@',
                };
                if buffer[index] == ' ' { buffer[index] = coverage_char; }
            }
        });
    }
}