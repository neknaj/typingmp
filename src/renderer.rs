// src/renderer.rs
// uefi featureが有効な場合、標準のallocクレートをインポート
#[cfg(feature = "uefi")]
extern crate alloc;

// uefi と std で使用する Vec と vec! を切り替える
#[cfg(feature = "uefi")]
use alloc::vec;
#[cfg(feature = "uefi")]
use alloc::vec::Vec;
#[cfg(not(feature = "uefi"))]
use std::vec::Vec;

#[cfg(feature = "uefi")]
use core_maths::CoreFloat;

use crate::ui::FontSize;
use ab_glyph::{point, Font, FontRef, OutlinedGlyph, PxScale, ScaleFont};

/// 背景の描画色
pub const BG_COLOR: u32 = 0x00_000000;

/// ピクセルバッファに線形グラデーションを描画する
pub fn draw_linear_gradient(
    buffer: &mut [u32],
    width: usize,
    height: usize,
    start_color: u32,
    end_color: u32,
    start_point: (f32, f32),
    end_point: (f32, f32),
) {
    let (x0, y0) = start_point;
    let (x1, y1) = end_point;

    let dx = x1 - x0;
    let dy = y1 - y0;
    let len_sq = dx * dx + dy * dy;

    for y in 0..height {
        for x in 0..width {
            let p_x = x as f32;
            let p_y = y as f32;

            let dot_product = (p_x - x0) * dx + (p_y - y0) * dy;
            let ratio = if len_sq == 0.0 {
                0.0
            } else {
                (dot_product / len_sq).clamp(0.0, 1.0)
            };

            let r = (((start_color >> 16) & 0xFF) as f32 * (1.0 - ratio)
                + ((end_color >> 16) & 0xFF) as f32 * ratio) as u32;
            let g = (((start_color >> 8) & 0xFF) as f32 * (1.0 - ratio)
                + ((end_color >> 8) & 0xFF) as f32 * ratio) as u32;
            let b = (((start_color) & 0xFF) as f32 * (1.0 - ratio)
                + ((end_color) & 0xFF) as f32 * ratio) as u32;
            let interpolated_color = (0xFF << 24) | (r << 16) | (g << 8) | b;

            let index = y * width + x;
            buffer[index] = interpolated_color;
        }
    }
}

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
        buffer: &mut [u32],
        stride: usize,
        font: &FontRef,
        text: &str,
        pos: (f32, f32),
        font_size: f32,
        color: u32,
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
                draw_glyph_to_pixel_buffer(buffer, stride, &outlined, color);
            }
            pen_x += scaled_font.h_advance(glyph_id);
            last_glyph = Some(glyph_id);
        }
    }

    /// アウトライン化されたグリフをピクセルバッファに描画する（内部関数）
    fn draw_glyph_to_pixel_buffer(
        buffer: &mut [u32],
        stride: usize,
        outlined: &OutlinedGlyph,
        color: u32,
    ) {
        let bounds = outlined.px_bounds();
        outlined.draw(|x, y, c| {
            let buffer_x = bounds.min.x as i32 + x as i32;
            let buffer_y = bounds.min.y as i32 + y as i32;
            let height = buffer.len() / stride;
            if buffer_x >= 0
                && buffer_x < stride as i32
                && buffer_y >= 0
                && buffer_y < height as i32
            {
                let index = (buffer_y as usize) * stride + (buffer_x as usize);
                let text_r = ((color >> 16) & 0xFF) as f32;
                let text_g = ((color >> 8) & 0xFF) as f32;
                let text_b = (color & 0xFF) as f32;
                let bg_r = ((buffer[index] >> 16) & 0xFF) as f32;
                let bg_g = ((buffer[index] >> 8) & 0xFF) as f32;
                let bg_b = (buffer[index] & 0xFF) as f32;
                let r = (text_r * c + bg_r * (1.0 - c)) as u32;
                let g = (text_g * c + bg_g * (1.0 - c)) as u32;
                let b = (text_b * c + bg_b * (1.0 - c)) as u32;
                buffer[index] = (0xFF << 24) | (r << 16) | (g << 8) | b;
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
    #[cfg(not(feature = "uefi"))]
    use std::convert::TryFrom;

    // TUIの1文字の縦横比をおよそ2:1と仮定
    const TUI_CHAR_ASPECT_RATIO: f32 = 2.0;
    // アートの1セルを構成する仮想ピクセル数。小さいほど高解像度（大きく）なる
    pub const ART_V_PIXELS_PER_CELL: f32 = 2.0;

    /// 指定されたテキストをASCIIアート化し、(文字バッファ, 幅, 高さ, アセント)を返す
    pub fn render_text_to_art(
        font: &FontRef,
        text: &str,
        font_size_px: f32,
    ) -> (Vec<char>, usize, usize, usize) {
        if text.is_empty() {
            return (Vec::new(), 0, 0, 0);
        }

        let scale = PxScale::from(font_size_px);
        let scaled_font = font.as_scaled(scale);
        let ascent = scaled_font.ascent();

        // アート全体のピクセル単位でのバウンディングボックスを計算
        let mut min_x = f32::MAX;
        let mut max_x = f32::MIN;
        let mut min_y = f32::MAX;
        let mut max_y = f32::MIN;
        let mut pen_x = 0.0;
        let mut last_glyph = None;

        for c in text.chars() {
            let glyph_id = font.glyph_id(c);
            if let Some(last) = last_glyph {
                pen_x += scaled_font.kern(last, glyph_id);
            }
            if let Some(outlined) = font.outline_glyph(glyph_id.with_scale(scale)) {
                let bounds = outlined.px_bounds();
                min_x = min_x.min(pen_x + bounds.min.x);
                max_x = max_x.max(pen_x + bounds.max.x);
                min_y = min_y.min(ascent + bounds.min.y);
                max_y = max_y.max(ascent + bounds.max.y);
            }
            pen_x += scaled_font.h_advance(glyph_id);
            last_glyph = Some(glyph_id);
        }
        max_x = max_x.max(pen_x); // 最後の文字の右端も考慮

        if min_x > max_x { // テキストに描画可能なグリフがなかった場合
            return (Vec::new(), 0, 0, 0);
        }

        let art_cell_height = ART_V_PIXELS_PER_CELL;
        let art_cell_width = art_cell_height / TUI_CHAR_ASPECT_RATIO;

        let art_width = ((max_x - min_x) / art_cell_width).ceil() as usize;
        let art_height = ((max_y - min_y) / art_cell_height).ceil() as usize;

        if art_width == 0 || art_height == 0 {
            return (Vec::new(), 0, 0, 0);
        }

        // --- アセント計算 ---
        let ascent_in_pixels = ascent - min_y;
        let ascent_in_cells = (ascent_in_pixels / art_cell_height).floor().max(0.0) as usize;
        // --- ここまで ---

        let mut coverage_buffer = vec![0.0f32; art_width * art_height];

        // グリフを描画し、各セルのカバレッジを計算
        pen_x = 0.0;
        last_glyph = None;
        for c in text.chars() {
            let glyph_id = font.glyph_id(c);
            if let Some(last) = last_glyph {
                pen_x += scaled_font.kern(last, glyph_id);
            }
            let glyph = glyph_id.with_scale_and_position(scale, point(pen_x, ascent));
            if let Some(outlined) = font.outline_glyph(glyph) {
                let bounds = outlined.px_bounds();
                outlined.draw(|x, y, v| {
                    let px = bounds.min.x + x as f32 - min_x;
                    let py = bounds.min.y + y as f32 - min_y;

                    let cell_x = (px / art_cell_width) as i32;
                    let cell_y = (py / art_cell_height) as i32;

                    if cell_x >= 0 && cell_x < art_width as i32 && cell_y >= 0 && cell_y < art_height as i32 {
                        let index = cell_y as usize * art_width + cell_x as usize;
                        coverage_buffer[index] = (coverage_buffer[index] + v).min(1.0);
                    }
                });
            }
            pen_x += scaled_font.h_advance(glyph_id);
            last_glyph = Some(glyph_id);
        }

        // カバレッジを文字に変換
        let char_buffer = coverage_buffer
            .into_iter()
            .map(|c| match (c * 4.99) as u8 {
                0 => ' ',
                1 => '.',
                2 => '*',
                3 => '#',
                _ => '@',
            })
            .collect();

        (char_buffer, art_width, art_height, ascent_in_cells)
    }

    /// 指定されたテキストを点字アート化し、(文字バッファ, 幅, 高さ, アセント)を返す
    pub fn render_text_to_braille_art(
        font: &FontRef,
        text: &str,
        font_size_px: f32,
    ) -> (Vec<char>, usize, usize, usize) {
        if text.is_empty() {
            return (Vec::new(), 0, 0, 0);
        }

        let scale = PxScale::from(font_size_px);
        let scaled_font = font.as_scaled(scale);
        let ascent = scaled_font.ascent();

        // アート全体のピクセル単位でのバウンディングボックスを計算 (ASCIIアート版と同じ)
        let mut min_x = f32::MAX;
        let mut max_x = f32::MIN;
        let mut min_y = f32::MAX;
        let mut max_y = f32::MIN;
        let mut pen_x = 0.0;
        let mut last_glyph = None;

        for c in text.chars() {
            let glyph_id = font.glyph_id(c);
            if let Some(last) = last_glyph {
                pen_x += scaled_font.kern(last, glyph_id);
            }
            if let Some(outlined) = font.outline_glyph(glyph_id.with_scale(scale)) {
                let bounds = outlined.px_bounds();
                min_x = min_x.min(pen_x + bounds.min.x);
                max_x = max_x.max(pen_x + bounds.max.x);
                min_y = min_y.min(ascent + bounds.min.y);
                max_y = max_y.max(ascent + bounds.max.y);
            }
            pen_x += scaled_font.h_advance(glyph_id);
            last_glyph = Some(glyph_id);
        }
        max_x = max_x.max(pen_x);

        if min_x > max_x {
            return (Vec::new(), 0, 0, 0);
        }
        
        // 点字は 4x2 のグリッド。1文字セル(高さ=幅*2)の比率に合わせる
        let art_cell_height = 4.0; 
        // FIX: アスペクト比の計算を修正
        let art_cell_width = art_cell_height / TUI_CHAR_ASPECT_RATIO;

        let art_width = ((max_x - min_x) / art_cell_width).ceil() as usize;
        let art_height = ((max_y - min_y) / art_cell_height).ceil() as usize;

        if art_width == 0 || art_height == 0 {
            return (Vec::new(), 0, 0, 0);
        }
        
        let ascent_in_pixels = ascent - min_y;
        let ascent_in_cells = (ascent_in_pixels / art_cell_height).floor().max(0.0) as usize;
        
        // グリフのピクセルカバレッジを計算するための高解像度バッファ
        // 点字の各ドットに対応させるため、TUIセルの2x4倍の解像度にする
        let sub_w = art_width * 2;
        let sub_h = art_height * 4;
        let mut sub_pixel_buffer = vec![0.0f32; sub_w * sub_h];

        pen_x = 0.0;
        last_glyph = None;
        for c in text.chars() {
            let glyph_id = font.glyph_id(c);
            if let Some(last) = last_glyph {
                pen_x += scaled_font.kern(last, glyph_id);
            }
            let glyph = glyph_id.with_scale_and_position(scale, point(pen_x, ascent));
            if let Some(outlined) = font.outline_glyph(glyph) {
                let bounds = outlined.px_bounds();
                outlined.draw(|x, y, v| {
                    let px = bounds.min.x + x as f32 - min_x;
                    let py = bounds.min.y + y as f32 - min_y;

                    // 高解像度バッファのどのサブピクセルに対応するか計算
                    let sub_x = (px / art_cell_width * 2.0) as i32;
                    let sub_y = (py / art_cell_height * 4.0) as i32;

                    if sub_x >= 0 && sub_x < sub_w as i32 && sub_y >= 0 && sub_y < sub_h as i32 {
                        let index = sub_y as usize * sub_w + sub_x as usize;
                        sub_pixel_buffer[index] = (sub_pixel_buffer[index] + v).min(1.0);
                    }
                });
            }
            pen_x += scaled_font.h_advance(glyph_id);
            last_glyph = Some(glyph_id);
        }
        
        // 高解像度バッファから点字文字バッファを生成
        let mut char_buffer = Vec::with_capacity(art_width * art_height);
        // 点字ドットとビットのマッピング
        // 1 • • 4  -> bit 0, 3
        // 2 • • 5  -> bit 1, 4
        // 3 • • 6  -> bit 2, 5
        // 7 • • 8  -> bit 6, 7
        const BIT_MAP: [[u8; 2]; 4] = [[0, 3], [1, 4], [2, 5], [6, 7]];

        for y in 0..art_height {
            for x in 0..art_width {
                let mut braille_byte: u32 = 0;
                // 2x4 のサブピクセルをチェック
                for dy in 0..4 {
                    for dx in 0..2 {
                        let sub_x = x * 2 + dx;
                        let sub_y = y * 4 + dy;
                        let index = sub_y * sub_w + sub_x;
                        if sub_pixel_buffer[index] > 0.3 { // カバレッジの閾値
                            braille_byte |= 1 << BIT_MAP[dy][dx];
                        }
                    }
                }
                // Unicodeの点字パターンは U+2800 から始まる
                let braille_char = char::try_from(0x2800 + braille_byte).unwrap_or(' ');
                char_buffer.push(braille_char);
            }
        }

        (char_buffer, art_width, art_height, ascent_in_cells)
    }
}