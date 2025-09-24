// src/tui.rs

#[cfg(not(feature = "uefi"))]
use crate::app::{App, AppEvent, Fonts, TuiDisplayMode};
#[cfg(not(feature = "uefi"))]
use crate::model::Segment;
#[cfg(not(feature = "uefi"))]
use crate::renderer::{gui_renderer, tui_renderer}; // gui_renderer をインポート
#[cfg(not(feature = "uefi"))]
use crate::ui::{
    self, ActiveLowerElement, Align, Anchor, FontSize, HorizontalAlign, LowerTypingSegment,
    Renderable, Shift, VerticalAlign,
};
#[cfg(not(feature = "uefi"))]
use ab_glyph::FontRef;
#[cfg(not(feature = "uefi"))]
use crossterm::{
    cursor, event, execute,
    event::{Event, KeyCode, KeyEventKind},
    style::Print,
    terminal,
};
#[cfg(not(feature = "uefi"))]
use std::io::{stdout, Write};
#[cfg(not(feature = "uefi"))]
use std::time::{Duration, Instant};

// 共通のスクロール計算ロジックのために、TUIでも仮想的なピクセル幅を定義する
#[cfg(not(feature = "uefi"))]
const TUI_VIRTUAL_PIXEL_WIDTH: usize = 1000;

