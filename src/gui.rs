// src/gui.rs

#[cfg(not(feature = "uefi"))] // Only compile if uefi feature is NOT enabled
use crate::app::{App, AppEvent};
#[cfg(not(feature = "uefi"))] // Only compile if uefi feature is NOT enabled
use crate::renderer::{gui_renderer, draw_linear_gradient};
#[cfg(not(feature = "uefi"))] // Only compile if uefi feature is NOT enabled
use crate::ui::{self, Renderable};
#[cfg(not(feature = "uefi"))] // Only compile if uefi feature is NOT enabled
use ab_glyph::FontRef;
#[cfg(not(feature = "uefi"))] // Only compile if uefi feature is NOT enabled
use minifb::{Key, KeyRepeat, Window, WindowOptions};

// --- Windows固有のインポートと処理 ---
// target_osがwindowsの場合のみコンパイルされる
#[cfg(all(target_os = "windows", not(feature = "uefi")))]
use raw_window_handle::{HasWindowHandle, WindowHandle, RawWindowHandle};
#[cfg(all(target_os = "windows", not(feature = "uefi")))]
use winapi::um::dwmapi::DwmSetWindowAttribute;
#[cfg(all(target_os = "windows", not(feature = "uefi")))]
use winapi::shared::windef::HWND;
#[cfg(all(target_os = "windows", not(feature = "uefi")))]
use winapi::shared::minwindef::{BOOL, TRUE};
#[cfg(all(target_os = "windows", not(feature = "uefi")))]
use winapi::ctypes::c_void;

// DWMWAのカスタム属性値 (Windows 11 SDKで導入されたもの)
#[cfg(all(target_os = "windows", not(feature = "uefi")))]
const DWMWA_CAPTION_COLOR: u32 = 35; // タイトルバーの背景色
#[cfg(all(target_os = "windows", not(feature = "uefi")))]
const DWMWA_TEXT_COLOR: u32 = 36;    // タイトルバーのテキスト色
#[cfg(all(target_os = "windows", not(feature = "uefi")))]
const DWMWA_BORDER_COLOR: u32 = 37;  // ウィンドウの境界線の色

// 以前のダークモード設定用 (もし使用したい場合)
// const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;

// RGBからCOLORREF (0x00BBGGRR) に変換するヘルパー関数
#[cfg(all(target_os = "windows", not(feature = "uefi")))]
fn rgb_to_colorref(r: u8, g: u8, b: u8) -> u32 {
    ((b as u32) << 16) | ((g as u32) << 8) | (r as u32)
}

// --- End Windows固有のインポートと処理 ---


