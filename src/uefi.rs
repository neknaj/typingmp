// src/uefi.rs

extern crate alloc;

use uefi::prelude::*;
use uefi::proto::console::gop::{BltOp, BltPixel, BltRegion, GraphicsOutput};
use uefi::proto::console::text::{Key, ScanCode};
use uefi::boot::{EventType, TimerTrigger, Tpl};
use crate::app::{App, AppEvent};
use crate::renderer::{TEXT_COLOR, BG_COLOR};
use crate::ui::{self, Renderable};
use ab_glyph::{Font, FontRef, point, OutlinedGlyph, PxScale, ScaleFont};
use alloc::vec::Vec;

const NORMAL_FONT_SIZE: f32 = 25.0;

pub fn run() -> Status {
    uefi::helpers::init().unwrap();

    // Get Graphics Output Protocol
    let gop_handle = uefi::boot::get_handle_for_protocol::<GraphicsOutput>().unwrap();
    let mut gop = uefi::boot::open_protocol_exclusive::<GraphicsOutput>(gop_handle).unwrap();

    // Get current mode info
    let mode_info = gop.current_mode_info();
    let (width, height) = mode_info.resolution();
    let big_font_size = height as f32 * 0.5;

    // Load font
    let font_data: &[u8] = include_bytes!("../fonts/NotoSerifJP-Regular.ttf");
    let font = FontRef::try_from_slice(font_data).expect("Failed to load font");

    let mut app = App::new();

    // Create timer event for the main loop tick
    let timer_event = unsafe { uefi::boot::create_event(EventType::TIMER, Tpl::APPLICATION, None, None).unwrap() };
    uefi::boot::set_timer(&timer_event, TimerTrigger::Relative(100_000)).unwrap(); // 10ms tick

    let mut events = [timer_event];

    while !app.should_quit {
        // Wait for the timer tick
        uefi::boot::wait_for_event(&mut events).unwrap();

        // Read all available keys by returning them from the closure
        let keys: Vec<Key> = uefi::system::with_stdin(|stdin| {
            let mut collected_keys = Vec::new();
            while let Ok(Some(key)) = stdin.read_key() {
                collected_keys.push(key);
            }
            collected_keys
        });

        // Process the collected keys
        for key in keys {
            match key {
                Key::Printable(c) => {
                    let ch: char = c.into();
                    if ch == '\u{0008}' { // Backspace
                        app.on_event(AppEvent::Backspace);
                    } else if ch == '\r' { // Enter
                        app.on_event(AppEvent::Enter);
                    } else {
                        app.on_event(AppEvent::Char(ch));
                    }
                },
                Key::Special(scan) => {
                    match scan {
                        ScanCode::ESCAPE => app.on_event(AppEvent::Escape),
                        ScanCode::UP => app.on_event(AppEvent::Up),
                        ScanCode::DOWN => app.on_event(AppEvent::Down),
                        _ => {},
                    }
                },
            }
        }

        // Render
        let mut pixel_buffer: alloc::vec::Vec<BltPixel> = alloc::vec![BltPixel::new(((BG_COLOR) & 0xFF) as u8, ((BG_COLOR >> 8) & 0xFF) as u8, ((BG_COLOR >> 16) & 0xFF) as u8); width * height];

        let render_list = ui::build_ui(&app);

        for item in render_list {
            match item {
                Renderable::BigText { text, anchor, shift, align } => {
                    let (text_width, text_height, _ascent) = measure_text(&font, text.as_str(), big_font_size);
                    let anchor_pos = ui::calculate_anchor_position(anchor, shift, width, height);
                    let (x, y) = ui::calculate_aligned_position(anchor_pos, text_width, text_height, align);
                    draw_text(
                        &mut pixel_buffer, width, &font, text.as_str(),
                        (x as f32, y as f32), big_font_size,
                    );
                }
                Renderable::Text { text, anchor, shift, align } => {
                    let (text_width, text_height, _ascent) = measure_text(&font, text.as_str(), NORMAL_FONT_SIZE);
                    let anchor_pos = ui::calculate_anchor_position(anchor, shift, width, height);
                    let (x, y) = ui::calculate_aligned_position(anchor_pos, text_width, text_height, align);
                    draw_text(
                        &mut pixel_buffer, width, &font, text.as_str(),
                        (x as f32, y as f32), NORMAL_FONT_SIZE,
                    );
                }
            }
        }

        // Blt to video
        gop.blt(BltOp::BufferToVideo {
            buffer: &pixel_buffer,
            src: BltRegion::Full,
            dest: (0, 0),
            dims: (width, height),
        }).unwrap();

        // Reset timer for the next tick
        uefi::boot::set_timer(&events[0], TimerTrigger::Relative(100_000)).unwrap();
    }

    Status::SUCCESS
}

