use crate::app::App;
use crate::renderer::tui_renderer;
use crate::ui::{self, Renderable};
use ab_glyph::{Font, FontRef};
use crossterm::{
    cursor, event, execute,
    event::{Event, KeyCode, KeyEventKind},
    style::Print,
    terminal,
};
use std::io::{stdout, Write};
use std::time::Duration;

/// TUIアプリケーションのメイン関数
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let font_data = include_bytes!("../fonts/NotoSerifJP-Regular.ttf");
    let font = FontRef::try_from_slice(font_data)?;

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
                Renderable::BigText { text, .. } => {
                    let text_layer = tui_renderer::render(&font, text, cols, rows);
                    for (i, ch) in text_layer.iter().enumerate() {
                        if *ch != ' ' {
                            char_buffer[i] = *ch;
                        }
                    }
                }
                Renderable::Text {
                    text,
                    anchor,
                    margin,
                } => {
                    let pos = ui::calculate_position(anchor, margin, cols, rows);
                    let mut x = pos.0;
                    let y = pos.1;
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

/// 文字バッファをターミナルに描画する
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
fn handle_input(app: &mut App) -> std::io::Result<()> {
    if event::poll(Duration::from_millis(100))? {
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('q') => app.should_quit = true,
                    KeyCode::Char(c) => app.on_key(c),
                    KeyCode::Backspace => app.on_backspace(),
                    _ => {}
                }
            }
        }
    }
    // エラーが起きなかった場合に成功(Ok)を返す
    Ok(())
}