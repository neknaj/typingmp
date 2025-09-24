// src/tui.rs

#[cfg(not(feature = "uefi"))]
use crate::app::{App, AppEvent, Fonts, TuiDisplayMode};
#[cfg(not(feature = "uefi"))]
use crate::model::Segment;
#[cfg(not(feature = "uefi"))]
use crate::renderer::{gui_renderer, tui_renderer}; // gui_renderer をインポート
#[cfg(not(feature = "uefi"))]
use crate::ui::{
    self, ActiveLowerElement, Align, Anchor, FontSize, HorizontalAlign, LowerTypingSegment,
    Renderable, Shift, VerticalAlign,
};
#[cfg(not(feature = "uefi"))]
use ab_glyph::FontRef;
#[cfg(not(feature = "uefi"))]
use crossterm::{
    cursor, event, execute,
    event::{Event, KeyCode, KeyEventKind},
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal,
};
#[cfg(not(feature = "uefi"))]
use std::io::{stdout, Write};
#[cfg(not(feature = "uefi"))]
use std::time::{Duration, Instant};

// 共通のスクロール計算ロジックのために、TUIでも仮想的なピクセル幅を定義する
#[cfg(not(feature = "uefi"))]
const TUI_VIRTUAL_PIXEL_WIDTH: usize = 1000;

/// ターミナルの一つのセルを表す構造体。文字と前景（文字）色を持つ。
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg(not(feature = "uefi"))]
struct Cell {
    char: char,
    fg_color: Color,
}

#[cfg(not(feature = "uefi"))]
impl Default for Cell {
    fn default() -> Self {
        Self {
            char: ' ',
            fg_color: Color::Reset,
        }
    }
}

/// u32形式のRGBカラーコードをcrosstermのColor::Rgbに変換する
#[cfg(not(feature = "uefi"))]
fn u32_to_crossterm_color(c: u32) -> Color {
    let r = ((c >> 16) & 0xFF) as u8;
    let g = ((c >> 8) & 0xFF) as u8;
    let b = (c & 0xFF) as u8;
    Color::Rgb { r, g, b }
}


