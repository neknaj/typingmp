// src/tui.rs

#[cfg(not(feature = "uefi"))]
use crate::app::{App, AppEvent};
#[cfg(not(feature = "uefi"))]
use crate::renderer::{calculate_pixel_font_size, tui_renderer};
#[cfg(not(feature = "uefi"))]
use crate::ui::{self, Align, Anchor, FontSize, Renderable, Shift};
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

// TUIの1文字の幅と高さを、仮想的なピクセル数で定義
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

    // メインループ
    while !app.should_quit {
        let (cols, rows) = terminal::size()?;
        let (cols, rows) = (cols as usize, rows as usize);

        // スムーズスクロール等の計算のため、仮想的なピクセルサイズを渡す
        let virtual_width = cols * VIRTUAL_CELL_WIDTH;
        let virtual_height = rows * VIRTUAL_CELL_HEIGHT;
        app.update(virtual_width, virtual_height, &font);

        let mut current_buffer = vec![' '; cols * rows];
        
        // 全バックエンド共通のUI構築ロジックを呼び出す
        let render_list = ui::build_ui(&app, &font, virtual_width, virtual_height);

        for item in render_list {
            match item {
                Renderable::Background { .. } => { /* TUIでは何もしない */ }
                // BigTextとTypingBaseをAA化の対象とする
                Renderable::BigText { text, anchor, shift, align, font_size, .. } |
                Renderable::TypingBase { text, anchor, shift, align, font_size, .. } => {
                    draw_art_text(&mut current_buffer, &font, &text, anchor, shift, align, font_size, cols, rows, virtual_width, virtual_height);
                }
                // TextとTypingRubyは通常のテキストとして描画
                Renderable::Text { text, anchor, shift, align, .. } |
                Renderable::TypingRuby { text, anchor, shift, align, .. }=> {
                    draw_plain_text(&mut current_buffer, &text, anchor, shift, align, cols, rows);
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
    // 画面全体のサイズ(width, height)に対する相対座標を計算
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
    cols: usize, rows: usize, virtual_width: usize, virtual_height: usize,
) {
    let font_size_px = calculate_pixel_font_size(font_size, virtual_width, virtual_height);
    let (art_buffer, art_width, art_height) = tui_renderer::render_text_to_art(font, text, font_size_px);

    if art_width == 0 || art_height == 0 {
        return;
    }
    
    // TUIのセル数(cols, rows)に対する相対座標を計算
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