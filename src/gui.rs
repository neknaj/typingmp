#[cfg(not(feature = "uefi"))]
use crate::app::{App, AppEvent, Fonts};
#[cfg(not(feature = "uefi"))]
use crate::backend::BackendError;
#[cfg(all(not(feature = "uefi"), feature = "gui-file"))]
use crate::io::DesktopProblemSourceProvider;
#[cfg(not(feature = "uefi"))]
use crate::io::{AssetProvider, BundledFont, DesktopAssetProvider};
#[cfg(not(feature = "uefi"))]
use crate::renderer::{calculate_pixel_font_size, draw_linear_gradient, gui_renderer};
#[cfg(not(feature = "uefi"))]
use crate::ui::{self, ActiveLowerElement, LowerTypingSegment, Renderable, UpperSegmentState};
#[cfg(not(feature = "uefi"))]
use ab_glyph::FontVec;
#[cfg(not(feature = "uefi"))]
use pixels::{Pixels, SurfaceTexture};
#[cfg(not(feature = "uefi"))]
use std::error::Error;
#[cfg(not(feature = "uefi"))]
use std::time::{Duration, Instant};

#[cfg(feature = "gui-file")]
use rfd::FileDialog;

#[cfg(not(feature = "uefi"))]
use winit::{
    event::{
        ElementState, Event, KeyboardInput, MouseButton, MouseScrollDelta, VirtualKeyCode,
        WindowEvent,
    },
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

#[cfg(not(feature = "uefi"))]
const FRAME_DURATION: Duration = Duration::from_millis(16);

#[cfg(not(feature = "uefi"))]
pub fn run() -> Result<(), Box<dyn Error>> {
    let asset_provider = DesktopAssetProvider::discover();
    let japanese_font =
        FontVec::try_from_vec(asset_provider.load_bundled_font(BundledFont::YujiSyukuRegular)?)
            .map_err(|_| BackendError::asset("failed to parse Yuji Syuku font"))?;
    let traditional_chinese_font =
        FontVec::try_from_vec(asset_provider.load_bundled_font(BundledFont::NotoSerifJpRegular)?)
            .map_err(|_| BackendError::asset("failed to parse Noto Serif JP font"))?;
    let simplified_chinese_font =
        FontVec::try_from_vec(asset_provider.load_bundled_font(BundledFont::NotoSerifJpRegular)?)
            .map_err(|_| BackendError::asset("failed to parse Noto Serif JP font"))?;

    let fonts = Fonts {
        japanese: japanese_font,
        traditional_chinese: Some(traditional_chinese_font),
        simplified_chinese: Some(simplified_chinese_font),
    };

    let event_loop = EventLoop::new();
    let mut window = WindowBuilder::new()
        .with_title("Neknaj Typing Multi-Platform")
        .with_inner_size(winit::dpi::LogicalSize::new(800.0, 500.0))
        .build(&event_loop)?;

    let size = window.inner_size();
    let mut width = size.width as usize;
    let mut height = size.height as usize;
    let mut pixel_buffer = vec![0u32; width * height];
    let mut pixels = Pixels::new(
        width as u32,
        height as u32,
        SurfaceTexture::new(width as u32, height as u32, &window),
    )?;

    let mut app = App::new(fonts);
    app.set_available_fonts(asset_provider.list_fonts());
    app.on_event(AppEvent::Start);

    let mut last_frame_time = Instant::now();
    let mut next_frame = last_frame_time + FRAME_DURATION;

    return event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::WaitUntil(next_frame);
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,

                WindowEvent::Resized(new_size) => {
                    if new_size.width > 0 && new_size.height > 0 {
                        width = new_size.width as usize;
                        height = new_size.height as usize;
                        pixel_buffer.resize(width * height, 0);
                        if let Err(err) = pixels.resize_surface(new_size.width, new_size.height) {
                            eprintln!("Failed to resize window surface: {err}");
                            *control_flow = ControlFlow::Exit;
                        }
                        if let Err(err) = pixels.resize_buffer(new_size.width, new_size.height) {
                            eprintln!("Failed to resize pixel buffer: {err}");
                            *control_flow = ControlFlow::Exit;
                        }
                    }
                }

                WindowEvent::MouseInput {
                    state: ElementState::Pressed,
                    button: MouseButton::Left,
                    ..
                } => {
                    app.on_event(AppEvent::Enter);
                }

                WindowEvent::MouseWheel { delta, .. } => {
                    let scroll_y = match delta {
                        MouseScrollDelta::LineDelta(_, y) => y,
                        MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
                    };
                    if scroll_y > 0.0 {
                        app.on_event(AppEvent::Up);
                    } else if scroll_y < 0.0 {
                        app.on_event(AppEvent::Down);
                    }
                }

                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            state: ElementState::Pressed,
                            virtual_keycode: Some(keycode),
                            ..
                        },
                    ..
                } => match keycode {
                    VirtualKeyCode::Up => app.on_event(AppEvent::Up),
                    VirtualKeyCode::Down => app.on_event(AppEvent::Down),
                    VirtualKeyCode::Back => app.on_event(AppEvent::Backspace),
                    VirtualKeyCode::Return => app.on_event(AppEvent::Enter),
                    VirtualKeyCode::Escape => app.on_event(AppEvent::Escape),
                    VirtualKeyCode::Tab => app.on_event(AppEvent::CycleTuiMode),
                    _ => {}
                },

                WindowEvent::ReceivedCharacter(c) => {
                    if !c.is_control() {
                        app.on_event(AppEvent::Char {
                            c,
                            timestamp: crate::timestamp::now(),
                        });
                    }
                }

                _ => {}
            },

            Event::MainEventsCleared => {
                let now = Instant::now();
                if now < next_frame {
                    return;
                }

                let delta_time = now.duration_since(last_frame_time).as_millis() as f64;
                last_frame_time = now;
                next_frame = now + FRAME_DURATION;

                #[cfg(feature = "gui-file")]
                if app.take_file_open_request() {
                    if let Some(path) = FileDialog::new()
                        .add_filter("Typing Problem", &["ntq"])
                        .pick_file()
                    {
                        match DesktopProblemSourceProvider::load_file(&path, 0) {
                            Ok(problem) => {
                                app.add_custom_problem(
                                    problem.name,
                                    problem.content,
                                    problem.timestamp_ms,
                                );
                            }
                            Err(err) => app.report_visible_error(err.to_string()),
                        }
                    }
                }

                if let Some(request) = app.take_font_load_request() {
                    match asset_provider.load_font(request.font_id) {
                        Ok(bytes) => {
                            if let Err(err) = app.apply_font_bytes(request.script, bytes) {
                                app.report_visible_error(format!("failed to apply font: {err:?}"));
                            }
                        }
                        Err(err) => app.report_visible_error(err.to_string()),
                    }
                }

                if app.snapshot().should_quit {
                    *control_flow = ControlFlow::Exit;
                    return;
                }

                render_frame(&mut app, width, height, &mut pixel_buffer, &mut pixels);

                if app.snapshot().should_quit {
                    *control_flow = ControlFlow::Exit;
                }

                app.update(width, height, delta_time.min(100.0));

                if let Err(err) = present_frame(width, height, &app, &mut pixel_buffer, &mut pixels)
                {
                    eprintln!("Failed to draw frame: {err}");
                    *control_flow = ControlFlow::Exit;
                }
            }

            _ => {}
        }
    });
}

