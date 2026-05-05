// src/uefi.rs

extern crate alloc;

use crate::app::{App, AppEvent, Fonts};
use crate::renderer::{ArgbSurface, RenderCache};
use crate::ui;
use ab_glyph::FontVec;
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

    // UEFI ではファイルシステムアクセスが困難なため、バイナリにフォントを埋め込む
    let yuji_font_data: &[u8] = include_bytes!("../fonts/YujiSyuku-Regular.ttf");
    let yuji_font =
        FontVec::try_from_vec(yuji_font_data.to_vec()).expect("Failed to load Yuji Syuku font");

    let traditional_chinese_font_data: &[u8] = include_bytes!("../fonts/NotoSerifJP-Regular.ttf");
    let traditional_chinese_font = FontVec::try_from_vec(traditional_chinese_font_data.to_vec())
        .expect("Failed to load Noto Serif JP font");
    let simplified_chinese_font_data: &[u8] = include_bytes!("../fonts/NotoSerifJP-Regular.ttf");
    let simplified_chinese_font = FontVec::try_from_vec(simplified_chinese_font_data.to_vec())
        .expect("Failed to load Noto Serif JP font");

    // Fonts構造体を初期化
    let fonts = Fonts {
        japanese: yuji_font,
        traditional_chinese: Some(traditional_chinese_font),
        simplified_chinese: Some(simplified_chinese_font),
    };

    // AppにFontsを渡して初期化
    let mut app = App::new(fonts);
    app.on_event(AppEvent::Start);

    let timer_event = unsafe {
        uefi::boot::create_event(EventType::TIMER, Tpl::APPLICATION, None, None).unwrap()
    };
    uefi::boot::set_timer(&timer_event, TimerTrigger::Relative(100_000)).unwrap();

    // 最後のフレームからの経過時間を記録するための変数を初期化
    let mut last_frame_time = crate::timestamp::now();

    let mut events = [timer_event];
    let mut render_cache = RenderCache::new();
    let mut argb_buffer = alloc::vec![0u32; width * height];
    let mut pixel_buffer = alloc::vec![BltPixel::new(0, 0, 0); width * height];

    while !app.snapshot().should_quit {
        uefi::boot::wait_for_event(&mut events).unwrap();

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

        app.update(width, height, delta_time);

        let current_font = app.get_current_font();
        let render_list = ui::build_ui(&app, current_font, width, height);
        if let Some(mut surface) = ArgbSurface::new(width, height, &mut argb_buffer) {
            surface.render(current_font, &render_list, &mut render_cache);
            for (pixel, color) in pixel_buffer.iter_mut().zip(surface.pixels().iter()) {
                *pixel = BltPixel::new(
                    ((*color >> 16) & 0xFF) as u8,
                    ((*color >> 8) & 0xFF) as u8,
                    (*color & 0xFF) as u8,
                );
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
