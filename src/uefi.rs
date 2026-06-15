// src/uefi.rs

extern crate alloc;

use crate::app::{App, AppEvent, Fonts};
use crate::io::{bundled_font_data, bundled_font_entries};
use crate::renderer::{ArgbSurface, RenderCache};
use crate::ui;
use ab_glyph::FontVec;
use alloc::vec::Vec;
use uefi::boot::{EventType, TimerTrigger, Tpl};
use uefi::prelude::*;
use uefi::proto::console::gop::{BltOp, BltPixel, BltRegion, GraphicsOutput};
use uefi::proto::console::text::{Key, ScanCode};

pub fn run() -> Status {
    match run_inner() {
        Ok(()) => Status::SUCCESS,
        Err(status) => {
            report_startup_failure(status);
            status
        }
    }
}

fn run_inner() -> core::result::Result<(), Status> {
    firmware(uefi::helpers::init())?;

    let gop_handle = firmware(uefi::boot::get_handle_for_protocol::<GraphicsOutput>())?;
    let mut gop = firmware(uefi::boot::open_protocol_exclusive::<GraphicsOutput>(
        gop_handle,
    ))?;

    let mode_info = gop.current_mode_info();
    let (width, height) = mode_info.resolution();

    // UEFI ではファイルシステムアクセスが困難なため、バイナリにフォントを埋め込む
    let yuji_font_data: &[u8] = include_bytes!("../fonts/YujiSyuku-Regular.ttf");
    let yuji_font = load_embedded_font(yuji_font_data)?;
    let ui_font_data: &[u8] = include_bytes!("../fonts/NotoSerifJP-Regular.ttf");
    let ui_font = load_embedded_font(ui_font_data)?;

    let simplified_chinese_font_data: &[u8] = include_bytes!("../fonts/MaShanZheng-Regular.ttf");
    let simplified_chinese_font = load_embedded_font(simplified_chinese_font_data)?;
    let traditional_chinese_font_data: &[u8] = include_bytes!("../fonts/MaShanZheng-Regular.ttf");
    let traditional_chinese_font = load_embedded_font(traditional_chinese_font_data)?;
    let english_font_data: &[u8] = include_bytes!("../fonts/Kalam-Regular.ttf");
    let english_font = load_embedded_font(english_font_data)?;

    // Fonts構造体を初期化
    let fonts = Fonts::new(
        ui_font,
        yuji_font,
        simplified_chinese_font,
        traditional_chinese_font,
        english_font,
    );

    // AppにFontsを渡して初期化
    let mut app = App::new(fonts);
    app.set_available_fonts(bundled_font_entries());
    app.on_event(AppEvent::Start);

    let timer_event = firmware(unsafe {
        uefi::boot::create_event(EventType::TIMER, Tpl::APPLICATION, None, None)
    })?;
    firmware(uefi::boot::set_timer(
        &timer_event,
        TimerTrigger::Relative(100_000),
    ))?;

    // 最後のフレームからの経過時間を記録するための変数を初期化
    let mut last_frame_time = crate::timestamp::now();

    let mut events = [timer_event];
    let mut render_cache = RenderCache::new();
    let mut argb_buffer = alloc::vec![0u32; width * height];
    let mut pixel_buffer = alloc::vec![BltPixel::new(0, 0, 0); width * height];

    while !app.snapshot().should_quit {
        firmware(uefi::boot::wait_for_event(&mut events))?;

        let keys: Vec<Key> = uefi::system::with_stdin(|stdin| {
            let mut collected_keys = Vec::new();
            while let Ok(Some(key)) = stdin.read_key() {
                collected_keys.push(key);
            }
            collected_keys
        });

        let now_time = crate::timestamp::now();
        let delta_time = now_time - last_frame_time;
        last_frame_time = now_time;

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
                            timestamp: now_time,
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

        let display_settings = app.display_settings();
        let viewport = display_settings.viewport(width, height);
        app.update(viewport.width, viewport.height, delta_time);
        if let Some(request) = app.take_font_load_request() {
            match bundled_font_data(request.font_id) {
                Some(bytes) => {
                    if app
                        .apply_font_bytes(request.target, request.font_name, bytes.to_vec())
                        .is_err()
                    {
                        app.report_visible_error("failed to apply font");
                    }
                }
                None => app.report_visible_error("selected bundled font was not found"),
            }
        }

        let fonts = app.fonts();
        let render_list = ui::build_ui(&app, fonts, viewport.width, viewport.height);
        if let Some(mut surface) = ArgbSurface::new(width, height, &mut argb_buffer) {
            surface.render(fonts, display_settings, &render_list, &mut render_cache);
            for (pixel, color) in pixel_buffer.iter_mut().zip(surface.pixels().iter()) {
                *pixel = BltPixel::new(
                    ((*color >> 16) & 0xFF) as u8,
                    ((*color >> 8) & 0xFF) as u8,
                    (*color & 0xFF) as u8,
                );
            }
        }
        firmware(gop.blt(BltOp::BufferToVideo {
            buffer: &pixel_buffer,
            src: BltRegion::Full,
            dest: (0, 0),
            dims: (width, height),
        }))?;

        firmware(uefi::boot::set_timer(
            &events[0],
            TimerTrigger::Relative(100_000),
        ))?;
    }

    Ok(())
}

fn firmware<T, D: core::fmt::Debug>(result: uefi::Result<T, D>) -> core::result::Result<T, Status> {
    result.map_err(|error| error.status())
}

fn report_startup_failure(status: Status) {
    uefi::println!("typingmp UEFI error: {:?}", status);
}

fn load_embedded_font(bytes: &[u8]) -> core::result::Result<FontVec, Status> {
    FontVec::try_from_vec(bytes.to_vec()).map_err(|_| Status::LOAD_ERROR)
}
