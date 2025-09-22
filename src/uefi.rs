// src/uefi.rs

extern crate alloc;

use crate::app::{App, AppEvent};
use crate::renderer::{calculate_pixel_font_size, gui_renderer};
use crate::ui::{self, Renderable};
use ab_glyph::{point, Font, FontRef, OutlinedGlyph, PxScale, ScaleFont};
use alloc::vec::Vec;
use uefi::prelude::*;
use uefi::proto::console::gop::{BltOp, BltPixel, BltRegion, GraphicsOutput};
use uefi::proto::console::text::{Key, ScanCode};

pub fn run() -> Status {
    uefi::helpers::init().unwrap();

    let gop_handle = uefi::boot::get_handle_for_protocol::<GraphicsOutput>().unwrap();
    let mut gop = uefi::boot::open_protocol_exclusive::<GraphicsOutput>(gop_handle).unwrap();

    let mode_info = gop.current_mode_info();
    let (width, height) = mode_info.resolution();

    let font_data: &[u8] = include_bytes!("../fonts/NotoSerifJP-Regular.ttf");
    let font = FontRef::try_from_slice(font_data).expect("Failed to load font");

    let mut app = App::new();
    app.on_event(AppEvent::Start);

    while !app.should_quit {
        uefi::boot::stall(16_000);

        let keys: Vec<Key> = uefi::system::with_stdin(|stdin| {
            let mut collected_keys = Vec::new();
            loop {
                match stdin.read_key() {
                    Ok(Some(key)) => collected_keys.push(key),
                    _ => break,
                }
            }
            collected_keys
        });

        for key in keys {
            match key {
                Key::Printable(c) => {
                    let ch: char = c.into();
                    if ch == '\u{0008}' { app.on_event(AppEvent::Backspace); } 
                    else if ch == '\r' { app.on_event(AppEvent::Enter); } 
                    else if ch != '\u{0000}' { app.on_event(AppEvent::Char(ch)); }
                }
                Key::Special(scan) => match scan {
                    ScanCode::ESCAPE => app.on_event(AppEvent::Escape),
                    ScanCode::UP => app.on_event(AppEvent::Up),
                    ScanCode::DOWN => app.on_event(AppEvent::Down),
                    _ => {}
                },
            }
        }

        let mut pixel_buffer: Vec<BltPixel> = alloc::vec![BltPixel::new(0, 0, 0); width * height];

        let render_list = ui::build_ui(&app, &font, width, height);

        for item in render_list {
            match item {
                Renderable::Background { gradient } => {
                    let start_r = ((gradient.start_color >> 16) & 0xFF) as u8;
                    let start_g = ((gradient.start_color >> 8) & 0xFF) as u8;
                    let start_b = (gradient.start_color & 0xFF) as u8;
                    for pixel in pixel_buffer.iter_mut() {
                        *pixel = BltPixel::new(start_r, start_g, start_b);
                    }
                }
                Renderable::BigText { text, anchor, shift, align, font_size, color }
                | Renderable::Text { text, anchor, shift, align, font_size, color } => {
                    let pixel_font_size = calculate_pixel_font_size(font_size, width, height);
                    let (text_width, text_height, _ascent) = gui_renderer::measure_text(&font, &text, pixel_font_size);
                    let anchor_pos = ui::calculate_anchor_position(anchor, shift, width, height);
                    let (x, y) = ui::calculate_aligned_position(anchor_pos, text_width, text_height, align);
                    draw_text(&mut pixel_buffer, width, &font, &text, (x as f32, y as f32), pixel_font_size, color);
                }
            }
        }

        gop.blt(BltOp::BufferToVideo {
            buffer: &pixel_buffer,
            src: BltRegion::Full,
            dest: (0, 0),
            dims: (width, height),
        }).unwrap();
    }

    Status::SUCCESS
}

fn draw_text(
    buffer: &mut [BltPixel],
    stride: usize,
    font: &FontRef,
    text: &str,
    pos: (f32, f32),
    font_size: f32,
    color: u32,
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
            draw_glyph_to_pixel_buffer(buffer, stride, &outlined, color);
        }
        pen_x += scaled_font.h_advance(glyph_id);
        last_glyph = Some(glyph_id);
    }
}

fn draw_glyph_to_pixel_buffer(
    buffer: &mut [BltPixel],
    stride: usize,
    outlined: &OutlinedGlyph,
    color: u32,
) {
    let bounds = outlined.px_bounds();
    outlined.draw(|x, y, c| {
        let buffer_x = bounds.min.x as i32 + x as i32;
        let buffer_y = bounds.min.y as i32 + y as i32;
        let height = buffer.len() / stride;
        if buffer_x >= 0 && buffer_x < stride as i32 && buffer_y >= 0 && buffer_y < height as i32 {
            let index = (buffer_y as usize) * stride + (buffer_x as usize);
            let text_r = ((color >> 16) & 0xFF) as f32;
            let text_g = ((color >> 8) & 0xFF) as f32;
            let text_b = (color & 0xFF) as f32;
            let bg_r = buffer[index].red as f32;
            let bg_g = buffer[index].green as f32;
            let bg_b = buffer[index].blue as f32;
            let r = (text_r * c + bg_r * (1.0 - c)) as u8;
            let g = (text_g * c + bg_g * (1.0 - c)) as u8;
            let b = (text_b * c + bg_b * (1.0 - c)) as u8;
            buffer[index] = BltPixel::new(r, g, b);
        }
    });
}