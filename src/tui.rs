// src/tui.rs

#[cfg(not(feature = "uefi"))]
use crate::app::{App, AppEvent, Fonts, TuiDisplayMode};
#[cfg(not(feature = "uefi"))]
use crate::model::Segment;
#[cfg(not(feature = "uefi"))]
use crate::renderer::tui_renderer;
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

#[cfg(not(feature = "uefi"))]
const VIRTUAL_CELL_WIDTH: usize = 1;
#[cfg(not(feature = "uefi"))]
const VIRTUAL_CELL_HEIGHT: usize = 1;

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

        let virtual_width = cols * VIRTUAL_CELL_WIDTH;
        let virtual_height = rows * VIRTUAL_CELL_HEIGHT;
        
        let now_time = Instant::now();
        let delta_time = now_time.duration_since(last_frame_time).as_millis() as f64;
        last_frame_time = now_time;
        
        app.update(virtual_width, virtual_height, delta_time);

        let mut current_buffer = vec![' '; cols * rows];

        let current_font = app.get_current_font();
        let render_list = ui::build_ui(&app, current_font, virtual_width, virtual_height);

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
                Renderable::TypingUpper { segments, anchor, shift, align, font_size, .. } => {
                    match app.tui_display_mode {
                        TuiDisplayMode::AsciiArt | TuiDisplayMode::Braille => {
                            let is_braille = app.tui_display_mode == TuiDisplayMode::Braille;
                            let target_art_height_in_cells = calculate_target_art_height(font_size, cols, rows);
                            if target_art_height_in_cells == 0 { continue; }
                            
                            let mut font_size_px = target_art_height_in_cells as f32 * tui_renderer::ART_V_PIXELS_PER_CELL;
                            if is_braille {
                                font_size_px *= 2.0;
                            }

                            let renderer = if is_braille { tui_renderer::render_text_to_braille_art } else { tui_renderer::render_text_to_art };

                            let total_width: u32 = segments.iter().map(|seg| {
                                renderer(current_font, &seg.base_text, font_size_px).1 as u32
                            }).sum();
                            
                            let (_, _, line_total_height, line_ascent) = renderer(current_font, "|", font_size_px);

                            if total_width == 0 { continue; }

                            let anchor_pos = ui::calculate_anchor_position(anchor, shift, cols, rows);
                            let (mut pen_x, line_start_y) = ui::calculate_aligned_position(anchor_pos, total_width, line_total_height as u32, align);
                            let line_baseline_y = line_start_y + line_ascent as i32;

                            for seg in segments {
                                let (art_buffer, art_width, _, char_ascent) = renderer(current_font, &seg.base_text, font_size_px);
                                let blit_y = line_baseline_y - char_ascent as i32;
                                blit_art(&mut current_buffer, cols, rows, &art_buffer, art_width, 0, pen_x as isize, blit_y as isize);

                                if let Some(ruby) = &seg.ruby_text {
                                    if is_braille {
                                        let ruby_font_size_px = font_size_px * 0.5;
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
                            let target_art_height_in_cells = calculate_target_art_height(font_size, cols, rows);
                            if target_art_height_in_cells == 0 { continue; }

                            let mut font_size_px = target_art_height_in_cells as f32 * tui_renderer::ART_V_PIXELS_PER_CELL;
                            if is_braille {
                                font_size_px *= 2.0;
                            }

                             let renderer = if is_braille { tui_renderer::render_text_to_braille_art } else { tui_renderer::render_text_to_art };

                            let total_width: u32 = app.typing_model.as_ref().map_or(0, |m| {
                                m.content.lines.get(m.status.line as usize).map_or(0, |line| {
                                    line.segments.iter().map(|seg| {
                                        let base_text = match seg {
                                            Segment::Plain { text } => text,
                                            Segment::Annotated { base, .. } => base,
                                        };
                                        renderer(current_font, base_text, font_size_px).1 as u32
                                    }).sum()
                                })
                            });
                            
                            let (_, _, line_total_height, line_ascent) = renderer(current_font, "|", font_size_px);

                            if total_width == 0 { continue; }

                            let anchor_pos = ui::calculate_anchor_position(anchor, shift, cols, rows);
                            let (mut pen_x, line_start_y) = ui::calculate_aligned_position(anchor_pos, total_width, line_total_height as u32, align);
                            let line_baseline_y = line_start_y + line_ascent as i32;

                            for seg in segments {
                                match seg {
                                    LowerTypingSegment::Completed { base_text, ruby_text, .. } => {
                                        let (art_buffer, art_width, _, char_ascent) = renderer(current_font, &base_text, font_size_px);
                                        let blit_y = line_baseline_y - char_ascent as i32;
                                        blit_art(&mut current_buffer, cols, rows, &art_buffer, art_width, 0, pen_x as isize, blit_y as isize);

                                        if let Some(ruby) = ruby_text {
                                            if is_braille {
                                                let ruby_font_size_px = font_size_px * 0.5;
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
                                            match el {
                                                ActiveLowerElement::Typed { character, .. } => {
                                                    let (art_buffer, art_width, _, char_ascent) = renderer(current_font, &character.to_string(), font_size_px);
                                                    let blit_y = line_baseline_y - char_ascent as i32;
                                                    blit_art(&mut current_buffer, cols, rows, &art_buffer, art_width, 0, pen_x as isize, blit_y as isize);
                                                    pen_x += art_width as i32;
                                                }
                                                ActiveLowerElement::Cursor => {
                                                    if !is_braille {
                                                        let cursor_height = line_total_height;
                                                        for y_offset in 0..cursor_height {
                                                            let target_y = line_start_y + y_offset as i32;
                                                            if target_y >= 0 && target_y < rows as i32 && pen_x >= 0 && pen_x < cols as i32 {
                                                                let index = (target_y as usize * cols) + pen_x as usize;
                                                                if index < current_buffer.len() {
                                                                    current_buffer[index] = '|';
                                                                }
                                                            }
                                                        }
                                                        pen_x += 1;
                                                    }
                                                }
                                                ActiveLowerElement::UnconfirmedInput(s) => {
                                                    let (art_buffer, art_width, _, char_ascent) = renderer(current_font, &s, font_size_px);
                                                    let blit_y = line_baseline_y - char_ascent as i32;
                                                    blit_art(&mut current_buffer, cols, rows, &art_buffer, art_width, 0, pen_x as isize, blit_y as isize);
                                                    pen_x += art_width as i32;
                                                }
                                                ActiveLowerElement::LastIncorrectInput(c) => {
                                                    let (art_buffer, art_width, _, char_ascent) = renderer(current_font, &c.to_string(), font_size_px);
                                                    let blit_y = line_baseline_y - char_ascent as i32;
                                                    blit_art(&mut current_buffer, cols, rows, &art_buffer, art_width, 0, pen_x as isize, blit_y as isize);
                                                    pen_x += art_width as i32;
                                                }
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
fn calculate_target_art_height(font_size: FontSize, cols: usize, rows: usize) -> usize {
     match font_size {
        FontSize::WindowHeight(ratio) => (rows as f32 * ratio).ceil() as usize,
        FontSize::WindowAreaSqrt(ratio) => {
            let base_dimension = (cols as f32 * rows as f32).sqrt();
            (base_dimension * ratio).ceil() as usize
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