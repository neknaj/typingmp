use ab_glyph::{FontRef, Font, GlyphId, OutlinedGlyph, point};

struct RubySegment<'a> {
    base: &'a str,
    ruby: &'a str,
}
enum TextSegment<'a> {
    Plain(&'a str),
    WithRuby(RubySegment<'a>),
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // --- 設定 ---
    let font_data = include_bytes!("../fonts/NotoSerifJP-Regular.ttf");
    let font = FontRef::try_from_slice(font_data)?;

    let base_font_size = 32.0;
    let ruby_font_size = 16.0;

    let base_y_pos = 50.0;
    
    let ruby_y_pos = base_y_pos - base_font_size * 0.65; 
    // -------------

    // --- 描画バッファの準備 ---
    let buffer_width = 500;
    let buffer_height = 80;
    let mut buffer = vec![' '; buffer_width * buffer_height];

    // --- スケール値の計算 ---
    let base_scale = base_font_size / (font.ascent_unscaled() - font.descent_unscaled());
    let ruby_scale = ruby_font_size / (font.ascent_unscaled() - font.descent_unscaled());

    // --- 描画したい文章 ---
    let segments = vec![
        TextSegment::WithRuby(RubySegment { base: "私", ruby: "わたし" }),
        TextSegment::Plain("の"),
        TextSegment::WithRuby(RubySegment { base: "名前", ruby: "なまえ" }),
        TextSegment::Plain("は"),
        TextSegment::WithRuby(RubySegment { base: "山田太郎", ruby: "やまだたろう" }),
        TextSegment::Plain("です。"),
    ];

    // --- 文字列のレイアウトと描画 ---
    let mut pen_x = 10.0; 

    for segment in segments {
        match segment {
            TextSegment::Plain(text) => {
                for character in text.chars() {
                    let glyph_id = font.glyph_id(character);
                    let glyph = glyph_id.with_scale_and_position(base_font_size, point(pen_x, base_y_pos));
                    if let Some(outlined) = font.outline_glyph(glyph) {
                        draw_glyph_to_buffer(&mut buffer, buffer_width, &outlined);
                    }
                    pen_x += font.h_advance_unscaled(glyph_id) * base_scale;
                }
            }
            TextSegment::WithRuby(ruby_segment) => {
                let base_width = measure_text_width(&font, ruby_segment.base, base_scale);
                let ruby_width = measure_text_width(&font, ruby_segment.ruby, ruby_scale);
                let ruby_start_x = pen_x + (base_width - ruby_width) / 2.0;
                let mut ruby_pen_x = ruby_start_x;
                for character in ruby_segment.ruby.chars() {
                    let glyph_id = font.glyph_id(character);
                    let glyph = glyph_id.with_scale_and_position(ruby_font_size, point(ruby_pen_x, ruby_y_pos));
                    if let Some(outlined) = font.outline_glyph(glyph) {
                        draw_glyph_to_buffer(&mut buffer, buffer_width, &outlined);
                    }
                    ruby_pen_x += font.h_advance_unscaled(glyph_id) * ruby_scale;
                }
                let mut base_pen_x = pen_x;
                for character in ruby_segment.base.chars() {
                    let glyph_id = font.glyph_id(character);
                    let glyph = glyph_id.with_scale_and_position(base_font_size, point(base_pen_x, base_y_pos));
                    if let Some(outlined) = font.outline_glyph(glyph) {
                        draw_glyph_to_buffer(&mut buffer, buffer_width, &outlined);
                    }
                    base_pen_x += font.h_advance_unscaled(glyph_id) * base_scale;
                }
                pen_x += base_width;
            }
        }
    }

    print_buffer(&buffer, buffer_width, buffer_height);

    Ok(())
}

fn measure_text_width(font: &FontRef, text: &str, scale: f32) -> f32 {
    let mut total_width = 0.0;
    for character in text.chars() {
        let glyph_id = font.glyph_id(character);
        total_width += font.h_advance_unscaled(glyph_id) * scale;
    }
    total_width
}
fn draw_glyph_to_buffer(buffer: &mut Vec<char>, width: usize, outlined_glyph: &OutlinedGlyph) {
    let bounds = outlined_glyph.px_bounds();
    outlined_glyph.draw(|x, y, c| {
        let buffer_x = bounds.min.x as usize + x as usize;
        let buffer_y = bounds.min.y as usize + y as usize;
        let height = buffer.len() / width;
        if buffer_x < width && buffer_y < height {
            let index = buffer_y * width + buffer_x;
            let coverage_char = match (c * 4.0).round() as u8 {
                0 => ' ', 1 => '.', 2 => '*', 3 => '#', _ => '@',
            };
            if buffer[index] == ' ' || buffer[index] == '.' { buffer[index] = coverage_char; }
        }
    });
}
fn print_buffer(buffer: &[char], width: usize, height: usize) {
    println!("--- Rendered String Start ---");
    for y in 0..height {
        for x in 0..width {
            print!("{}", buffer[y * width + x]);
        }
        println!();
    }
    println!("--- Rendered String End ---");
}