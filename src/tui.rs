// src/tui.rs

#[cfg(not(feature = "uefi"))]
use crate::app::{App, AppEvent};
#[cfg(not(feature = "uefi"))]
use crate::renderer::{tui_renderer};
#[cfg(not(feature = "uefi"))]
use crate::ui::{self, ActiveLowerElement, Align, Anchor, FontSize, LowerTypingSegment, Renderable, Shift};
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
                    let full_text: String = segments.iter().map(|s| s.base_text.as_str()).collect();
                    draw_art_text(&mut current_buffer, &font, &full_text, anchor, shift, align, font_size, cols, rows);
                }
                Renderable::TypingLower { segments, anchor, shift, align, .. } => {
                    let mut full_text = String::new();
                    for seg in segments {
                        match seg {
                            // FIX: `base_text` を `&base_text` として借用
                            LowerTypingSegment::Completed { base_text, .. } => full_text.push_str(&base_text),
                            LowerTypingSegment::Active { elements } => {
                                for el in elements {
                                    match el {
                                        // FIX: `*character` を `character` に変更
                                        ActiveLowerElement::Typed { character, .. } => full_text.push(character),
                                        ActiveLowerElement::Cursor => full_text.push('|'),
                                        // FIX: `s` を `&s` として借用
                                        ActiveLowerElement::UnconfirmedInput(s) => full_text.push_str(&s),
                                        // FIX: `*c` を `c` に変更
                                        ActiveLowerElement::LastIncorrectInput(c) => full_text.push(c),
                                    }
                                }
                            }
                        }
                    }
                    draw_plain_text(&mut current_buffer, &full_text, anchor, shift, align, cols, rows);
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

    if start_y >= 0 && start_y < height as i32 {
        for (i, c) in text.chars().enumerate() {
            let current_x = start_x + i as i32;
            if current_x >= 0 && current_x < width as i32 {
                let index = (start_y as usize * width) + current_x as usize;
                if index < buffer.len() {
                    buffer[index] = c;
                }
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
    let target_art_height_in_cells = match font_size {
        FontSize::WindowHeight(ratio) => (rows as f32 * ratio).ceil() as usize,
        FontSize::WindowAreaSqrt(ratio) => {
            let base_dimension = (cols as f32 * rows as f32).sqrt();
            (base_dimension * ratio).ceil() as usize
        }
    };

    if target_art_height_in_cells == 0 { return; }

    let font_size_px = target_art_height_in_cells as f32 * tui_renderer::ART_V_PIXELS_PER_CELL;
    let (art_buffer, art_width, art_height) = tui_renderer::render_text_to_art(font, text, font_size_px);

    if art_width == 0 || art_height == 0 { return; }
    
    let anchor_pos = ui::calculate_anchor_position(anchor, shift, cols, rows);
    let (start_x, start_y) = ui::calculate_aligned_position(anchor_pos, art_width as u32, art_height as u32, align);
    
    blit_art(buffer, cols, rows, &art_buffer, art_width, art_height, start_x as isize, start_y as isize);
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