/// TUIアプリケーションのメイン関数
#[cfg(not(feature = "uefi"))]
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let yuji_font_data = include_bytes!("../fonts/YujiSyuku-Regular.ttf");
    let yuji_font = FontRef::try_from_slice(yuji_font_data).map_err(|_| "Failed to load Yuji Syuku font")?;
    
    let noto_font_data = include_bytes!("../fonts/NotoSerifJP-Regular.ttf");
    let noto_font = FontRef::try_from_slice(noto_font_data).map_err(|_| "Failed to load Noto Serif JP font")?;

    let fonts = Fonts {
        yuji_syuku: yuji_font,
        noto_serif: noto_font,
    };

    let mut stdout = stdout();
    terminal::enable_raw_mode()?;
    execute!(stdout, terminal::EnterAlternateScreen, cursor::Hide)?;

    let mut app = App::new(fonts);
    app.on_event(AppEvent::Start);

    let mut previous_buffer = Vec::new();
    let mut previous_state = app.state;
    let mut last_frame_time = Instant::now();

    while !app.should_quit {
        let (cols, rows) = terminal::size()?;
        let (cols, rows) = (cols as usize, rows as usize);

        // ターミナルのアスペクト比に合わせて仮想的な高さを計算
        let virtual_height = if cols > 0 {
            (TUI_VIRTUAL_PIXEL_WIDTH as f32 / cols as f32 * rows as f32 * (1.0 / tui_renderer::TUI_CHAR_ASPECT_RATIO)) as usize
        } else {
            0
        };
        
        let now_time = Instant::now();
        let delta_time = now_time.duration_since(last_frame_time).as_millis() as f64;
        last_frame_time = now_time;
        
        // app.updateには仮想ピクセルサイズを渡す
        app.update(TUI_VIRTUAL_PIXEL_WIDTH, virtual_height, delta_time);

        // シーンが変更された場合、差分描画をスキップして全画面を再描画するようにする
        if app.state != previous_state {
            previous_buffer.clear();
            execute!(stdout, terminal::Clear(terminal::ClearType::All))?;
        }

        let mut current_buffer = vec![Cell::default(); cols * rows];

        let current_font = app.get_current_font();
        // ui.build_uiにも仮想ピクセルサイズを渡す
        let render_list = ui::build_ui(&app, current_font, TUI_VIRTUAL_PIXEL_WIDTH, virtual_height);

        for item in render_list {
            match item {
                Renderable::Background { .. } => { /* TUIでは何もしない */ }
                Renderable::BigText { text, anchor, shift, align, font_size, color, .. } => {
                    let crossterm_color = u32_to_crossterm_color(color);
                    match app.tui_display_mode {
                        TuiDisplayMode::AsciiArt | TuiDisplayMode::Braille => {
                            let is_braille = app.tui_display_mode == TuiDisplayMode::Braille;
                            draw_art_text(&mut current_buffer, current_font, &text, anchor, shift, align, font_size, cols, rows, is_braille, crossterm_color);
                        }
                        TuiDisplayMode::SimpleText => {
                            draw_plain_text(&mut current_buffer, &text, anchor, shift, align, cols, rows, crossterm_color);
                        }
                    }
                }
                Renderable::Text { text, anchor, shift, align, color, .. } => {
                    draw_plain_text(&mut current_buffer, &text, anchor, shift, align, cols, rows, u32_to_crossterm_color(color));
                }
                Renderable::TypingUpper { segments, anchor, shift, align, font_size } => {
                     match app.tui_display_mode {
                        TuiDisplayMode::AsciiArt | TuiDisplayMode::Braille => {
                            let is_braille = app.tui_display_mode == TuiDisplayMode::Braille;
                            let font_size_px = crate::renderer::calculate_pixel_font_size(font_size, TUI_VIRTUAL_PIXEL_WIDTH, virtual_height);

                            let mut render_font_size = font_size_px;
                            if is_braille { render_font_size *= 2.0; }
                            let renderer = if is_braille { tui_renderer::render_text_to_braille_art } else { tui_renderer::render_text_to_art };
                            
                            let (total_width_cells, total_width_pixels) = segments.iter().fold((0_u32, 0.0_f32), |(acc_cells, acc_pixels), seg| {
                                let cells = renderer(current_font, &seg.base_text, render_font_size).1 as u32;
                                let pixels = gui_renderer::measure_text(current_font, &seg.base_text, font_size_px).0 as f32;
                                (acc_cells + cells, acc_pixels + pixels)
                            });

                            if total_width_cells == 0 { continue; }
                            
                            let pixels_per_cell = if total_width_cells > 0 { total_width_pixels as f64 / total_width_cells as f64 } else { 1.0 };
                            let scroll_offset_cells = (app.typing_model.as_ref().unwrap().scroll.scroll / pixels_per_cell).round() as i32;

                            let (_, _, line_total_height, line_ascent) = renderer(current_font, "|", render_font_size);

                            let y_only_shift = Shift { x: 0.0, y: shift.y };
                            let anchor_pos = ui::calculate_anchor_position(anchor, y_only_shift, cols, rows);
                            let (center_pen_x, line_start_y) = ui::calculate_aligned_position(anchor_pos, total_width_cells, line_total_height as u32, align);
                            let mut pen_x = center_pen_x - scroll_offset_cells;
                            let line_baseline_y = line_start_y + line_ascent as i32;

                            for seg in segments {
                                let color = u32_to_crossterm_color(match seg.state {
                                    ui::UpperSegmentState::Correct => ui::CORRECT_COLOR,
                                    ui::UpperSegmentState::Incorrect => ui::INCORRECT_COLOR,
                                    ui::UpperSegmentState::Active => ui::ACTIVE_COLOR,
                                    ui::UpperSegmentState::Pending => ui::PENDING_COLOR,
                                });

                                let (art_buffer, art_width, _, char_ascent) = renderer(current_font, &seg.base_text, render_font_size);
                                let blit_y = line_baseline_y - char_ascent as i32;
                                blit_art(&mut current_buffer, cols, rows, &art_buffer, art_width, 0, pen_x as isize, blit_y as isize, color);

                                if let Some(ruby) = &seg.ruby_text {
                                    let ruby_color = color;
                                    if is_braille {
                                        let ruby_font_size_px = render_font_size * 0.5;
                                        let (ruby_art_buffer, ruby_art_width, ruby_art_height, _) = tui_renderer::render_text_to_braille_art(current_font, &ruby, ruby_font_size_px);
                                        let ruby_anchor_pos = (pen_x + (art_width as i32 / 2), line_start_y - 1);
                                        let (ruby_x, ruby_y) = ui::calculate_aligned_position(ruby_anchor_pos, ruby_art_width as u32, ruby_art_height as u32, Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Bottom });
                                        blit_art(&mut current_buffer, cols, rows, &ruby_art_buffer, ruby_art_width, ruby_art_height, ruby_x as isize, ruby_y as isize, ruby_color);
                                    } else {
                                        let (ruby_width, _) = measure_plain_text(ruby);
                                        let ruby_anchor_pos = (pen_x + (art_width as i32 / 2), line_start_y - 1);
                                        let (ruby_x, ruby_y) = ui::calculate_aligned_position(ruby_anchor_pos, ruby_width, 1, Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Bottom });
                                        draw_plain_text_at(&mut current_buffer, ruby, ruby_x, ruby_y, cols, ruby_color);
                                    }
                                }
                                pen_x += art_width as i32;
                            }
                        }
                        TuiDisplayMode::SimpleText => {
                            // Calculate total width for centering
                            let total_width = segments.iter().map(|s| s.base_text.chars().count()).sum::<usize>();
                            let anchor_pos = ui::calculate_anchor_position(anchor, shift, cols, rows);
                            let (mut pen_x, pen_y) = ui::calculate_aligned_position(anchor_pos, total_width as u32, 1, align);

                            for seg in segments {
                                let color = u32_to_crossterm_color(match seg.state {
                                    ui::UpperSegmentState::Correct => ui::CORRECT_COLOR,
                                    ui::UpperSegmentState::Incorrect => ui::INCORRECT_COLOR,
                                    ui::UpperSegmentState::Active => ui::ACTIVE_COLOR,
                                    ui::UpperSegmentState::Pending => ui::PENDING_COLOR,
                                });
                                draw_plain_text_at(&mut current_buffer, &seg.base_text, pen_x, pen_y, cols, color);
                                pen_x += seg.base_text.chars().count() as i32;
                            }
                        }
                    }
                }
                Renderable::TypingLower { segments, anchor, shift, align, font_size, .. } => {
                    match app.tui_display_mode {
                        TuiDisplayMode::AsciiArt | TuiDisplayMode::Braille => {
                            let is_braille = app.tui_display_mode == TuiDisplayMode::Braille;
                            let font_size_px = crate::renderer::calculate_pixel_font_size(font_size, TUI_VIRTUAL_PIXEL_WIDTH, virtual_height);
                            let mut render_font_size = font_size_px;
                            if is_braille { render_font_size *= 2.0; }
                            let renderer = if is_braille { tui_renderer::render_text_to_braille_art } else { tui_renderer::render_text_to_art };
                            
                            let full_line_words = &app.typing_model.as_ref().unwrap().content.lines[app.typing_model.as_ref().unwrap().status.line as usize].words;

                            let (total_width_cells, total_width_pixels) = full_line_words.iter().flat_map(|w| &w.segments).fold((0_u32, 0.0_f32), |(acc_cells, acc_pixels), seg| {
                                let text = match seg {
                                    Segment::Plain { text } => text.as_str(),
                                    Segment::Annotated { base, .. } => base.as_str(),
                                };
                                let cells = renderer(current_font, text, render_font_size).1 as u32;
                                let pixels = gui_renderer::measure_text(current_font, text, font_size_px).0 as f32;
                                (acc_cells + cells, acc_pixels + pixels)
                            });

                            if total_width_cells == 0 { continue; }

                            let pixels_per_cell = if total_width_cells > 0 { total_width_pixels as f64 / total_width_cells as f64 } else { 1.0 };
                            let scroll_offset_cells = (app.typing_model.as_ref().unwrap().scroll.scroll / pixels_per_cell).round() as i32;

                            let (_, _, line_total_height, line_ascent) = renderer(current_font, "|", render_font_size);

                            let y_only_shift = Shift { x: 0.0, y: shift.y };
                            let anchor_pos = ui::calculate_anchor_position(anchor, y_only_shift, cols, rows);
                            let (center_pen_x, line_start_y) = ui::calculate_aligned_position(anchor_pos, total_width_cells, line_total_height as u32, align);
                            let mut pen_x = center_pen_x - scroll_offset_cells;
                            let line_baseline_y = line_start_y + line_ascent as i32;

                            for seg in segments {
                                match seg {
                                    LowerTypingSegment::Completed { base_text, ruby_text, is_correct } => {
                                        let color = u32_to_crossterm_color(if is_correct { ui::CORRECT_COLOR } else { ui::INCORRECT_COLOR });
                                        let (art_buffer, art_width, _, char_ascent) = renderer(current_font, &base_text, render_font_size);
                                        let blit_y = line_baseline_y - char_ascent as i32;
                                        blit_art(&mut current_buffer, cols, rows, &art_buffer, art_width, 0, pen_x as isize, blit_y as isize, color);

                                        if let Some(ruby) = ruby_text {
                                            if is_braille {
                                                let ruby_font_size_px = render_font_size * 0.5;
                                                let (ruby_art_buffer, ruby_art_width, ruby_art_height, _) = tui_renderer::render_text_to_braille_art(current_font, &ruby, ruby_font_size_px);
                                                let ruby_anchor_pos = (pen_x + (art_width as i32 / 2), line_start_y - 1);
                                                let (ruby_x, ruby_y) = ui::calculate_aligned_position(ruby_anchor_pos, ruby_art_width as u32, ruby_art_height as u32, Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Bottom });
                                                blit_art(&mut current_buffer, cols, rows, &ruby_art_buffer, ruby_art_width, ruby_art_height, ruby_x as isize, ruby_y as isize, color);
                                            } else {
                                                let (ruby_width, _) = measure_plain_text(&ruby);
                                                let ruby_anchor = (pen_x + (art_width as i32 / 2), line_start_y - 1);
                                                let (rx, ry) = ui::calculate_aligned_position(ruby_anchor, ruby_width, 1, Align { horizontal: HorizontalAlign::Center, vertical: VerticalAlign::Bottom });
                                                draw_plain_text_at(&mut current_buffer, &ruby, rx, ry, cols, color);
                                            }
                                        }
                                        pen_x += art_width as i32;
                                    }
                                    LowerTypingSegment::Active { elements } => {
                                        for el in elements {
                                            let (text_to_render, color) = match el {
                                                ActiveLowerElement::Typed { character, is_correct } => (character.to_string(), u32_to_crossterm_color(if is_correct { ui::CORRECT_COLOR } else { ui::INCORRECT_COLOR })),
                                                ActiveLowerElement::Cursor => ("|".to_string(), u32_to_crossterm_color(ui::CURSOR_COLOR)),
                                                ActiveLowerElement::UnconfirmedInput(s) => (s.clone(), u32_to_crossterm_color(ui::UNCONFIRMED_COLOR)),
                                                ActiveLowerElement::LastIncorrectInput(c) => (c.to_string(), u32_to_crossterm_color(ui::WRONG_KEY_COLOR)),
                                            };

                                            if text_to_render == "|" {
                                                let cursor_height = line_total_height;
                                                for y_offset in 0..cursor_height {
                                                    let target_y = line_start_y + y_offset as i32;
                                                    if target_y >= 0 && target_y < rows as i32 && pen_x >= 0 && pen_x < cols as i32 {
                                                        let idx = target_y as usize * cols + pen_x as usize;
                                                        current_buffer[idx] = Cell { char: '|', fg_color: color };
                                                    }
                                                }
                                                pen_x += 1; // カーソルは常に1セル幅
                                            } else {
                                                let (art_buffer, art_width, _, char_ascent) = renderer(current_font, &text_to_render, render_font_size);
                                                let blit_y = line_baseline_y - char_ascent as i32;
                                                blit_art(&mut current_buffer, cols, rows, &art_buffer, art_width, 0, pen_x as isize, blit_y as isize, color);
                                                pen_x += art_width as i32;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        TuiDisplayMode::SimpleText => {
                            // First, calculate the total width of the line in characters for centering.
                            let full_line_words = &app.typing_model.as_ref().unwrap().content.lines[app.typing_model.as_ref().unwrap().status.line as usize].words;
                            let total_width_chars = full_line_words.iter().flat_map(|w| &w.segments).map(|seg| {
                                let text = match seg {
                                    Segment::Plain { text } => text.as_str(),
                                    Segment::Annotated { base, .. } => base.as_str(),
                                };
                                text.chars().count()
                            }).sum::<usize>();

                            // Calculate the starting position for the centered line
                            let anchor_pos = ui::calculate_anchor_position(anchor, shift, cols, rows);
                            let (mut pen_x, pen_y) = ui::calculate_aligned_position(anchor_pos, total_width_chars as u32, 1, align);

                            // Now, draw each segment
                            for seg in segments {
                                match seg {
                                    LowerTypingSegment::Completed { base_text, is_correct, .. } => {
                                        let color = u32_to_crossterm_color(if is_correct { ui::CORRECT_COLOR } else { ui::INCORRECT_COLOR });
                                        draw_plain_text_at(&mut current_buffer, &base_text, pen_x, pen_y, cols, color);
                                        pen_x += base_text.chars().count() as i32;
                                    },
                                    LowerTypingSegment::Active { elements } => {
                                        for el in elements {
                                             let (text, color) = match el {
                                                ActiveLowerElement::Typed { character, is_correct } => (character.to_string(), u32_to_crossterm_color(if is_correct { ui::CORRECT_COLOR } else { ui::INCORRECT_COLOR })),
                                                ActiveLowerElement::Cursor => ("|".to_string(), u32_to_crossterm_color(ui::CURSOR_COLOR)),
                                                ActiveLowerElement::UnconfirmedInput(s) => (s.clone(), u32_to_crossterm_color(ui::UNCONFIRMED_COLOR)),
                                                ActiveLowerElement::LastIncorrectInput(c) => (c.to_string(), u32_to_crossterm_color(ui::WRONG_KEY_COLOR)),
                                            };
                                            draw_plain_text_at(&mut current_buffer, &text, pen_x, pen_y, cols, color);
                                            pen_x += text.chars().count() as i32;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Renderable::ProgressBar { anchor, shift, width_ratio, height_ratio: _, progress, bg_color, fg_color } => {
                    // TUIでは高さは常に1セル
                    let bar_width_chars = (cols as f32 * width_ratio) as usize;

                    let anchor_pos = ui::calculate_anchor_position(anchor, shift, cols, rows);
                    // anchor_posが左下を指すので、Y座標は1引く
                    let start_x = anchor_pos.0;
                    let start_y = (anchor_pos.1 - 1).max(0);

                    if start_y < 0 || start_y >= rows as i32 { continue; }

                    let bg_crossterm_color = u32_to_crossterm_color(bg_color);
                    let fg_crossterm_color = u32_to_crossterm_color(fg_color);
                    let filled_chars = (bar_width_chars as f32 * progress).round() as usize;

                    for i in 0..bar_width_chars {
                        let x = start_x + i as i32;
                        if x >= 0 && x < cols as i32 {
                            let idx = start_y as usize * cols + x as usize;
                            let (char, color) = if i < filled_chars { ('█', fg_crossterm_color) } else { ('─', bg_crossterm_color) };
                            current_buffer[idx] = Cell { char, fg_color: color };
                        }
                    }
                }
            }
        }

        draw_buffer_to_terminal(&mut stdout, &current_buffer, &previous_buffer, cols, rows)?;
        
        previous_buffer = current_buffer;
        previous_state = app.state;

        handle_input(&mut app)?;
    }

    execute!(stdout, cursor::Show, terminal::LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;
    Ok(())
}

/// ASCIIまたは点字アートをバッファに転写する
#[cfg(not(feature = "uefi"))]
fn blit_art(
    buffer: &mut [Cell], buf_w: usize, buf_h: usize,
    art: &[char], art_w: usize, _art_h: usize,
    start_x: isize, start_y: isize, color: Color,
) {
    if art_w == 0 { return; }
    let art_h = if art.is_empty() { 0 } else { art.len() / art_w };

    for y in 0..art_h {
        let target_y = start_y + y as isize;
        if target_y >= 0 && target_y < buf_h as isize {
            for x in 0..art_w {
                let target_x = start_x + x as isize;
                if target_x >= 0 && target_x < buf_w as isize {
                    let art_char = art[y * art_w + x];
                    if art_char != ' ' { // Don't blit spaces
                        let idx = target_y as usize * buf_w + target_x as usize;
                        buffer[idx] = Cell { char: art_char, fg_color: color };
                    }
                }
            }
        }
    }
}

/// TUIでのテキストの寸法を計算する（文字数、1行）
#[cfg(not(feature = "uefi"))]
fn measure_plain_text(text: &str) -> (u32, u32) {
    (text.chars().count() as u32, 1)
}

/// 通常のテキストを描画する
#[cfg(not(feature = "uefi"))]
fn draw_plain_text(
    buffer: &mut [Cell], text: &str, anchor: Anchor, shift: Shift, align: Align,
    width: usize, height: usize, color: Color,
) {
    let (text_width, text_height) = measure_plain_text(text);
    let anchor_pos = ui::calculate_anchor_position(anchor, shift, width, height);
    let (start_x, start_y) = ui::calculate_aligned_position(anchor_pos, text_width, text_height, align);
    draw_plain_text_at(buffer, text, start_x, start_y, width, color);
}

/// 指定した座標にプレーンテキストを描画するヘルパー関数
#[cfg(not(feature = "uefi"))]
fn draw_plain_text_at(buffer: &mut [Cell], text: &str, x: i32, y: i32, width: usize, color: Color) {
    if y < 0 || y >= (buffer.len() / width) as i32 { return; }
    for (i, c) in text.chars().enumerate() {
        let current_x = x + i as i32;
        if current_x >= 0 && current_x < width as i32 {
            let idx = y as usize * width + current_x as usize;
            if idx < buffer.len() {
                buffer[idx] = Cell { char: c, fg_color: color };
            }
        }
    }
}


/// AA化または点字化されたテキストを描画する
#[cfg(not(feature = "uefi"))]
fn draw_art_text(
    buffer: &mut [Cell], font: &FontRef, text: &str, anchor: Anchor, shift: Shift, align: Align, font_size: FontSize,
    cols: usize, rows: usize, is_braille: bool, color: Color,
) {
    let target_art_height_in_cells = calculate_target_art_height(font_size, cols, rows);
    if target_art_height_in_cells == 0 { return; }

    let mut font_size_px = target_art_height_in_cells as f32 * tui_renderer::ART_V_PIXELS_PER_CELL;
    if is_braille {
        font_size_px *= 2.0;
    }

    let (art_buffer, art_width, art_height, _) = if is_braille {
        tui_renderer::render_text_to_braille_art(font, text, font_size_px)
    } else {
        tui_renderer::render_text_to_art(font, text, font_size_px)
    };

    if art_width == 0 || art_height == 0 { return; }
    
    let anchor_pos = ui::calculate_anchor_position(anchor, shift, cols, rows);
    let (start_x, start_y) = ui::calculate_aligned_position(anchor_pos, art_width as u32, art_height as u32, align);
    
    blit_art(buffer, cols, rows, &art_buffer, art_width, art_height, start_x as isize, start_y as isize, color);
}

/// フォントサイズ指定から目標となるAAの高さを計算するヘルパー関数
#[cfg(not(feature = "uefi"))]
fn calculate_target_art_height(font_size: FontSize, _cols: usize, rows: usize) -> usize {
     match font_size {
        FontSize::WindowHeight(ratio) => (rows as f32 * ratio).ceil() as usize,
        FontSize::WindowAreaSqrt(ratio) => {
            // TUIではアスペクト比が不定なため、高さを基準にする
            (rows as f32 * ratio * 2.0).ceil() as usize
        }
    }
}

#[cfg(feature = "uefi")]
pub fn run() -> Result<(), Box<dyn core::error::Error>> {
    Err("TUI is not supported in UEFI environment yet.".into())
}

/// 差分を検出し、ターミナルに必要な部分だけ描画する。
/// 同じ色の文字が続く場合は、まとめて描画することでパフォーマンスを最適化する。
#[cfg(not(feature = "uefi"))]
fn draw_buffer_to_terminal(
    stdout: &mut impl Write,
    current_buffer: &[Cell],
    previous_buffer: &[Cell],
    width: usize,
    rows: usize,
) -> std::io::Result<()> {
    let is_full_redraw = previous_buffer.is_empty() || current_buffer.len() != previous_buffer.len();

    if is_full_redraw {
        execute!(stdout, terminal::Clear(terminal::ClearType::All))?;
    }

    for y in 0..rows {
        let row_start = y * width;
        if row_start >= current_buffer.len() { break; }
        let row_end = (row_start + width).min(current_buffer.len());

        let current_row = &current_buffer[row_start..row_end];
        
        let needs_redraw = if is_full_redraw {
            true
        } else {
            let prev_row = &previous_buffer[row_start..row_end];
            current_row != prev_row
        };

        if !needs_redraw {
            continue;
        }

        execute!(stdout, cursor::MoveTo(0, y as u16))?;
        let mut last_color = Color::Reset;
        let mut batch = String::new();
        
        for cell in current_row {
            if cell.fg_color != last_color {
                if !batch.is_empty() {
                    execute!(stdout, Print(&batch))?;
                    batch.clear();
                }
                execute!(stdout, SetForegroundColor(cell.fg_color))?;
                last_color = cell.fg_color;
            }
            batch.push(cell.char);
        }
        if !batch.is_empty() {
            execute!(stdout, Print(&batch))?;
        }
    }

    execute!(stdout, ResetColor, cursor::Hide)?;
    stdout.flush()
}


/// キーボード入力を処理する
#[cfg(not(feature = "uefi"))]
fn handle_input(app: &mut App) -> std::io::Result<()> {
    if event::poll(Duration::from_millis(100))? {
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char(c) => app.on_event(AppEvent::Char { c, timestamp: crate::timestamp::now() }),
                    KeyCode::Backspace => app.on_event(AppEvent::Backspace),
                    KeyCode::Up => app.on_event(AppEvent::Up),
                    KeyCode::Down => app.on_event(AppEvent::Down),
                    KeyCode::Enter => app.on_event(AppEvent::Enter),
                    KeyCode::Esc => app.on_event(AppEvent::Escape),
                    KeyCode::Tab => app.on_event(AppEvent::CycleTuiMode),
                    _ => {}
                }
            }
        }
    }
    Ok(())
}

/// シンプルモード用にテキストを描画するヘルパー関数
#[cfg(not(feature = "uefi"))]
fn draw_simple_typing_text(
    buffer: &mut [Cell],
    text: &str,
    anchor: Anchor,
    shift: Shift,
    align: Align,
    width: usize,
    height: usize,
    color: Color,
) {
    draw_plain_text(buffer, text, anchor, shift, align, width, height, color);
}