use ab_glyph::{FontRef, Font, Glyph, point};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. フォントファイルを読み込む
    let font_data = include_bytes!("../fonts/NotoSerifJP-Regular.ttf");
    let font = FontRef::try_from_slice(font_data)?;

    // 2. グリフを準備する
    let glyph: Glyph = font.glyph_id('あ')
        .with_scale_and_position(48.0, point(0.0, 0.0));

    // 3. グリフのアウトラインを取得する
    if let Some(outlined_glyph) = font.outline_glyph(glyph) {
        let bounds = outlined_glyph.px_bounds();
        println!("Glyph bounds: {:?}", bounds);

        // --- ここからが修正箇所 ---

        // 変更点1: グリフのバウンディングボックスと同じサイズの文字バッファを作成する
        let width = bounds.width() as usize;
        let height = bounds.height() as usize;
        // 背景文字（空白）で初期化
        let mut buffer = vec![' '; width * height];

        // 変更点2: バッファにグリフを描画する。コンソールには直接出力しない
        outlined_glyph.draw(|x, y, c| {
            // 被覆率 c を文字に変換
            let coverage_char = match (c * 4.0).round() as u8 {
                0 => ' ',
                1 => '.',
                2 => '*',
                3 => '#',
                _ => '@',
            };

            // バッファ内の正しいインデックスに文字を書き込む
            let index = y as usize * width + x as usize;
            if index < buffer.len() {
                buffer[index] = coverage_char;
            }
        });

        // 変更点3: 完成したバッファをコンソールに一行ずつ表示する
        println!("--- Rasterized Glyph Start ---");
        for y in 0..height {
            for x in 0..width {
                let index = y * width + x;
                print!("{}", buffer[index]);
            }
            println!(); // 各行の終わりに改行を入れる
        }
        println!("--- Rasterized Glyph End ---");

    } else {
        println!("Glyph for 'あ' not found in font.");
    }

    Ok(())
}