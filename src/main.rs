// -----------------------------------------------------------------------------
// モジュール：app - アプリケーションの共通状態とロジック
// -----------------------------------------------------------------------------
mod app {
    pub struct App {
        pub input_text: String,
        pub should_quit: bool,
    }

    impl App {
        pub fn new() -> Self {
            Self {
                input_text: "Hello, World!".to_string(),
                should_quit: false,
            }
        }

        pub fn on_key(&mut self, c: char) {
            self.input_text.push(c);
        }

        pub fn on_backspace(&mut self) {
            self.input_text.pop();
        }
    }
}


// -----------------------------------------------------------------------------
// モジュール：renderer - ab_glyphを使った共通レンダリングロジック
// -----------------------------------------------------------------------------
mod renderer {
    use ab_glyph::{point, Font, FontRef, OutlinedGlyph};

    /// GUIバックエンド用のレンダラ
    pub mod gui_renderer {
        use super::*;
        const TEXT_COLOR: u32 = 0x00_FFFFFF;
        const BG_COLOR: u32 = 0x00_101010;
        
        pub fn render(font: &FontRef, text: &str, width: usize, height: usize) -> Vec<u32> {
            let mut buffer = vec![BG_COLOR; width * height];

            let font_size = 48.0;
            let scale = font_size / (font.ascent_unscaled() - font.descent_unscaled());
            let mut pen_x = 15.0;
            let pen_y = 60.0;

            for character in text.chars() {
                let glyph = font.glyph_id(character).with_scale_and_position(font_size, point(pen_x, pen_y));
                if let Some(outlined) = font.outline_glyph(glyph) {
                    draw_glyph_to_pixel_buffer(&mut buffer, width, &outlined);
                }
                pen_x += font.h_advance_unscaled(font.glyph_id(character)) * scale;
            }
            buffer
        }
        
        fn draw_glyph_to_pixel_buffer(buffer: &mut Vec<u32>, width: usize, outlined: &OutlinedGlyph) {
            let bounds = outlined.px_bounds();
            outlined.draw(|x, y, c| {
                let buffer_x = bounds.min.x as i32 + x as i32;
                let buffer_y = bounds.min.y as i32 + y as i32;
                let height = buffer.len() / width;

                if buffer_x >= 0 && buffer_x < width as i32 && buffer_y >= 0 && buffer_y < height as i32 {
                    let index = (buffer_y as usize) * width + (buffer_x as usize);
                    let text_r = ((TEXT_COLOR >> 16) & 0xFF) as f32;
                    let text_g = ((TEXT_COLOR >> 8) & 0xFF) as f32;
                    let text_b = (TEXT_COLOR & 0xFF) as f32;
                    let bg_r = ((BG_COLOR >> 16) & 0xFF) as f32;
                    let bg_g = ((BG_COLOR >> 8) & 0xFF) as f32;
                    let bg_b = (BG_COLOR & 0xFF) as f32;
                    let r = (text_r * c + bg_r * (1.0 - c)) as u32;
                    let g = (text_g * c + bg_g * (1.0 - c)) as u32;
                    let b = (text_b * c + bg_b * (1.0 - c)) as u32;
                    buffer[index] = (r << 16) | (g << 8) | b;
                }
            });
        }
    }

    /// TUIバックエンド用のレンダラ
    pub mod tui_renderer {
        use super::*;
        
        pub fn render(font: &FontRef, text: &str, width: usize, height: usize) -> Vec<char> {
            let mut buffer = vec![' '; width * height];
            
            // ターミナルの高さに合わせてフォントサイズを動的に調整
            let font_size = height as f32 * 0.8;
            let scale = font_size / (font.ascent_unscaled() - font.descent_unscaled());
            let mut pen_x = 2.0;
            let pen_y = height as f32 * 0.7;

            for character in text.chars() {
                let glyph = font.glyph_id(character).with_scale_and_position(font_size, point(pen_x, pen_y));
                if let Some(outlined) = font.outline_glyph(glyph) {
                    draw_glyph_to_char_buffer(&mut buffer, width, &outlined);
                }
                pen_x += font.h_advance_unscaled(font.glyph_id(character)) * scale;
            }
            buffer
        }
        
