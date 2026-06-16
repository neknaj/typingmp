#[cfg(not(feature = "uefi"))]
use crate::app::{App, AppEvent};
#[cfg(not(feature = "uefi"))]
use crate::font_loading::load_desktop_fonts;
#[cfg(all(not(feature = "uefi"), feature = "gui-file"))]
use crate::io::DesktopProblemSourceProvider;
#[cfg(not(feature = "uefi"))]
use crate::io::{AssetProvider, DesktopAssetProvider};
#[cfg(not(feature = "uefi"))]
use crate::renderer::{write_argb_as_rgba_bytes, ArgbSurface, RenderCache};
#[cfg(not(feature = "uefi"))]
use crate::ui;
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
    let fonts = load_desktop_fonts(&asset_provider)?;

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
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
    let mut render_cache = RenderCache::new();

    let mut last_frame_time = Instant::now();
    let mut next_frame = last_frame_time + FRAME_DURATION;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::WaitUntil(next_frame);
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,

                WindowEvent::Resized(new_size) if new_size.width > 0 && new_size.height > 0 => {
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

                WindowEvent::ReceivedCharacter(c) if !c.is_control() => {
                    app.on_event(AppEvent::Char {
                        c,
                        timestamp: crate::timestamp::now(),
                    });
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
                            if let Err(err) =
                                app.apply_font_bytes(request.target, request.font_name, bytes)
                            {
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

                let frame_changed = render_frame(
                    &mut app,
                    width,
                    height,
                    &mut pixel_buffer,
                    &mut render_cache,
                );

                if app.snapshot().should_quit {
                    *control_flow = ControlFlow::Exit;
                }

                let viewport = app.display_settings().viewport(width, height);
                app.update(viewport.width, viewport.height, delta_time.min(100.0));

                if let Err(err) =
                    present_frame(width, height, frame_changed, &mut pixel_buffer, &mut pixels)
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
    render_cache: &mut RenderCache,
) -> bool {
    let display_settings = app.display_settings();
    let viewport = display_settings.viewport(width, height);
    let fonts = app.fonts();
    let render_list = ui::build_ui(app, fonts, viewport.width, viewport.height);
    if let Some(mut surface) = ArgbSurface::new(width, height, pixel_buffer) {
        surface
            .render(fonts, display_settings, &render_list, render_cache)
            .changed()
    } else {
        false
    }
}

#[cfg(not(feature = "uefi"))]
fn present_frame(
    width: usize,
    height: usize,
    frame_changed: bool,
    pixel_buffer: &mut [u32],
    pixels: &mut Pixels,
) -> Result<(), Box<dyn Error>> {
    if width * height > 0 && pixel_buffer.len() != width * height {
        return Ok(());
    }
    if !frame_changed {
        return Ok(());
    }
    let frame = pixels.frame_mut();
    write_argb_as_rgba_bytes(pixel_buffer, frame);
    pixels
        .render()
        .map_err(|err| -> Box<dyn Error> { format!("{err}").into() })?;
    Ok(())
}

#[cfg(feature = "uefi")]
pub fn run() -> Result<(), Box<dyn core::error::Error>> {
    Err("GUI is not supported in UEFI environment.".into())
}
