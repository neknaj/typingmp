// src/tui.rs

#[cfg(feature = "uefi")]
extern crate alloc;

use crate::app::{App, AppEvent};
use crate::renderer::tui_renderer;
use crate::ui::{self, Renderable};
use ab_glyph::FontRef;
use crossterm::{
    cursor, event, execute,
    event::{Event, KeyCode, KeyEventKind},
    style::Print,
    terminal,
};

#[cfg(not(feature = "uefi"))]
use std::io::{stdout, Write};
#[cfg(feature = "uefi")]
use alloc::string::String;
#[cfg(feature = "uefi")]
use alloc::vec::Vec;
#[cfg(feature = "uefi")]
use core::fmt::Write; // For stdout.flush()
#[cfg(not(feature = "uefi"))]
use std::time::Duration;
#[cfg(feature = "uefi")]
use uefi::boot::TimerTrigger; // For Duration equivalent

/// TUIアプリケーションのメイン関数
#[cfg(not(feature = "uefi"))]
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let font_data = include_bytes!("../fonts/NotoSerifJP-Regular.ttf");
    let font = FontRef::try_from_slice(font_data).map_err(|_| "Failed to load font from slice")?;

    let mut stdout = stdout();
    terminal::enable_raw_mode()?;
    execute!(stdout, terminal::EnterAlternateScreen, cursor::Hide)?;
    let mut app = App::new();

    // メインループ
    while !app.should_quit {
        let (cols, rows) = terminal::size()?;
        let (cols, rows) = (cols as usize, rows as usize);

        let mut char_buffer = vec![' '; cols * rows];
        let render_list = ui::build_ui(&app);

        for item in render_list {
            match item {
                Renderable::BigText { text, font_size, .. } => {
                    let text_layer = tui_renderer::render(&font, &text, cols, rows, font_size);
                    for (i, ch) in text_layer.iter().enumerate() {
                        if *ch != ' ' {
                            char_buffer[i] = *ch;
                        }
                    }
                }
                Renderable::Text {
                    text,
                    anchor,
                    shift,
                    align,
                    font_size,
                } => {
                    // TUIでは複雑なアライメントは実装が難しいため、単純な位置計算に留める
                    // font_sizeはTUIのmeasure_textでは直接利用されないが、将来的な拡張のために抽出しておく
                    let (text_width, text_height) = tui_renderer::measure_text(&text);
                    let anchor_pos = ui::calculate_anchor_position(anchor, shift, cols, rows);
                    let (mut x, y) = ui::calculate_aligned_position(anchor_pos, text_width, text_height, align);

                    if y < 0 || y >= rows as i32 {
                        continue;
                    }
                    for c in text.chars() {
                        if x >= 0 && x < cols as i32 {
                            char_buffer[(y as usize) * cols + (x as usize)] = c;
                        }
                        x += 1;
                    }
                }
            }
        }
        draw_buffer_to_terminal(&mut stdout, &char_buffer, cols)?;
        handle_input(&mut app)?;
    }

    execute!(stdout, cursor::Show, terminal::LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;
    Ok(())
}

/// TUIアプリケーションのメイン関数 (UEFI版)
#[cfg(feature = "uefi")]
pub fn run() -> Result<(), Box<dyn core::error::Error>> {
    // UEFI環境ではTUIはサポートしないか、別の実装が必要
    // ここでは単にエラーを返すか、何もしない
    // TODO: UEFI環境でのTUI実装を検討
    Err("TUI is not supported in UEFI environment yet.".into())
}


/// 文字バッファをターミナルに描画する
#[cfg(not(feature = "uefi"))]
fn draw_buffer_to_terminal(
    stdout: &mut impl Write,
    buffer: &[char],
    width: usize,
) -> std::io::Result<()> {
    execute!(stdout, cursor::MoveTo(0, 0))?;
    for row in buffer.chunks(width) {
        let line: String = row.iter().collect();
        execute!(stdout, Print(line), cursor::MoveToNextLine(1))?;
    }
    // stdout.flush()がResultを返すため、これがこの関数の戻り値になる
    stdout.flush()
}

/// キーボード入力を処理する
#[cfg(not(feature = "uefi"))]
fn handle_input(app: &mut App) -> std::io::Result<()> {
    if event::poll(Duration::from_millis(100))? {
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char(c) => app.on_event(AppEvent::Char(c)),
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
    // エラーが起きなかった場合に成功(Ok)を返す
    Ok(())
}

// UEFI環境ではTUIのhandle_inputは不要
#[cfg(feature = "uefi")]
fn handle_input(_app: &mut App) -> Result<(), Box<dyn core::error::Error>> {
    Ok(())
}

// UEFI環境ではdraw_buffer_to_terminalは不要
#[cfg(feature = "uefi")]
fn draw_buffer_to_terminal(
    _stdout: &mut impl Write,
    _buffer: &[char],
    _width: usize,
) -> Result<(), Box<dyn core::error::Error>> {
    Ok(())
}