/// TUIアプリケーションのメイン関数
#[cfg(not(feature = "uefi"))]
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let yuji_font_data = include_bytes!("../fonts/YujiSyuku-Regular.ttf");
    let yuji_font = FontRef::try_from_slice(yuji_font_data).map_err(|_| "Failed to load Yuji Syuku font")?;
    
    let noto_font_data = include_bytes!("../fonts/NotoSerifJP-Regular.ttf");
    let noto_font = FontRef::try_from_slice(noto_font_data).map_err(|_| "Failed to load Noto Serif JP font")?;

    let fonts = Fonts {
        yuji_syuku: yuji_font,
        noto_serif: noto_font,
    };

    let mut stdout = stdout();
    terminal::enable_raw_mode()?;
    execute!(stdout, terminal::EnterAlternateScreen, cursor::Hide)?;

    let mut app = App::new(fonts);
    app.on_event(AppEvent::Start);

    let mut previous_buffer = Vec::new();
    let mut last_frame_time = Instant::now();

    while !app.should_quit {
        let (cols, rows) = terminal::size()?;
        let (cols, rows) = (cols as usize, rows as usize);

        // ターミナルのアスペクト比に合わせて仮想的な高さを計算
        let virtual_height = if cols > 0 {
            (TUI_VIRTUAL_PIXEL_WIDTH as f32 / cols as f32 * rows as f32 * (1.0 / tui_renderer::TUI_CHAR_ASPECT_RATIO)) as usize
        } else {
            0
        };
        
        let now_time = Instant::now();
        let delta_time = now_time.duration_since(last_frame_time).as_millis() as f64;
        last_frame_time = now_time;
        
        // app.updateには仮想ピクセルサイズを渡す
        app.update(TUI_VIRTUAL_PIXEL_WIDTH, virtual_height, delta_time);

        let mut current_buffer = vec![' '; cols * rows];

        let current_font = app.get_current_font();
        // ui.build_uiにも仮想ピクセルサイズを渡す
        let render_list = ui::build_ui(&app, current_font, TUI_VIRTUAL_PIXEL_WIDTH, virtual_height);

        for item in render_list {
            match item {
                Renderable::Background { .. } => { /* TUIでは何もしない */ }
                Renderable::BigText { text, anchor, shift, align, font_size, .. } => {
                    match app.tui_display_mode {
                        TuiDisplayMode::AsciiArt | TuiDisplayMode::Braille => {
                            let is_braille = app.tui_display_mode == TuiDisplayMode::Braille;
                            draw_art_text(&mut current_buffer, current_font, &text, anchor, shift, align, font_size, cols, rows, is_braille);
                        }
                        TuiDisplayMode::SimpleText => {
                            draw_plain_text(&mut current_buffer, &text, anchor, shift, align, cols, rows);
                        }
                    }
                }
                Renderable::Text { text, anchor, shift, align, .. } => {
                    draw_plain_text(&mut current_buffer, &text, anchor, shift, align, cols, rows);
                }
                Renderable::TypingUpper { segments, anchor, shift, align, font_size } => {
                     match app.tui_display_mode {
                        TuiDisplayMode::AsciiArt | TuiDisplayMode::Braille => {
                            let is_braille = app.tui_display_mode == TuiDisplayMode::Braille;
                            let font_size_px = crate::renderer::calculate_pixel_font_size(font_size, TUI_VIRTUAL_PIXEL_WIDTH, virtual_height);

                            let mut render_font_size = font_size_px;
                            if is_braille { render_font_size *= 2.0; }
                            let renderer = if is_braille { tui_renderer::render_text_to_braille_art } else { tui_renderer::render_text_to_art };
                            
                            let (total_width_cells, total_width_pixels) = segments.iter().fold((0_u32, 0.0_f32), |(acc_cells, acc_pixels), seg| {
                                let cells = renderer(current_font, &seg.base_text, render_font_size).1 as u32;
                                let pixels = gui_renderer::measure_text(current_font, &seg.base_text, font_size_px).0 as f32;
                                (acc_cells + cells, acc_pixels + pixels)
                            });

                            if total_width_cells == 0 { continue; }
                            
                            let pixels_per_cell = if total_width_cells > 0 { total_width_pixels as f64 / total_width_cells as f64 } else { 1.0 };
                            let scroll_offset_cells = (app.typing_model.as_ref().unwrap().scroll.scroll / pixels_per_cell).round() as i32;

                            let (_, _, line_total_height, line_ascent) = renderer(current_font, "|", render_font_size);

                            let y_only_shift = Shift { x: 0.0, y: shift.y };
                            let anchor_pos = ui::calculate_anchor_position(anchor, y_only_shift, cols, rows);
                            let (center_pen_x, line_start_y) = ui::calculate_aligned_position(anchor_pos, total_width_cells, line_total_height as u32, align);
                            let mut pen_x = center_pen_x - scroll_offset_cells;
                            let line_baseline_y = line_start_y + line_ascent as i32;

                            for seg in segments {
                                let (art_buffer, art_width, _, char_ascent) = renderer(current_font, &seg.base_text, render_font_size);
                                let blit_y = line_baseline_y - char_ascent as i32;
                                blit_art(&mut current_buffer, cols, rows, &art_buffer, art_width, 0, pen_x as isize, blit_y as isize);

                                if let Some(ruby) = &seg.ruby_text {
                                    if is_braille {
                                        let ruby_font_size_px = render_font_size * 0.5;
                                        let (ruby_art_buffer, ruby_art_width, ruby_art_height, _) = tui_renderer::render_text_to_braille_art(current_font, &ruby, ruby_font_size_px);
                                        let ruby_anchor_pos = (pen_x + (art_width as i32 / 2), line_start_y - 1);
                                        let (ruby_x, ruby_y) = ui::calculate_aligned_position(ruby_anchor_pos, ruby_art_width as u32, ruby_art_height as u32, Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Bottom });
                                        blit_art(&mut current_buffer, cols, rows, &ruby_art_buffer, ruby_art_width, ruby_art_height, ruby_x as isize, ruby_y as isize);
                                    } else {
                                        let (ruby_width, _) = measure_plain_text(ruby);
                                        let ruby_anchor_pos = (pen_x + (art_width as i32 / 2), line_start_y - 1);
                                        let (ruby_x, ruby_y) = ui::calculate_aligned_position(ruby_anchor_pos, ruby_width, 1, Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Bottom });
                                        draw_plain_text_at(&mut current_buffer, ruby, ruby_x, ruby_y, cols);
                                    }
                                }
                                pen_x += art_width as i32;
                            }
                        }
                        TuiDisplayMode::SimpleText => {
                            let text = segments.iter().map(|s| s.base_text.clone()).collect::<String>();
                            draw_simple_typing_text(&mut current_buffer, &text, anchor, shift, align, cols, rows);
                        }
                    }
                }
                Renderable::TypingLower { segments, anchor, shift, align, font_size, .. } => {
                    match app.tui_display_mode {
                        TuiDisplayMode::AsciiArt | TuiDisplayMode::Braille => {
                            let is_braille = app.tui_display_mode == TuiDisplayMode::Braille;
                            let font_size_px = crate::renderer::calculate_pixel_font_size(font_size, TUI_VIRTUAL_PIXEL_WIDTH, virtual_height);
                            let mut render_font_size = font_size_px;
                            if is_braille { render_font_size *= 2.0; }
                            let renderer = if is_braille { tui_renderer::render_text_to_braille_art } else { tui_renderer::render_text_to_art };
                            
                            let full_line_words = &app.typing_model.as_ref().unwrap().content.lines[app.typing_model.as_ref().unwrap().status.line as usize].words;

                            let (total_width_cells, total_width_pixels) = full_line_words.iter().flat_map(|w| &w.segments).fold((0_u32, 0.0_f32), |(acc_cells, acc_pixels), seg| {
                                let text = match seg {
                                    Segment::Plain { text } => text.as_str(),
                                    Segment::Annotated { base, .. } => base.as_str(),
                                };
                                let cells = renderer(current_font, text, render_font_size).1 as u32;
                                let pixels = gui_renderer::measure_text(current_font, text, font_size_px).0 as f32;
                                (acc_cells + cells, acc_pixels + pixels)
                            });

                            if total_width_cells == 0 { continue; }

                            let pixels_per_cell = if total_width_cells > 0 { total_width_pixels as f64 / total_width_cells as f64 } else { 1.0 };
                            let scroll_offset_cells = (app.typing_model.as_ref().unwrap().scroll.scroll / pixels_per_cell).round() as i32;

                            let (_, _, line_total_height, line_ascent) = renderer(current_font, "|", render_font_size);

                            let y_only_shift = Shift { x: 0.0, y: shift.y };
                            let anchor_pos = ui::calculate_anchor_position(anchor, y_only_shift, cols, rows);
                            let (center_pen_x, line_start_y) = ui::calculate_aligned_position(anchor_pos, total_width_cells, line_total_height as u32, align);
                            let mut pen_x = center_pen_x - scroll_offset_cells;
                            let line_baseline_y = line_start_y + line_ascent as i32;

                            for seg in segments {
                                match seg {
                                    LowerTypingSegment::Completed { base_text, ruby_text, .. } => {
                                        let (art_buffer, art_width, _, char_ascent) = renderer(current_font, &base_text, render_font_size);
                                        let blit_y = line_baseline_y - char_ascent as i32;
                                        blit_art(&mut current_buffer, cols, rows, &art_buffer, art_width, 0, pen_x as isize, blit_y as isize);

                                        if let Some(ruby) = ruby_text {
                                            if is_braille {
                                                let ruby_font_size_px = render_font_size * 0.5;
                                                let (ruby_art_buffer, ruby_art_width, ruby_art_height, _) = tui_renderer::render_text_to_braille_art(current_font, &ruby, ruby_font_size_px);
                                                let ruby_anchor_pos = (pen_x + (art_width as i32 / 2), line_start_y - 1);
                                                let (ruby_x, ruby_y) = ui::calculate_aligned_position(ruby_anchor_pos, ruby_art_width as u32, ruby_art_height as u32, Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Bottom });
                                                blit_art(&mut current_buffer, cols, rows, &ruby_art_buffer, ruby_art_width, ruby_art_height, ruby_x as isize, ruby_y as isize);
                                            } else {
                                                let (ruby_width, _) = measure_plain_text(&ruby);
                                                let ruby_anchor = (pen_x + (art_width as i32 / 2), line_start_y - 1);
                                                let (rx, ry) = ui::calculate_aligned_position(ruby_anchor, ruby_width, 1, Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Bottom });
                                                draw_plain_text_at(&mut current_buffer, &ruby, rx, ry, cols);
                                            }
                                        }
                                        pen_x += art_width as i32;
                                    }
                                    LowerTypingSegment::Active { elements } => {
                                        for el in elements {
                                            let text_to_render = match el {
                                                ActiveLowerElement::Typed { character, .. } => character.to_string(),
                                                ActiveLowerElement::Cursor => "|".to_string(),
                                                ActiveLowerElement::UnconfirmedInput(s) => s.clone(),
                                                ActiveLowerElement::LastIncorrectInput(c) => c.to_string(),
                                            };

                                            if text_to_render == "|" {
                                                let cursor_height = line_total_height;
                                                for y_offset in 0..cursor_height {
                                                    let target_y = line_start_y + y_offset as i32;
                                                    if target_y >= 0 && target_y < rows as i32 && pen_x >= 0 && pen_x < cols as i32 {
                                                        current_buffer[target_y as usize * cols + pen_x as usize] = '|';
                                                    }
                                                }
                                                pen_x += 1; // カーソルは常に1セル幅
                                            } else {
                                                let (art_buffer, art_width, _, char_ascent) = renderer(current_font, &text_to_render, render_font_size);
                                                let blit_y = line_baseline_y - char_ascent as i32;
                                                blit_art(&mut current_buffer, cols, rows, &art_buffer, art_width, 0, pen_x as isize, blit_y as isize);
                                                pen_x += art_width as i32;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        TuiDisplayMode::SimpleText => {
                            let mut text_to_draw = String::new();
                            for seg in segments {
                                match seg {
                                    LowerTypingSegment::Completed { base_text, .. } => text_to_draw.push_str(&base_text),
                                    LowerTypingSegment::Active { elements } => {
                                        for el in elements {
                                            match el {
                                                ActiveLowerElement::Typed { character, .. } => text_to_draw.push(character),
                                                ActiveLowerElement::Cursor => text_to_draw.push('|'),
                                                ActiveLowerElement::UnconfirmedInput(s) => text_to_draw.push_str(&s),
                                                ActiveLowerElement::LastIncorrectInput(c) => text_to_draw.push(c),
                                            }
                                        }
                                    }
                                }
                            }
                            draw_simple_typing_text(&mut current_buffer, &text_to_draw, anchor, shift, align, cols, rows);
                        }
                    }
                }
            }
        }

        draw_buffer_to_terminal(&mut stdout, &current_buffer, &previous_buffer, cols)?;
        previous_buffer = current_buffer;

        handle_input(&mut app)?;
    }

    execute!(stdout, cursor::Show, terminal::LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;
    Ok(())
}

/// ASCIIまたは点字アートをバッファに転写する
#[cfg(not(feature = "uefi"))]
fn blit_art(
    buffer: &mut [char], buf_w: usize, buf_h: usize,
    art: &[char], art_w: usize, _art_h: usize,
    start_x: isize, start_y: isize,
) {
    if art_w == 0 { return; }
    let art_h = if art.is_empty() { 0 } else { art.len() / art_w };

    for y in 0..art_h {
        let target_y = start_y + y as isize;
        if target_y >= 0 && target_y < buf_h as isize {
            for x in 0..art_w {
                let target_x = start_x + x as isize;
                if target_x >= 0 && target_x < buf_w as isize {
                    let art_char = art[y * art_w + x];
                    if art_char != ' ' { // Don't blit spaces
                        buffer[target_y as usize * buf_w + target_x as usize] = art_char;
                    }
                }
            }
        }
    }
}

/// TUIでのテキストの寸法を計算する（文字数、1行）
#[cfg(not(feature = "uefi"))]
fn measure_plain_text(text: &str) -> (u32, u32) {
    (text.chars().count() as u32, 1)
}

/// 通常のテキストを描画する
#[cfg(not(feature = "uefi"))]
fn draw_plain_text(
    buffer: &mut [char], text: &str, anchor: Anchor, shift: Shift, align: Align,
    width: usize, height: usize,
) {
    let (text_width, text_height) = measure_plain_text(text);
    let anchor_pos = ui::calculate_anchor_position(anchor, shift, width, height);
    let (start_x, start_y) = ui::calculate_aligned_position(anchor_pos, text_width, text_height, align);
    draw_plain_text_at(buffer, text, start_x, start_y, width);
}

/// 指定した座標にプレーンテキストを描画するヘルパー関数
#[cfg(not(feature = "uefi"))]
fn draw_plain_text_at(buffer: &mut [char], text: &str, x: i32, y: i32, width: usize) {
    if y < 0 || y >= (buffer.len() / width) as i32 { return; }
    for (i, c) in text.chars().enumerate() {
        let current_x = x + i as i32;
        if current_x >= 0 && current_x < width as i32 {
            if (y as usize * width + current_x as usize) < buffer.len() {
                buffer[y as usize * width + current_x as usize] = c;
            }
        }
    }
}


/// AA化または点字化されたテキストを描画する
#[cfg(not(feature = "uefi"))]
fn draw_art_text(
    buffer: &mut [char], font: &FontRef, text: &str, anchor: Anchor, shift: Shift, align: Align, font_size: FontSize,
    cols: usize, rows: usize, is_braille: bool,
) {
    let target_art_height_in_cells = calculate_target_art_height(font_size, cols, rows);
    if target_art_height_in_cells == 0 { return; }

    let mut font_size_px = target_art_height_in_cells as f32 * tui_renderer::ART_V_PIXELS_PER_CELL;
    if is_braille {
        font_size_px *= 2.0;
    }

    let (art_buffer, art_width, art_height, _) = if is_braille {
        tui_renderer::render_text_to_braille_art(font, text, font_size_px)
    } else {
        tui_renderer::render_text_to_art(font, text, font_size_px)
    };

    if art_width == 0 || art_height == 0 { return; }
    
    let anchor_pos = ui::calculate_anchor_position(anchor, shift, cols, rows);
    let (start_x, start_y) = ui::calculate_aligned_position(anchor_pos, art_width as u32, art_height as u32, align);
    
    blit_art(buffer, cols, rows, &art_buffer, art_width, art_height, start_x as isize, start_y as isize);
}

/// フォントサイズ指定から目標となるAAの高さを計算するヘルパー関数
#[cfg(not(feature = "uefi"))]
fn calculate_target_art_height(font_size: FontSize, _cols: usize, rows: usize) -> usize {
     match font_size {
        FontSize::WindowHeight(ratio) => (rows as f32 * ratio).ceil() as usize,
        FontSize::WindowAreaSqrt(ratio) => {
            // TUIではアスペクト比が不定なため、高さを基準にする
            (rows as f32 * ratio * 2.0).ceil() as usize
        }
    }
}

#[cfg(feature = "uefi")]
pub fn run() -> Result<(), Box<dyn core::error::Error>> {
    Err("TUI is not supported in UEFI environment yet.".into())
}

/// 差分を検出し、ターミナルに必要な部分だけ描画する
#[cfg(not(feature = "uefi"))]
fn draw_buffer_to_terminal(
    stdout: &mut impl Write, current_buffer: &[char], previous_buffer: &[char], width: usize,
) -> std::io::Result<()> {
    if previous_buffer.len() != current_buffer.len() {
        execute!(stdout, terminal::Clear(terminal::ClearType::All))?;
        for (y, row) in current_buffer.chunks(width).enumerate() {
            let line: String = row.iter().collect();
            if y < u16::MAX as usize {
                execute!(stdout, cursor::MoveTo(0, y as u16), Print(line))?;
            }
        }
    } else {
        for (y, (current_row, previous_row)) in current_buffer
            .chunks(width)
            .zip(previous_buffer.chunks(width))
            .enumerate()
        {
            if current_row != previous_row {
                let line: String = current_row.iter().collect();
                if y < u16::MAX as usize {
                    execute!(stdout, cursor::MoveTo(0, y as u16), Print(line))?;
                }
            }
        }
    }
    stdout.flush()
}


/// キーボード入力を処理する
#[cfg(not(feature = "uefi"))]
fn handle_input(app: &mut App) -> std::io::Result<()> {
    if event::poll(Duration::from_millis(100))? {
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char(c) => app.on_event(AppEvent::Char { c, timestamp: crate::timestamp::now() }),
                    KeyCode::Backspace => app.on_event(AppEvent::Backspace),
                    KeyCode::Up => app.on_event(AppEvent::Up),
                    KeyCode::Down => app.on_event(AppEvent::Down),
                    KeyCode::Enter => app.on_event(AppEvent::Enter),
                    KeyCode::Esc => app.on_event(AppEvent::Escape),
                    KeyCode::Tab => app.on_event(AppEvent::CycleTuiMode),
                    _ => {}
                }
            }
        }
    }
    Ok(())
}

/// シンプルモード用にテキストを描画するヘルパー関数
#[cfg(not(feature = "uefi"))]
fn draw_simple_typing_text(
    buffer: &mut [char],
    text: &str,
    anchor: Anchor,
    shift: Shift,
    align: Align,
    width: usize,
    height: usize,
) {
    draw_plain_text(buffer, text, anchor, shift, align, width, height);
}