// src/uefi.rs

extern crate alloc;

use crate::app::{App, AppEvent};
use crate::renderer::{calculate_pixel_font_size, gui_renderer, BG_COLOR};
use crate::ui::{self, ActiveLowerElement, LowerTypingSegment, Renderable, UpperSegmentState};
use ab_glyph::{point, Font, FontRef, OutlinedGlyph, PxScale, ScaleFont};
use alloc::vec::Vec;
use uefi::boot::{EventType, TimerTrigger, Tpl};
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
    App::on_event(&mut app, AppEvent::Start);

    let timer_event =
        unsafe { uefi::boot::create_event(EventType::TIMER, Tpl::APPLICATION, None, None).unwrap() };
    uefi::boot::set_timer(&timer_event, TimerTrigger::Relative(100_000)).unwrap();

    let mut events = [timer_event];

    while !app.should_quit {
        uefi::boot::wait_for_event(&mut events).unwrap();

        let keys: Vec<Key> = uefi::system::with_stdin(|stdin| {
            let mut collected_keys = Vec::new();
            while let Ok(Some(key)) = stdin.read_key() {
                collected_keys.push(key);
            }
            collected_keys
        });

        for key in keys {
            match key {
                Key::Printable(c) => {
                    let ch: char = c.into();
                    if ch == '\u{0008}' {
                        app.on_event(AppEvent::Backspace);
                    } else if ch == '\r' {
                        app.on_event(AppEvent::Enter);
                    } else {
                        app.on_event(AppEvent::Char {
                            c: ch,
                            timestamp: 0.0, // UEFIでは正確なタイムスタンプは無いため0.0
                        });
                    }
                }
                Key::Special(scan) => match scan {
                    ScanCode::ESCAPE => app.on_event(AppEvent::Escape),
                    ScanCode::UP => app.on_event(AppEvent::Up),
                    ScanCode::DOWN => app.on_event(AppEvent::Down),
                    _ => {}
                },
            }
        }

        app.update(width, height, &font);

        let mut pixel_buffer: alloc::vec::Vec<BltPixel> = alloc::vec![
            BltPixel::new(
                ((BG_COLOR >> 16) & 0xFF) as u8,
                ((BG_COLOR >> 8) & 0xFF) as u8,
                ((BG_COLOR) & 0xFF) as u8
            );
            width * height
        ];

        let render_list = ui::build_ui(&app, &font, width, height);

        for item in render_list {
            match item {
                Renderable::BigText { text, anchor, shift, align, font_size, color } |
                Renderable::Text { text, anchor, shift, align, font_size, color } => {
                    let pixel_font_size = calculate_pixel_font_size(font_size, width, height);
                    let (text_width, text_height, _ascent) =
                        gui_renderer::measure_text(&font, text.as_str(), pixel_font_size);
                    let anchor_pos = ui::calculate_anchor_position(anchor, shift, width, height);
                    let (x, y) = ui::calculate_aligned_position(anchor_pos, text_width, text_height, align);
                    draw_text(
                        &mut pixel_buffer,
                        width,
                        &font,
                        text.as_str(),
                        (x as f32, y as f32),
                        pixel_font_size,
                        color,
                    );
                }
                Renderable::Background { gradient } => {
                    let mut temp_buffer: Vec<u32> = Vec::with_capacity(width * height);
                    for pixel in pixel_buffer.iter() {
                        temp_buffer.push(
                            (pixel.red as u32) << 16
                                | (pixel.green as u32) << 8
                                | (pixel.blue as u32),
                        );
                    }
                    crate::renderer::draw_linear_gradient(
                        &mut temp_buffer,
                        width, height,
                        gradient.start_color, gradient.end_color,
                        (0.0, 0.0), (width as f32, height as f32),
                    );
                    for (i, color) in temp_buffer.iter().enumerate() {
                        pixel_buffer[i] = BltPixel::new(
                            ((color >> 16) & 0xFF) as u8,
                            ((color >> 8) & 0xFF) as u8,
                            (color & 0xFF) as u8,
                        );
                    }
                }
                Renderable::TypingUpper { segments, anchor, shift, align, font_size } => {
                    let pixel_font_size = calculate_pixel_font_size(font_size, width, height);
                    let ruby_pixel_font_size = pixel_font_size * 0.4;
                    
                    let total_width = segments.iter().map(|seg| {
                        gui_renderer::measure_text(&font, seg.base_text.as_str(), pixel_font_size).0
                    }).sum::<u32>();
                    let total_height = gui_renderer::measure_text(&font, " ", pixel_font_size).1;

                    let anchor_pos = ui::calculate_anchor_position(anchor, shift, width, height);
                    let (mut pen_x, y) = ui::calculate_aligned_position(anchor_pos, total_width, total_height, align);

                    for seg in segments {
                        let color = match seg.state {
                            UpperSegmentState::Correct => ui::CORRECT_COLOR,
                            UpperSegmentState::Incorrect => ui::INCORRECT_COLOR,
                            UpperSegmentState::Active => ui::ACTIVE_COLOR,
                            UpperSegmentState::Pending => ui::PENDING_COLOR,
                        };
                        draw_text(&mut pixel_buffer, width, &font, seg.base_text.as_str(), (pen_x as f32, y as f32), pixel_font_size, color);
                        
                        if let Some(ruby) = &seg.ruby_text {
                            let (base_w, ..) = gui_renderer::measure_text(&font, seg.base_text.as_str(), pixel_font_size);
                            let (ruby_w, ..) = gui_renderer::measure_text(&font, ruby.as_str(), ruby_pixel_font_size);
                            let ruby_x = pen_x as f32 + (base_w as f32 - ruby_w as f32) / 2.0;
                            let ruby_y = y as f32 - ruby_pixel_font_size*0.5;
                            draw_text(&mut pixel_buffer, width, &font, ruby.as_str(), (ruby_x, ruby_y), ruby_pixel_font_size, color);
                        }
                        
                        let (seg_width, _, _) = gui_renderer::measure_text(&font, seg.base_text.as_str(), pixel_font_size);
                        pen_x += seg_width as i32;
                    }
                }
                Renderable::TypingLower { segments, anchor, shift, align, font_size, target_line_total_width } => {
                    let pixel_font_size = calculate_pixel_font_size(font_size, width, height);
                    let ruby_pixel_font_size = pixel_font_size * 0.3;
                    let total_height = gui_renderer::measure_text(&font, " ", pixel_font_size).1;

                    let anchor_pos = ui::calculate_anchor_position(anchor, shift, width, height);
                    let (mut pen_x, y) = ui::calculate_aligned_position(anchor_pos, target_line_total_width, total_height, align);

                    for seg in segments {
                        match seg {
                            LowerTypingSegment::Completed { base_text, ruby_text, is_correct } => {
                                let color = if is_correct { ui::CORRECT_COLOR } else { ui::INCORRECT_COLOR };
                                draw_text(&mut pixel_buffer, width, &font, base_text.as_str(), (pen_x as f32, y as f32), pixel_font_size, color);
                                
                                if let Some(ruby) = ruby_text {
                                    let (base_w, ..) = gui_renderer::measure_text(&font, base_text.as_str(), pixel_font_size);
                                    let (ruby_w, ..) = gui_renderer::measure_text(&font, ruby.as_str(), ruby_pixel_font_size);
                                    let ruby_x = pen_x as f32 + (base_w as f32 - ruby_w as f32) / 2.0;
                                    let ruby_y = y as f32 - ruby_pixel_font_size*0.5;
                                    draw_text(&mut pixel_buffer, width, &font, ruby.as_str(), (ruby_x, ruby_y), ruby_pixel_font_size, color);
                                }

                                pen_x += gui_renderer::measure_text(&font, base_text.as_str(), pixel_font_size).0 as i32;
                            }
                            LowerTypingSegment::Active { elements } => {
                                for el in elements {
                                    let (text, color) = match el {
                                        // FIX: `to_string(character)` を `to_string(&character)` に変更
                                        ActiveLowerElement::Typed { character, is_correct } => (alloc::string::ToString::to_string(&character), if is_correct { ui::CORRECT_COLOR } else { ui::INCORRECT_COLOR }),
                                        ActiveLowerElement::Cursor => (alloc::string::ToString::to_string(&'|'), ui::CURSOR_COLOR),
                                        ActiveLowerElement::UnconfirmedInput(s) => (s.clone(), ui::UNCONFIRMED_COLOR),
                                        // FIX: `to_string(c)` を `to_string(&c)` に変更
                                        ActiveLowerElement::LastIncorrectInput(c) => (alloc::string::ToString::to_string(&c), ui::WRONG_KEY_COLOR),
                                    };
                                    draw_text(&mut pixel_buffer, width, &font, text.as_str(), (pen_x as f32, y as f32), pixel_font_size, color);
                                    pen_x += gui_renderer::measure_text(&font, text.as_str(), pixel_font_size).0 as i32;
                                }
                            }
                        }
                    }
                }
            }
        }

        gop.blt(BltOp::BufferToVideo {
            buffer: &pixel_buffer,
            src: BltRegion::Full,
            dest: (0, 0),
            dims: (width, height),
        })
        .unwrap();

        uefi::boot::set_timer(&events[0], TimerTrigger::Relative(100_000)).unwrap();
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
            let text_b = (color & 0xFF) as f32;
            let text_g = ((color >> 8) & 0xFF) as f32;
            let text_r = ((color >> 16) & 0xFF) as f32;
            let bg_b = buffer[index].blue as f32;
            let bg_g = buffer[index].green as f32;
            let bg_r = buffer[index].red as f32;
            let b = (text_b * c + bg_b * (1.0 - c)) as u8;
            let g = (text_g * c + bg_g * (1.0 - c)) as u8;
            let r = (text_r * c + bg_r * (1.0 - c)) as u8;
            buffer[index] = BltPixel::new(r, g, b);
        }
    });
}