/// GUIアプリケーションのメイン関数
#[cfg(not(feature = "uefi"))] // Only compile if uefi feature is NOT enabled
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let font_data = include_bytes!("../fonts/NotoSerifJP-Regular.ttf");
    let font = FontRef::try_from_slice(font_data).map_err(|_| "Failed to load font from slice")?;

    let mut width = 800;
    let mut height = 500;

    let mut window = Window::new(
        "Neknaj Typing Multi-Platform",
        width,
        height,
        WindowOptions {
            resize: true,
            ..WindowOptions::default()
        },
    )?;
    window.set_target_fps(60);

    // --- Windows固有のタイトルバーカスタマイズ ---
    // target_osがwindowsの場合のみこのブロックがコンパイル・実行される
    #[cfg(all(target_os = "windows", not(feature = "uefi")))]
    {
        // カスタム色定義
        // 背景色: 黒
        let caption_color = rgb_to_colorref(0, 0, 0); 
        // テキスト色: 白
        let text_color = rgb_to_colorref(255, 255, 255); 
        // 境界線の色: 濃いグレー
        let border_color = rgb_to_colorref(10, 10, 10); 

        match window.window_handle() {
            Ok(handle_wrapper) => {
                if let RawWindowHandle::Win32(handle) = handle_wrapper.as_raw() {
                    let hwnd = handle.hwnd.get() as HWND; 

                    // DWMWA_CAPTION_COLOR (タイトルバーの背景色) を設定
                    let result_caption = unsafe {
                        DwmSetWindowAttribute(
                            hwnd,
                            DWMWA_CAPTION_COLOR,
                            &caption_color as *const _ as *const c_void,
                            std::mem::size_of_val(&caption_color) as u32,
                        )
                    };
                    if result_caption != 0 {
                        eprintln!("Failed to set DWMWA_CAPTION_COLOR: {}", result_caption);
                    }

                    // DWMWA_TEXT_COLOR (タイトルバーのテキスト色) を設定
                    let result_text = unsafe {
                        DwmSetWindowAttribute(
                            hwnd,
                            DWMWA_TEXT_COLOR,
                            &text_color as *const _ as *const c_void,
                            std::mem::size_of_val(&text_color) as u32,
                        )
                    };
                    if result_text != 0 {
                        eprintln!("Failed to set DWMWA_TEXT_COLOR: {}", result_text);
                    }

                    // DWMWA_BORDER_COLOR (ウィンドウの境界線の色) を設定
                    let result_border = unsafe {
                        DwmSetWindowAttribute(
                            hwnd,
                            DWMWA_BORDER_COLOR,
                            &border_color as *const _ as *const c_void,
                            std::mem::size_of_val(&border_color) as u32,
                        )
                    };
                    if result_border != 0 {
                        eprintln!("Failed to set DWMWA_BORDER_COLOR: {}", result_border);
                    }

                    if result_caption == 0 && result_text == 0 && result_border == 0 {
                        println!("Successfully attempted to set custom colors for Windows title bar.");
                    } else {
                        eprintln!("Some DWM color attributes failed to set. This might occur on Windows 10 or older versions.");
                    }

                } else {
                    eprintln!("Non-Win32 raw window handle obtained, skipping DWM attribute set.");
                }
            },
            Err(e) => {
                eprintln!("Failed to retrieve Windows window handle: {:?}", e);
            }
        }
    }
    // --- Windows固有のカスタマイズ終了 ---

    let mut app = App::new();
    App::on_event(&mut app, AppEvent::Start);

    // メインループ
    while window.is_open() && !app.should_quit {
        let (new_width, new_height) = window.get_size();
        if new_width != width || new_height != height {
            width = new_width;
            height = new_height;
        }

        handle_input(&mut window, &mut app);

        // 1. 背景色でピクセルバッファをクリア
        let mut pixel_buffer = vec![0u32; width * height];

        // 2. UI定義から描画リストを取得
        let render_list = ui::build_ui(&app);

        // 3. 描画リストの各要素を解釈して描画
        for item in render_list {
            match item {
                Renderable::Background { gradient } => {
                    draw_linear_gradient(
                        &mut pixel_buffer, width, height,
                        gradient.start_color, gradient.end_color,
                        (0.0, 0.0), (width as f32, height as f32),
                    );
                }
                Renderable::BigText { text, anchor, shift, align, font_size } => {
                    let pixel_font_size = crate::renderer::calculate_pixel_font_size(font_size, width, height);
                    let (text_width, text_height, _ascent) = gui_renderer::measure_text(&font, &text, pixel_font_size);
                    let anchor_pos = ui::calculate_anchor_position(anchor, shift, width, height);
                    let (x, y) = ui::calculate_aligned_position(anchor_pos, text_width, text_height, align);
                    gui_renderer::draw_text(
                        &mut pixel_buffer, width, &font, &text,
                        (x as f32, y as f32), pixel_font_size,
                    );
                }
                Renderable::Text { text, anchor, shift, align, font_size } => {
                    let pixel_font_size = crate::renderer::calculate_pixel_font_size(font_size, width, height);
                    let (text_width, text_height, _ascent) = gui_renderer::measure_text(&font, &text, pixel_font_size);
                    let anchor_pos = ui::calculate_anchor_position(anchor, shift, width, height);
                    let (x, y) = ui::calculate_aligned_position(anchor_pos, text_width, text_height, align);
                    gui_renderer::draw_text(
                        &mut pixel_buffer, width, &font, &text,
                        (x as f32, y as f32), pixel_font_size,
                    );
                }
            }
        }
        
        // 4. 完成したバッファをウィンドウに表示
        window.update_with_buffer(&pixel_buffer, width, height)?;
    }
    Ok(())
}

/// キーボード入力を処理する
#[cfg(not(feature = "uefi"))] // Only compile if uefi feature is NOT enabled
fn handle_input(window: &mut Window, app: &mut App) {
    // Escapeキーは常に処理する
    if window.is_key_down(Key::Escape) {
        app.on_event(AppEvent::Escape);
    }

    for key in window.get_keys_pressed(KeyRepeat::Yes) {
        match key {
            Key::Up => app.on_event(AppEvent::Up),
            Key::Down => app.on_event(AppEvent::Down),
            Key::Backspace => app.on_event(AppEvent::Backspace),
            Key::Enter => app.on_event(AppEvent::Enter),
            Key::Space => app.on_event(AppEvent::Char(' ')),
            _ => {
                if let Some(char_key) = key_to_char(key, window.is_key_down(Key::LeftShift) || window.is_key_down(Key::RightShift)) {
                    app.on_event(AppEvent::Char(char_key));
                }
            }
        }
    }
}


// キーコードを文字に変換するヘルパー関数 (変更なし)
#[cfg(not(feature = "uefi"))] // Only compile if uefi feature is NOT enabled
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

// Dummy run function for UEFI build
#[cfg(feature = "uefi")]
pub fn run() -> Result<(), Box<dyn core::error::Error>> {
    Err("GUI is not supported in UEFI environment.".into())
}