        fn draw_glyph_to_char_buffer(buffer: &mut Vec<char>, width: usize, outlined: &OutlinedGlyph) {
            let bounds = outlined.px_bounds();
            outlined.draw(|x, y, c| {
                let buffer_x = bounds.min.x as usize + x as usize;
                let buffer_y = bounds.min.y as usize + y as usize;
                let height = buffer.len() / width;
                if buffer_x < width && buffer_y < height {
                    let index = buffer_y * width + buffer_x;
                    let coverage_char = match (c * 4.0).round() as u8 {
                        0 => ' ', 1 => '.', 2 => '*', 3 => '#', _ => '@',
                    };
                    if buffer[index] == ' ' { buffer[index] = coverage_char; }
                }
            });
        }
    }
}


// -----------------------------------------------------------------------------
// GUIバックエンドの実装 (feature = "gui" の時のみコンパイル)
// -----------------------------------------------------------------------------
#[cfg(feature = "gui")]
mod gui {
    use crate::app::App;
    use crate::renderer::gui_renderer;
    use ab_glyph::{Font, FontRef};
    use minifb::{Key, KeyRepeat, Window, WindowOptions};

    const WIDTH: usize = 800;
    const HEIGHT: usize = 100;

    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        let font_data = include_bytes!("../fonts/NotoSerifJP-Regular.ttf");
        let font = FontRef::try_from_slice(font_data)?;

        let mut window = Window::new("GUI Text Input", WIDTH, HEIGHT, WindowOptions::default())?;
        window.limit_update_rate(Some(std::time::Duration::from_micros(16600)));

        let mut app = App::new();

        while window.is_open() && !app.should_quit {
            handle_input(&window, &mut app);

            let pixel_buffer = gui_renderer::render(&font, &app.input_text, WIDTH, HEIGHT);
            window.update_with_buffer(&pixel_buffer, WIDTH, HEIGHT)?;
        }
        Ok(())
    }

    fn handle_input(window: &Window, app: &mut App) {
        if window.is_key_down(Key::Escape) {
            app.should_quit = true;
        }

        let keys_pressed = window.get_keys_pressed(KeyRepeat::Yes);
        let is_shift = window.is_key_down(Key::LeftShift) || window.is_key_down(Key::RightShift);
        
        for key in keys_pressed {
            match key {
                Key::Backspace => app.on_backspace(),
                Key::Space => app.input_text.push(' '),
                k if (Key::A as usize <= k as usize) && (k as usize <= Key::Z as usize) => {
                    let mut c = (((k as u8) - (Key::A as u8)) + b'a') as char;
                    if is_shift { c = c.to_ascii_uppercase(); }
                    app.on_key(c);
                }
                k if (Key::Key0 as usize <= k as usize) && (k as usize <= Key::Key9 as usize) => {
                    let c = (((k as u8) - (Key::Key0 as u8)) + b'0') as char;
                    app.on_key(c);
                }
                _ => {}
            }
        }
    }
}


// -----------------------------------------------------------------------------
// TUIバックエンドの実装 (feature = "tui" の時のみコンパイル)
// -----------------------------------------------------------------------------
#[cfg(feature = "tui")]
mod tui {
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
}


// -----------------------------------------------------------------------------
// main関数 - featureフラグに応じて各バックエンドを起動
// -----------------------------------------------------------------------------
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // `gui` featureが有効な場合に、こちらのブロックがコンパイルされる
    #[cfg(feature = "gui")]
    {
        // もし`tui`も同時に有効になっていたら、警告を出す
        #[cfg(feature = "tui")]
        {
            println!("Warning: Both 'gui' and 'tui' features are enabled.");
            println!("Prioritizing GUI backend. To run the TUI version, use:");
            println!("cargo run --no-default-features --features tui");
        }
        println!("Starting GUI version... (Close the window or press ESC to exit)");
        // gui::run()を実行して終了
        return gui::run();
    }

    // `gui` featureが無効、かつ `tui` featureが有効な場合にのみ、こちらのブロックがコンパイルされる
    #[cfg(all(not(feature = "gui"), feature = "tui"))]
    {
        println!("Starting TUI version... (Press 'q' to exit)");
        // TUIモードに入る前に少し待機してメッセージを読めるようにする
        std::thread::sleep(std::time::Duration::from_secs(2));
        // tui::run()を実行して終了
        return tui::run();
    }

    // `gui`も`tui`もどちらも有効でない場合に、こちらのブロックがコンパイルされる
    #[cfg(not(any(feature = "gui", feature = "tui")))]
    {
        println!("No backend feature enabled. Please run with --features gui or --features tui");
        return Ok(());
    }
}