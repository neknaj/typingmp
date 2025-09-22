// src/gui.rs

use crate::app::App;
use crate::renderer::{gui_renderer, BG_COLOR};
use crate::ui::{self, Renderable};
use ab_glyph::FontRef;
use minifb::{Key, KeyRepeat, Window, WindowOptions};

const WIDTH: usize = 800;
const HEIGHT: usize = 300;
const NORMAL_FONT_SIZE: f32 = 16.0;

/// GUIアプリケーションのメイン関数
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let font_data = include_bytes!("../fonts/NotoSerifJP-Regular.ttf");
    let font = FontRef::try_from_slice(font_data).map_err(|_| "Failed to load font from slice")?;

    let mut window = Window::new("GUI Text Input", WIDTH, HEIGHT, WindowOptions::default())?;
    window.set_target_fps(60);
    let mut app = App::new();
    let big_font_size = HEIGHT as f32 * 0.5;

    // メインループ
    while window.is_open() && !app.should_quit {
        handle_input(&mut window, &mut app);

        // 1. 背景色でピクセルバッファをクリア
        let mut pixel_buffer = vec![BG_COLOR; WIDTH * HEIGHT];

        // 2. UI定義から描画リストを取得
        let render_list = ui::build_ui(&app);

        // 3. 描画リストの各要素を解釈して描画
        for item in render_list {
            match item {
                Renderable::BigText { text, anchor, shift, align } => {
                    let (text_width, text_height) = gui_renderer::measure_text(&font, text, big_font_size);
                    let anchor_pos = ui::calculate_anchor_position(anchor, shift, WIDTH, HEIGHT);
                    let (x, y) = ui::calculate_aligned_position(anchor_pos, text_width, text_height, align);
                    gui_renderer::draw_text(
                        &mut pixel_buffer, WIDTH, &font, text,
                        (x as f32, y as f32), big_font_size,
                    );
                }
                Renderable::Text { text, anchor, shift, align } => {
                    let (text_width, text_height) = gui_renderer::measure_text(&font, text, NORMAL_FONT_SIZE);
                    let anchor_pos = ui::calculate_anchor_position(anchor, shift, WIDTH, HEIGHT);
                    let (x, y) = ui::calculate_aligned_position(anchor_pos, text_width, text_height, align);
                    gui_renderer::draw_text(
                        &mut pixel_buffer, WIDTH, &font, text,
                        (x as f32, y as f32), NORMAL_FONT_SIZE,
                    );
                }
            }
        }
        
        // 4. 完成したバッファをウィンドウに表示
        window.update_with_buffer(&pixel_buffer, WIDTH, HEIGHT)?;
    }
    Ok(())
}

/// キーボード入力を処理する
fn handle_input(window: &mut Window, app: &mut App) {
    if window.is_key_down(Key::Escape) {
        app.should_quit = true;
    }
    
    for key in window.get_keys_pressed(KeyRepeat::Yes) {
        match key {
            Key::Backspace => app.on_backspace(),
            Key::Enter => app.on_key('\n'),
            Key::Space => app.on_key(' '),
            _ => {
                if let Some(char_key) = key_to_char(key, window.is_key_down(Key::LeftShift) || window.is_key_down(Key::RightShift)) {
                    app.on_key(char_key);
                }
            }
        }
    }
}

// キーコードを文字に変換するヘルパー関数 (変更なし)
fn key_to_char(key: Key, is_shift: bool) -> Option<char> {
    match (key, is_shift) {
        (Key::A, false) => Some('a'), (Key::A, true) => Some('A'),
        (Key::B, false) => Some('b'), (Key::B, true) => Some('B'),
        (Key::C, false) => Some('c'), (Key::C, true) => Some('C'),
        (Key::D, false) => Some('d'), (Key::D, true) => Some('D'),
        (Key::E, false) => Some('e'), (Key::E, true) => Some('E'),
        (Key::F, false) => Some('f'), (Key::F, true) => Some('F'),
        (Key::G, false) => Some('g'), (Key::G, true) => Some('G'),
        (Key::H, false) => Some('h'), (Key::H, true) => Some('H'),
        (Key::I, false) => Some('i'), (Key::I, true) => Some('I'),
        (Key::J, false) => Some('j'), (Key::J, true) => Some('J'),
        (Key::K, false) => Some('k'), (Key::K, true) => Some('K'),
        (Key::L, false) => Some('l'), (Key::L, true) => Some('L'),
        (Key::M, false) => Some('m'), (Key::M, true) => Some('M'),
        (Key::N, false) => Some('n'), (Key::N, true) => Some('N'),
        (Key::O, false) => Some('o'), (Key::O, true) => Some('O'),
        (Key::P, false) => Some('p'), (Key::P, true) => Some('P'),
        (Key::Q, false) => Some('q'), (Key::Q, true) => Some('Q'),
        (Key::R, false) => Some('r'), (Key::R, true) => Some('R'),
        (Key::S, false) => Some('s'), (Key::S, true) => Some('S'),
        (Key::T, false) => Some('t'), (Key::T, true) => Some('T'),
        (Key::U, false) => Some('u'), (Key::U, true) => Some('U'),
        (Key::V, false) => Some('v'), (Key::V, true) => Some('V'),
        (Key::W, false) => Some('w'), (Key::W, true) => Some('W'),
        (Key::X, false) => Some('x'), (Key::X, true) => Some('X'),
        (Key::Y, false) => Some('y'), (Key::Y, true) => Some('Y'),
        (Key::Z, false) => Some('z'), (Key::Z, true) => Some('Z'),
        (Key::Key0, false) => Some('0'), (Key::Key0, true) => Some(')'),
        (Key::Key1, false) => Some('1'), (Key::Key1, true) => Some('!'),
        (Key::Key2, false) => Some('2'), (Key::Key2, true) => Some('@'),
        (Key::Key3, false) => Some('3'), (Key::Key3, true) => Some('#'),
        (Key::Key4, false) => Some('4'), (Key::Key4, true) => Some('$'),
        (Key::Key5, false) => Some('5'), (Key::Key5, true) => Some('%'),
        (Key::Key6, false) => Some('6'), (Key::Key6, true) => Some('^'),
        (Key::Key7, false) => Some('7'), (Key::Key7, true) => Some('&'),
        (Key::Key8, false) => Some('8'), (Key::Key8, true) => Some('*'),
        (Key::Key9, false) => Some('9'), (Key::Key9, true) => Some('('),
        (Key::Comma, false) => Some(','), (Key::Comma, true) => Some('<'),
        (Key::Period, false) => Some('.'), (Key::Period, true) => Some('>'),
        (Key::Slash, false) => Some('/'), (Key::Slash, true) => Some('?'),
        (Key::Semicolon, false) => Some(';'), (Key::Semicolon, true) => Some(':'),
        (Key::Equal, false) => Some('='), (Key::Equal, true) => Some('+'),
        (Key::Minus, false) => Some('-'), (Key::Minus, true) => Some('_'),
        _ => None,
    }
}