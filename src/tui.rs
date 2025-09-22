// src/tui.rs

#[cfg(feature = "uefi")]
extern crate alloc;

use crate::app::{App, AppEvent};
use crate::renderer::tui_renderer;
use crate::ui::{self, Renderable};
use ab_glyph::FontRef;
use crossterm::{
    cursor, event,
    event::{Event, KeyCode, KeyEventKind},
    execute,
    style::Print,
    terminal,
};

#[cfg(feature = "uefi")]
use alloc::string::String;
#[cfg(feature = "uefi")]
use alloc::vec::Vec;
#[cfg(feature = "uefi")]
use core::fmt::Write;
#[cfg(not(feature = "uefi"))]
use std::io::{stdout, Write};
#[cfg(not(feature = "uefi"))]
use std::time::Duration;

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

    // メインループ
    while !app.should_quit {
        let (cols, rows) = terminal::size()?;
        let (cols, rows) = (cols as usize, rows as usize);

        let mut char_buffer = vec![' '; cols * rows];
        
        // 全ての状態で共通のUI構築と描画ロジックを使用する
        let render_list = ui::build_ui(&app, &font, cols, rows);

        for item in render_list {
            match item {
                Renderable::Background { .. } => {
                    // TUIでは背景グラデーションは描画しない
                }
                Renderable::Text { text, anchor, shift, align, .. } |
                Renderable::BigText { text, anchor, shift, align, .. } => {
                    // 1. TUIグリッド上でのテキストサイズを計算
                    let (text_width, text_height) = tui_renderer::measure_text(&text);
                    
                    // 2. アンカー位置を計算
                    let anchor_pos = ui::calculate_anchor_position(anchor, shift, cols, rows);

                    // 3. 揃えを考慮した最終的な描画開始位置(左上)を計算
                    let (start_x, start_y) = ui::calculate_aligned_position(anchor_pos, text_width, text_height, align);

                    // 4. バッファに文字を描画（範囲チェック付き）
                    if start_y >= 0 && start_y < rows as i32 {
                        for (i, c) in text.chars().enumerate() {
                            let current_x = start_x + i as i32;
                            if current_x >= 0 && current_x < cols as i32 {
                                let index = (start_y as usize * cols) + current_x as usize;
                                if index < char_buffer.len() {
                                    char_buffer[index] = c;
                                }
                            }
                        }
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
    Ok(())
}