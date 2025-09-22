// -----------------------------------------------------------------------------
// TUIバックエンドの実装 (feature = "tui" の時のみコンパイル)
// -----------------------------------------------------------------------------
use crate::app::App;
use crate::renderer::tui_renderer;
use ab_glyph::{Font, FontRef};
use crossterm::{cursor, event, style::Print, terminal, execute, event::{Event, KeyCode, KeyEventKind}};
use std::io::{stdout, Write};
use std::time::Duration;

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let font_data = include_bytes!("../fonts/NotoSerifJP-Regular.ttf");
    let font = FontRef::try_from_slice(font_data)?;

    let mut stdout = stdout();
    terminal::enable_raw_mode()?;
    execute!(stdout, terminal::EnterAlternateScreen, cursor::Hide)?;

    let mut app = App::new();

    while !app.should_quit {
        let (cols, rows) = terminal::size()?;
        
        // 描画
        let char_buffer = tui_renderer::render(&font, &app.input_text, cols as usize, rows as usize);
        draw_buffer_to_terminal(&mut stdout, &char_buffer, cols as usize)?;

        // 入力
        handle_input(&mut app)?;
    }
    
    execute!(stdout, cursor::Show, terminal::LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;
    Ok(())
}

fn draw_buffer_to_terminal(stdout: &mut impl Write, buffer: &[char], width: usize) -> std::io::Result<()> {
    execute!(stdout, cursor::MoveTo(0, 0))?;
    for row in buffer.chunks(width) {
        let line: String = row.iter().collect();
        execute!(stdout, Print(line), cursor::MoveToNextLine(1))?;
    }
    stdout.flush()
}

fn handle_input(app: &mut App) -> std::io::Result<()> {
    if event::poll(Duration::from_millis(100))? {
        if let Event::Key(key) = event::read()? {
            // `Press` イベントの時だけ処理するようにガード節を追加
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
    Ok(())
}