#[cfg(not(feature = "uefi"))]
fn render_frame(
    app: &mut App,
    width: usize,
    height: usize,
    pixel_buffer: &mut [u32],
    pixels: &mut Pixels,
) {
    if pixel_buffer.len() != width * height {
        pixel_buffer.fill(0);
    }

    let current_font = app.get_current_font();
    let render_list = ui::build_ui(&app, current_font, width, height);

    for item in render_list {
        match item {
            Renderable::Background { gradient } => {
                draw_linear_gradient(
                    pixel_buffer,
                    width,
                    height,
                    gradient.start_color,
                    gradient.end_color,
                    (0.0, 0.0),
                    (width as f32, height as f32),
                );
            }
            Renderable::BigText {
                text,
                anchor,
                shift,
                align,
                font_size,
                color,
            }
            | Renderable::Text {
                text,
                anchor,
                shift,
                align,
                font_size,
                color,
            } => {
                let pixel_font_size = calculate_pixel_font_size(font_size, width, height);
                let (text_width, text_height, _) =
                    gui_renderer::measure_text(current_font, &text, pixel_font_size);
                let anchor_pos = ui::calculate_anchor_position(anchor, shift, width, height);
                let (x, y) =
                    ui::calculate_aligned_position(anchor_pos, text_width, text_height, align);
                gui_renderer::draw_text(
                    pixel_buffer,
                    width,
                    current_font,
                    &text,
                    (x as f32, y as f32),
                    pixel_font_size,
                    color,
                );
            }
            Renderable::TypingUpper {
                segments,
                anchor,
                shift,
                align,
                font_size,
            } => {
                let pixel_font_size = calculate_pixel_font_size(font_size, width, height);
                let ruby_pixel_font_size = pixel_font_size * 0.4;
                let total_width = segments
                    .iter()
                    .map(|seg| {
                        gui_renderer::measure_text(current_font, &seg.base_text, pixel_font_size).0
                    })
                    .sum::<u32>();
                let total_height = gui_renderer::measure_text(current_font, " ", pixel_font_size).1;

                let anchor_pos = ui::calculate_anchor_position(anchor, shift, width, height);
                let (mut pen_x, y) =
                    ui::calculate_aligned_position(anchor_pos, total_width, total_height, align);

                for seg in segments {
                    let color = match seg.state {
                        UpperSegmentState::Correct => ui::CORRECT_COLOR,
                        UpperSegmentState::Incorrect => ui::INCORRECT_COLOR,
                        UpperSegmentState::Active => ui::ACTIVE_COLOR,
                        UpperSegmentState::Pending => ui::PENDING_COLOR,
                    };

                    gui_renderer::draw_text(
                        pixel_buffer,
                        width,
                        current_font,
                        &seg.base_text,
                        (pen_x as f32, y as f32),
                        pixel_font_size,
                        color,
                    );

                    if let Some(ruby) = &seg.ruby_text {
                        let (base_w, ..) = gui_renderer::measure_text(
                            current_font,
                            &seg.base_text,
                            pixel_font_size,
                        );
                        let (ruby_w, ..) =
                            gui_renderer::measure_text(current_font, ruby, ruby_pixel_font_size);
                        let ruby_x = pen_x as f32 + (base_w as f32 - ruby_w as f32) / 2.0;
                        let ruby_y = y as f32 - ruby_pixel_font_size * 0.5;
                        gui_renderer::draw_text(
                            pixel_buffer,
                            width,
                            current_font,
                            ruby,
                            (ruby_x, ruby_y),
                            ruby_pixel_font_size,
                            color,
                        );
                    }

                    let (seg_width, _, _) =
                        gui_renderer::measure_text(current_font, &seg.base_text, pixel_font_size);
                    pen_x += seg_width as i32;
                }
            }
            Renderable::ProgressBar {
                anchor,
                shift,
                width_ratio,
                height_ratio,
                progress,
                bg_color,
                fg_color,
            } => {
                let bar_width = (width as f32 * width_ratio) as u32;
                let bar_height = (height as f32 * height_ratio) as u32;

                let anchor_pos = ui::calculate_anchor_position(anchor, shift, width, height);
                let start_x = anchor_pos.0 as usize;
                let start_y = (anchor_pos.1 - bar_height as i32).max(0) as usize;

                gui_renderer::draw_rect(
                    pixel_buffer,
                    width,
                    start_x,
                    start_y,
                    bar_width as usize,
                    bar_height as usize,
                    bg_color,
                );

                let fg_width = (bar_width as f32 * progress) as usize;
                if fg_width > 0 {
                    gui_renderer::draw_rect(
                        pixel_buffer,
                        width,
                        start_x,
                        start_y,
                        fg_width,
                        bar_height as usize,
                        fg_color,
                    );
                }
            }
            Renderable::TypingLower {
                segments,
                anchor,
                shift,
                align,
                font_size,
                target_line_total_width,
            } => {
                let pixel_font_size = calculate_pixel_font_size(font_size, width, height);
                let ruby_pixel_font_size = pixel_font_size * 0.3;
                let total_height = gui_renderer::measure_text(current_font, " ", pixel_font_size).1;

                let anchor_pos = ui::calculate_anchor_position(anchor, shift, width, height);
                let (mut pen_x, y) = ui::calculate_aligned_position(
                    anchor_pos,
                    target_line_total_width,
                    total_height,
                    align,
                );
                let visible_left = -100;
                let visible_right = width as i32 + 100;

                for seg in segments {
                    match seg {
                        LowerTypingSegment::Completed {
                            base_text,
                            ruby_text,
                            is_correct,
                            width: seg_width,
                        } => {
                            let seg_width_px = seg_width as i32;
                            if seg_width_px > 0
                                && pen_x <= visible_right
                                && pen_x + seg_width_px >= visible_left
                            {
                                let color = if is_correct {
                                    ui::CORRECT_COLOR
                                } else {
                                    ui::INCORRECT_COLOR
                                };
                                if seg_width_px > 0 {
                                    gui_renderer::draw_text(
                                        pixel_buffer,
                                        width,
                                        current_font,
                                        &base_text,
                                        (pen_x as f32, y as f32),
                                        pixel_font_size,
                                        color,
                                    );
                                }

                                if let Some(ruby) = ruby_text {
                                    let base_w = seg_width;
                                    let (ruby_w, ..) = gui_renderer::measure_text(
                                        current_font,
                                        &ruby,
                                        ruby_pixel_font_size,
                                    );
                                    if ruby_w > 0 {
                                        let ruby_x =
                                            pen_x as f32 + (base_w as f32 - ruby_w as f32) / 2.0;
                                        let ruby_y = y as f32 - ruby_pixel_font_size * 0.5;
                                        gui_renderer::draw_text(
                                            pixel_buffer,
                                            width,
                                            current_font,
                                            &ruby,
                                            (ruby_x, ruby_y),
                                            ruby_pixel_font_size,
                                            color,
                                        );
                                    }
                                }
                            }
                            if seg_width_px > 0 {
                                pen_x += seg_width_px;
                            }
                        }
                        LowerTypingSegment::Active { elements } => {
                            for el in elements {
                                let (text, color) = match el {
                                    ActiveLowerElement::Typed {
                                        character,
                                        is_correct,
                                    } => (
                                        character.to_string(),
                                        if is_correct {
                                            ui::CORRECT_COLOR
                                        } else {
                                            ui::INCORRECT_COLOR
                                        },
                                    ),
                                    ActiveLowerElement::Cursor => {
                                        ("|".to_string(), ui::CURSOR_COLOR)
                                    }
                                    ActiveLowerElement::UnconfirmedInput(s) => {
                                        (s.clone(), ui::UNCONFIRMED_COLOR)
                                    }
                                    ActiveLowerElement::LastIncorrectInput(c) => {
                                        (c.to_string(), ui::WRONG_KEY_COLOR)
                                    }
                                };
                                let text_width = gui_renderer::measure_text(
                                    current_font,
                                    &text,
                                    pixel_font_size,
                                )
                                .0 as i32;
                                if text_width > 0
                                    && pen_x <= visible_right
                                    && pen_x + text_width >= visible_left
                                {
                                    gui_renderer::draw_text(
                                        pixel_buffer,
                                        width,
                                        current_font,
                                        &text,
                                        (pen_x as f32, y as f32),
                                        pixel_font_size,
                                        color,
                                    );
                                }
                                pen_x += text_width;
                            }
                        }
                    }
                }
            }
        }
    }

    let clear_color = crate::renderer::BG_COLOR;
    for pixel in pixel_buffer.iter_mut() {
        if *pixel == 0 {
            *pixel = clear_color;
        }
    }
}

#[cfg(not(feature = "uefi"))]
fn present_frame(
    width: usize,
    height: usize,
    _app: &App,
    pixel_buffer: &mut [u32],
    pixels: &mut Pixels,
) -> Result<(), Box<dyn Error>> {
    if width * height > 0 && pixel_buffer.len() != width * height {
        return Ok(());
    }
    let frame = pixels.frame_mut();
    for (i, color) in pixel_buffer.iter().enumerate() {
        let base = i * 4;
        frame[base] = ((color >> 16) & 0xff) as u8;
        frame[base + 1] = ((color >> 8) & 0xff) as u8;
        frame[base + 2] = (color & 0xff) as u8;
        frame[base + 3] = ((color >> 24) & 0xff) as u8;
    }
    pixels
        .render()
        .map_err(|err| -> Box<dyn Error> { format!("{err}").into() })?;
    Ok(())
}

#[cfg(feature = "uefi")]
pub fn run() -> Result<(), Box<dyn core::error::Error>> {
    Err("GUI is not supported in UEFI environment.".into())
}
