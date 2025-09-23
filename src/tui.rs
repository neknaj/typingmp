// src/tui.rs

#[cfg(not(feature = "uefi"))]
use crate::app::{App, AppEvent};
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
use std::time::Duration;

#[cfg(not(feature = "uefi"))]
const VIRTUAL_CELL_WIDTH: usize = 1;
#[cfg(not(feature = "uefi"))]
const VIRTUAL_CELL_HEIGHT: usize = 1;

/// TUIアプリケーションのメイン関数
#[cfg(not(feature = "uefi"))]
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let font_data = include_bytes!("../fonts/NotoSerifJP-Regular.ttf");
    let font = FontRef::try_from_slice(font_data).map_err(|_| "Failed to load font from slice")?;

    let mut stdout = stdout();
    terminal::enable_raw_mode()?;
    execute!(stdout, terminal::EnterAlternateScreen, cursor::Hide)?;

    let mut app = App::new();
    app.on_event(AppEvent::Start);

    let mut previous_buffer = Vec::new();

    while !app.should_quit {
        let (cols, rows) = terminal::size()?;
        let (cols, rows) = (cols as usize, rows as usize);

        let virtual_width = cols * VIRTUAL_CELL_WIDTH;
        let virtual_height = rows * VIRTUAL_CELL_HEIGHT;
        app.update(virtual_width, virtual_height, &font);

        let mut current_buffer = vec![' '; cols * rows];

        let render_list = ui::build_ui(&app, &font, virtual_width, virtual_height);

        for item in render_list {
            match item {
                Renderable::Background { .. } => { /* TUIでは何もしない */ }
                Renderable::BigText { text, anchor, shift, align, font_size, .. } => {
                    draw_art_text(&mut current_buffer, &font, &text, anchor, shift, align, font_size, cols, rows);
                }
                Renderable::Text { text, anchor, shift, align, .. } => {
                    draw_plain_text(&mut current_buffer, &text, anchor, shift, align, cols, rows);
                }
                Renderable::TypingUpper { segments, anchor, shift, align, font_size, .. } => {
                    let target_art_height_in_cells = calculate_target_art_height(font_size, cols, rows);
                    if target_art_height_in_cells == 0 { continue; }
                    let font_size_px = target_art_height_in_cells as f32 * tui_renderer::ART_V_PIXELS_PER_CELL;

                    let arts: Vec<_> = segments.iter().map(|seg| {
                        tui_renderer::render_text_to_art(&font, &seg.base_text, font_size_px)
                    }).collect();
                    let total_width: usize = arts.iter().map(|(_, w, _)| *w).sum();
                    let total_height: usize = arts.first().map(|(_, _, h)| *h).unwrap_or(0);
                    if total_width == 0 { continue; }

                    let anchor_pos = ui::calculate_anchor_position(anchor, shift, cols, rows);
                    let (mut pen_x, start_y) = ui::calculate_aligned_position(anchor_pos, total_width as u32, total_height as u32, align);

                    for (i, (art_buffer, art_width, art_height)) in arts.iter().map(|(b,w,h)|(b,*w,*h)).enumerate() {
                        let seg = &segments[i];

                        blit_art(&mut current_buffer, cols, rows, &art_buffer, art_width, art_height, pen_x as isize, start_y as isize);

                        if let Some(ruby) = &seg.ruby_text {
                            let (ruby_width, _) = measure_plain_text(ruby);
                            let ruby_anchor_pos = (pen_x + (art_width as i32 / 2), start_y);
                            let (ruby_x, ruby_y) = ui::calculate_aligned_position(ruby_anchor_pos, ruby_width, 1, Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Bottom });
                            draw_plain_text_at(&mut current_buffer, ruby, ruby_x, ruby_y, cols);
                        }
                        pen_x += art_width as i32;
                    }
                }
                Renderable::TypingLower { segments, anchor, shift, align, font_size, .. } => {
                    let target_art_height_in_cells = calculate_target_art_height(font_size, cols, rows);
                    if target_art_height_in_cells == 0 { continue; }
                    let font_size_px = target_art_height_in_cells as f32 * tui_renderer::ART_V_PIXELS_PER_CELL;

                    let mut total_width = 0;
                    let mut seg_info = Vec::new();
                    for seg in &segments {
                        match seg {
                            LowerTypingSegment::Completed { base_text, .. } => {
                                let (_, w, _) = tui_renderer::render_text_to_art(&font, base_text, font_size_px);
                                total_width += w;
                                seg_info.push((w, true));
                            }
                            LowerTypingSegment::Active { elements } => {
                                let text = get_active_lower_text(elements);
                                let (w, _) = measure_plain_text(&text);
                                total_width += w as usize;
                                seg_info.push((w as usize, false));
                            }
                        }
                    }
                    let total_height = target_art_height_in_cells;
                    if total_width == 0 { continue; }

                    let anchor_pos = ui::calculate_anchor_position(anchor, shift, cols, rows);
                    let (mut pen_x, start_y) = ui::calculate_aligned_position(anchor_pos, total_width as u32, total_height as u32, align);

                    for (i, seg) in segments.iter().enumerate() {
                        let (seg_width, is_art) = seg_info[i];
                        if is_art {
                            if let LowerTypingSegment::Completed { base_text, ruby_text, .. } = seg {
                                let (art_buffer, art_width, art_height) = tui_renderer::render_text_to_art(&font, base_text, font_size_px);
                                blit_art(&mut current_buffer, cols, rows, &art_buffer, art_width, art_height, pen_x as isize, start_y as isize);
                                if let Some(ruby) = ruby_text {
                                    let (ruby_width, _) = measure_plain_text(ruby);
                                    let ruby_anchor = (pen_x + (art_width as i32 / 2), start_y);
                                    let (rx, ry) = ui::calculate_aligned_position(ruby_anchor, ruby_width, 1, Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Bottom });
                                    draw_plain_text_at(&mut current_buffer, ruby, rx, ry, cols);
                                }
                            }
                        } else {
                            if let LowerTypingSegment::Active { elements } = seg {
                                let text = get_active_lower_text(elements);
                                let plain_y = start_y + (total_height as i32 / 2);
                                draw_plain_text_at(&mut current_buffer, &text, pen_x, plain_y, cols);
                            }
                        }
                        pen_x += seg_width as i32;
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

/// ASCIIアートをバッファに転写する
#[cfg(not(feature = "uefi"))]
fn blit_art(
    buffer: &mut [char], buf_w: usize, buf_h: usize,
    art: &[char], art_w: usize, art_h: usize,
    start_x: isize, start_y: isize,
) {
    if art_w == 0 { return; }
    for y in 0..art_h {
        let target_y = start_y + y as isize;
        if target_y >= 0 && target_y < buf_h as isize {
            for x in 0..art_w {
                let target_x = start_x + x as isize;
                if target_x >= 0 && target_x < buf_w as isize {
                    let art_char = art[y * art_w + x];
                    if art_char != ' ' {
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


/// AA化されたテキストを描画する
#[cfg(not(feature = "uefi"))]
fn draw_art_text(
    buffer: &mut [char], font: &FontRef, text: &str, anchor: Anchor, shift: Shift, align: Align, font_size: FontSize,
    cols: usize, rows: usize,
) {
    let target_art_height_in_cells = calculate_target_art_height(font_size, cols, rows);
    if target_art_height_in_cells == 0 { return; }

    let font_size_px = target_art_height_in_cells as f32 * tui_renderer::ART_V_PIXELS_PER_CELL;
    let (art_buffer, art_width, art_height) = tui_renderer::render_text_to_art(font, text, font_size_px);

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

/// ActiveLowerElementのスライスから表示用の文字列を生成するヘルパー関数
#[cfg(not(feature = "uefi"))]
fn get_active_lower_text(elements: &[ActiveLowerElement]) -> String {
    let mut text = String::new();
    for el in elements {
        match el {
            // FIX: `character` をデリファレンス
            ActiveLowerElement::Typed { character, .. } => text.push(*character),
            ActiveLowerElement::Cursor => text.push('|'),
            ActiveLowerElement::UnconfirmedInput(s) => text.push_str(s),
            // FIX: `c` をデリファレンス
            ActiveLowerElement::LastIncorrectInput(c) => text.push(*c),
        }
    }
    text
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
    }
    
    for (y, (current_row, previous_row)) in current_buffer
        .chunks(width)
        .zip(previous_buffer.chunks(width).chain(std::iter::repeat([].as_slice())))
        .enumerate()
    {
        if current_row != previous_row || previous_buffer.is_empty() {
            let line: String = current_row.iter().collect();
            if y < u16::MAX as usize {
                execute!(stdout, cursor::MoveTo(0, y as u16), Print(line))?;
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
                    _ => {}
                }
            }
        }
    }
    Ok(())
}