fn draw_text(
    buffer: &mut [BltPixel], stride: usize, font: &FontRef, text: &str,
    pos: (f32, f32), font_size: f32,
) {
    let scale = PxScale::from(font_size);
    let scaled_font = font.as_scaled(scale);
    let ascent = scaled_font.ascent();
    let mut pen_x = pos.0;
    let pen_y = pos.1 + ascent;

    let mut last_glyph = None;
    for character in text.chars() {
        let glyph_id = font.glyph_id(character);
        if let Some(last) = last_glyph {
            pen_x += scaled_font.kern(last, glyph_id);
        }
        let glyph = glyph_id.with_scale_and_position(scale, point(pen_x, pen_y));
        if let Some(outlined) = font.outline_glyph(glyph) {
            draw_glyph_to_pixel_buffer(buffer, stride, &outlined);
        }
        pen_x += scaled_font.h_advance(glyph_id);
        last_glyph = Some(glyph_id);
    }
}

fn draw_glyph_to_pixel_buffer(buffer: &mut [BltPixel], stride: usize, outlined: &OutlinedGlyph) {
    let bounds = outlined.px_bounds();
    outlined.draw(|x, y, c| {
        let buffer_x = bounds.min.x as i32 + x as i32;
        let buffer_y = bounds.min.y as i32 + y as i32;
        let height = buffer.len() / stride;
        if buffer_x >= 0 && buffer_x < stride as i32 && buffer_y >= 0 && buffer_y < height as i32 {
            let index = (buffer_y as usize) * stride + (buffer_x as usize);
            let text_b = (TEXT_COLOR & 0xFF) as f32;
            let text_g = ((TEXT_COLOR >> 8) & 0xFF) as f32;
            let text_r = ((TEXT_COLOR >> 16) & 0xFF) as f32;
            let bg_b = buffer[index].blue as f32;
            let bg_g = buffer[index].green as f32;
            let bg_r = buffer[index].red as f32;
            let b = (text_b * c + bg_b * (1.0 - c)) as u8;
            let g = (text_g * c + bg_g * (1.0 - c)) as u8;
            let r = (text_r * c + bg_r * (1.0 - c)) as u8;
            buffer[index] = BltPixel::new(b, g, r);
        }
    });
}

/// テキストの描画サイズを計算する
fn measure_text(font: &FontRef, text: &str, size: f32) -> (u32, u32, f32) {
    let scale = PxScale::from(size);
    let scaled_font = font.as_scaled(scale);
    let mut total_width = 0.0;

    let mut last_glyph_id = None;
    for c in text.chars() {
        if c == '\n' {
            continue;
        }
        let glyph = font.glyph_id(c);
        if let Some(last_id) = last_glyph_id {
            total_width += scaled_font.kern(last_id, glyph);
        }
        total_width += scaled_font.h_advance(glyph);
        last_glyph_id = Some(glyph);
    }
    let height = scaled_font.ascent() - scaled_font.descent();
    (total_width as u32, height as u32, scaled_font.ascent())
}
