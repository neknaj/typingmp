// src/tui.rs

#[cfg(not(feature = "uefi"))]
use crate::app::{App, AppEvent, FontBundle, Fonts, TuiDisplayMode};
#[cfg(not(feature = "uefi"))]
use crate::backend::BackendError;
#[cfg(not(feature = "uefi"))]
use crate::display::{DisplaySettings, DisplayViewport};
#[cfg(not(feature = "uefi"))]
use crate::font::{script_for_segment, scripts_for_line, segment_script_runs};
#[cfg(not(feature = "uefi"))]
use crate::io::{AssetProvider, BundledFont, DesktopAssetProvider};
#[cfg(not(feature = "uefi"))]
use crate::model::{Line, Segment};
#[cfg(not(feature = "uefi"))]
use crate::renderer::{gui_renderer, tui_renderer}; // gui_renderer をインポート
#[cfg(not(feature = "uefi"))]
use crate::terminal_width;
#[cfg(not(feature = "uefi"))]
use crate::ui::{
    self, ActiveLowerElement, Align, Anchor, FontSize, HorizontalAlign, LowerTypingSegment,
    Renderable, Shift, TypingLineAlignment, UpperTypingSegment, VerticalAlign,
};
#[cfg(not(feature = "uefi"))]
use ab_glyph::FontVec;
#[cfg(not(feature = "uefi"))]
use crossterm::{
    cursor, event,
    event::{Event, KeyCode, KeyEventKind},
    execute,
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

#[cfg(not(feature = "uefi"))]
type TuiGlyphRenderer = fn(&FontVec, &str, f32) -> (Vec<char>, usize, usize, usize);

#[cfg(not(feature = "uefi"))]
const CONTINUATION_CELL: char = '\0';

/// ターミナルの一つのセルを表す構造体。文字と前景（文字）色を持つ。
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg(not(feature = "uefi"))]
struct Cell {
    char: char,
    fg_color: Color,
}

#[derive(Clone, Copy)]
#[cfg(not(feature = "uefi"))]
struct TuiViewport {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    frame_width: usize,
    frame_height: usize,
}

#[cfg(not(feature = "uefi"))]
impl TuiViewport {
    const fn new(width: usize, height: usize) -> Self {
        Self {
            x: 0,
            y: 0,
            width,
            height,
            frame_width: width,
            frame_height: height,
        }
    }

    fn from_display_settings(
        settings: DisplaySettings,
        frame_width: usize,
        frame_height: usize,
        virtual_height: usize,
    ) -> Self {
        let virtual_viewport = settings.viewport(TUI_VIRTUAL_PIXEL_WIDTH, virtual_height);
        Self::from_virtual_viewport(frame_width, frame_height, virtual_height, virtual_viewport)
    }

    fn from_virtual_viewport(
        frame_width: usize,
        frame_height: usize,
        virtual_height: usize,
        virtual_viewport: DisplayViewport,
    ) -> Self {
        if frame_width == 0 || frame_height == 0 {
            return Self::new(frame_width, frame_height);
        }

        let map_x = |value: usize| -> usize {
            ((value as f64 / TUI_VIRTUAL_PIXEL_WIDTH.max(1) as f64) * frame_width as f64)
                .round()
                .clamp(0.0, frame_width as f64) as usize
        };
        let map_y = |value: usize| -> usize {
            if virtual_height == 0 {
                0
            } else {
                ((value as f64 / virtual_height as f64) * frame_height as f64)
                    .round()
                    .clamp(0.0, frame_height as f64) as usize
            }
        };

        let x = map_x(virtual_viewport.x);
        let y = map_y(virtual_viewport.y);
        let right = map_x(virtual_viewport.x.saturating_add(virtual_viewport.width));
        let bottom = map_y(virtual_viewport.y.saturating_add(virtual_viewport.height));

        Self {
            x,
            y,
            width: right.saturating_sub(x).max(1).min(frame_width - x),
            height: bottom.saturating_sub(y).max(1).min(frame_height - y),
            frame_width,
            frame_height,
        }
    }

    fn contains(self, x: isize, y: isize) -> bool {
        x >= self.x as isize
            && y >= self.y as isize
            && x < self.x.saturating_add(self.width) as isize
            && y < self.y.saturating_add(self.height) as isize
            && x < self.frame_width as isize
            && y < self.frame_height as isize
    }

    fn local_to_frame(self, x: i32, y: i32) -> Option<(usize, usize)> {
        let frame_x = self.x as isize + x as isize;
        let frame_y = self.y as isize + y as isize;
        if self.contains(frame_x, frame_y) {
            Some((frame_x as usize, frame_y as usize))
        } else {
            None
        }
    }
}

#[derive(Clone, Copy)]
#[cfg(not(feature = "uefi"))]
struct TuiTextPlacement {
    anchor: Anchor,
    shift: Shift,
    align: Align,
}

#[cfg(not(feature = "uefi"))]
impl TuiTextPlacement {
    const fn new(anchor: Anchor, shift: Shift, align: Align) -> Self {
        Self {
            anchor,
            shift,
            align,
        }
    }
}

#[derive(Clone, Copy)]
#[cfg(not(feature = "uefi"))]
struct TuiArtStyle {
    font_size: FontSize,
    is_braille: bool,
    color: Color,
}

#[cfg(not(feature = "uefi"))]
impl TuiArtStyle {
    const fn new(font_size: FontSize, is_braille: bool, color: Color) -> Self {
        Self {
            font_size,
            is_braille,
            color,
        }
    }
}

#[derive(Clone, Copy)]
#[cfg(not(feature = "uefi"))]
struct CellPosition {
    x: isize,
    y: isize,
}

#[cfg(not(feature = "uefi"))]
impl CellPosition {
    const fn new(x: isize, y: isize) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy)]
#[cfg(not(feature = "uefi"))]
struct ArtBlitPlacement {
    width: usize,
    position: CellPosition,
}

#[cfg(not(feature = "uefi"))]
impl ArtBlitPlacement {
    const fn new(width: usize, x: isize, y: isize) -> Self {
        Self {
            width,
            position: CellPosition::new(x, y),
        }
    }
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

#[cfg(not(feature = "uefi"))]
struct TerminalGuard {
    raw_mode_enabled: bool,
    alternate_screen_enabled: bool,
    cursor_hidden: bool,
}

#[cfg(not(feature = "uefi"))]
impl TerminalGuard {
    fn enter(stdout: &mut impl Write) -> Result<Self, Box<dyn std::error::Error>> {
        terminal::enable_raw_mode()?;
        let mut guard = Self {
            raw_mode_enabled: true,
            alternate_screen_enabled: false,
            cursor_hidden: false,
        };

        if let Err(error) = execute!(stdout, terminal::EnterAlternateScreen) {
            guard.restore(stdout);
            return Err(error.into());
        }
        guard.alternate_screen_enabled = true;
        if let Err(error) = execute!(stdout, cursor::Hide) {
            guard.restore(stdout);
            return Err(error.into());
        }
        guard.cursor_hidden = true;
        Ok(guard)
    }

    fn restore(&mut self, stdout: &mut impl Write) {
        if self.cursor_hidden || self.alternate_screen_enabled {
            let _ = execute!(stdout, cursor::Show, terminal::LeaveAlternateScreen);
            self.cursor_hidden = false;
            self.alternate_screen_enabled = false;
        }
        if self.raw_mode_enabled {
            let _ = terminal::disable_raw_mode();
            self.raw_mode_enabled = false;
        }
    }
}

#[cfg(not(feature = "uefi"))]
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let mut stdout = stdout();
        self.restore(&mut stdout);
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

#[cfg(not(feature = "uefi"))]
fn tui_line_widths(
    line: &Line,
    fonts: &Fonts,
    renderer: TuiGlyphRenderer,
    render_font_size: f32,
    font_size_px: f32,
) -> (u32, f32) {
    let scripts = scripts_for_line(line);
    let mut total_cells = 0_u32;
    let mut total_pixels = 0.0_f32;
    let mut segment_index = 0usize;

    for word in &line.words {
        for segment in &word.segments {
            let script = scripts
                .get(segment_index)
                .copied()
                .unwrap_or_else(|| script_for_segment(segment));
            segment_index += 1;

            for run in segment_script_runs(segment, script) {
                let font = fonts.get_for_script(run.script);
                let run_render_font_size =
                    fonts.scaled_size_for_script(run.script, render_font_size);
                let run_font_size_px = fonts.scaled_size_for_script(run.script, font_size_px);
                total_cells += renderer(font, &run.base_text, run_render_font_size).1 as u32;
                total_pixels +=
                    gui_renderer::measure_text(font, &run.base_text, run_font_size_px).0 as f32;
            }
        }
    }

    (total_cells, total_pixels)
}

fn visible_to_full_cells(
    visible_cells: u32,
    visible_pixels: f32,
    line_alignment: TypingLineAlignment,
) -> (u32, i32) {
    if visible_cells == 0 {
        return (0, 0);
    }

    let pixels_per_cell = if visible_pixels > 0.0 {
        visible_pixels as f64 / visible_cells as f64
    } else {
        1.0
    };
    let full_cells = if line_alignment.full_line_width > 0 {
        (line_alignment.full_line_width as f64 / pixels_per_cell)
            .round()
            .max(visible_cells as f64) as u32
    } else {
        visible_cells
    };
    let visible_start_cells = if line_alignment.visible_start_width > 0 {
        (line_alignment.visible_start_width as f64 / pixels_per_cell).round() as i32
    } else {
        0
    };

    (full_cells, visible_start_cells)
}

fn upper_segments_cell_widths(
    segments: &[UpperTypingSegment],
    fonts: &Fonts,
    renderer: TuiGlyphRenderer,
    render_font_size: f32,
    font_size_px: f32,
) -> (Vec<usize>, u32, f32) {
    let mut widths = Vec::new();
    let mut total_cells = 0_u32;
    let mut total_pixels = 0.0_f32;

    for segment in segments {
        let font = fonts.get_for_script(segment.script);
        let segment_render_font_size =
            fonts.scaled_size_for_script(segment.script, render_font_size);
        let segment_font_size_px = fonts.scaled_size_for_script(segment.script, font_size_px);
        let width = renderer(font, &segment.base_text, segment_render_font_size).1;
        widths.push(width);
        total_cells += width as u32;
        total_pixels +=
            gui_renderer::measure_text(font, &segment.base_text, segment_font_size_px).0 as f32;
    }

    (widths, total_cells, total_pixels)
}

fn upper_segments_char_width(
    segments: &[UpperTypingSegment],
    fonts: &Fonts,
    font_size: f32,
) -> (u32, f32) {
    let mut total_chars = 0_u32;
    let mut total_pixels = 0.0_f32;

    for segment in segments {
        total_chars += terminal_width::text_width(&segment.base_text) as u32;
        total_pixels += gui_renderer::measure_text(
            fonts.get_for_script(segment.script),
            &segment.base_text,
            fonts.scaled_size_for_script(segment.script, font_size),
        )
        .0 as f32;
    }

    (total_chars, total_pixels)
}

/// TUIアプリケーションのメイン関数
fn lower_art_line_metrics(
    segments: &[LowerTypingSegment],
    fonts: &Fonts,
    renderer: TuiGlyphRenderer,
    render_font_size: f32,
) -> (usize, usize) {
    let mut max_ascent = 0usize;
    let mut max_descent = 0usize;

    let mut measure = |font: &FontVec, text: &str, size: f32| {
        let (_, _, height, ascent) = renderer(font, text, size);
        max_ascent = max_ascent.max(ascent);
        max_descent = max_descent.max(height.saturating_sub(ascent));
    };

    for segment in segments {
        match segment {
            LowerTypingSegment::Completed {
                base_text, script, ..
            } => {
                measure(
                    fonts.get_for_script(*script),
                    base_text,
                    fonts.scaled_size_for_script(*script, render_font_size),
                );
            }
            LowerTypingSegment::Active { elements, script } => {
                for element in elements {
                    match element {
                        ActiveLowerElement::Typed {
                            character, script, ..
                        }
                        | ActiveLowerElement::LastIncorrectInput { character, script } => {
                            let mut text = String::new();
                            text.push(*character);
                            measure(
                                fonts.get_for_script(*script),
                                &text,
                                fonts.scaled_size_for_script(*script, render_font_size),
                            );
                        }
                        ActiveLowerElement::Cursor => {
                            measure(
                                fonts.get_for_script(*script),
                                "|",
                                fonts.scaled_size_for_script(*script, render_font_size),
                            );
                        }
                        ActiveLowerElement::UnconfirmedInput { text, script } => {
                            measure(
                                fonts.get_unconfirmed_for_script(*script),
                                text,
                                fonts.scaled_size_for_unconfirmed_script(*script, render_font_size),
                            );
                        }
                    }
                }
            }
        }
    }

    if max_ascent == 0 && max_descent == 0 {
        let (_, _, height, ascent) = renderer(fonts.primary(), "|", render_font_size);
        (height, ascent)
    } else {
        (max_ascent + max_descent, max_ascent)
    }
}

#[cfg(not(feature = "uefi"))]
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let asset_provider = DesktopAssetProvider::discover();
    let ui_font =
        FontVec::try_from_vec(asset_provider.load_bundled_font(BundledFont::NotoSerifJpRegular)?)
            .map_err(|_| BackendError::asset("failed to parse Noto Serif JP font"))?;
    let japanese_font =
        FontVec::try_from_vec(asset_provider.load_bundled_font(BundledFont::YujiSyukuRegular)?)
            .map_err(|_| BackendError::asset("failed to parse Yuji Syuku font"))?;
    let japanese_ruby_font =
        FontVec::try_from_vec(asset_provider.load_bundled_font(BundledFont::YujiSyukuRegular)?)
            .map_err(|_| BackendError::asset("failed to parse Yuji Syuku font"))?;
    let japanese_unconfirmed_font =
        FontVec::try_from_vec(asset_provider.load_bundled_font(BundledFont::YujiSyukuRegular)?)
            .map_err(|_| BackendError::asset("failed to parse Yuji Syuku font"))?;
    let simplified_chinese_font =
        FontVec::try_from_vec(asset_provider.load_bundled_font(BundledFont::LongCangRegular)?)
            .map_err(|_| BackendError::asset("failed to parse Long Cang font"))?;
    let simplified_chinese_ruby_font =
        FontVec::try_from_vec(asset_provider.load_bundled_font(BundledFont::AlegreyaRegular)?)
            .map_err(|_| BackendError::asset("failed to parse Alegreya font"))?;
    let simplified_chinese_unconfirmed_font =
        FontVec::try_from_vec(asset_provider.load_bundled_font(BundledFont::AlegreyaRegular)?)
            .map_err(|_| BackendError::asset("failed to parse Alegreya font"))?;
    let traditional_chinese_font =
        FontVec::try_from_vec(asset_provider.load_bundled_font(BundledFont::LongCangRegular)?)
            .map_err(|_| BackendError::asset("failed to parse Long Cang font"))?;
    let traditional_chinese_ruby_font =
        FontVec::try_from_vec(asset_provider.load_bundled_font(BundledFont::AlegreyaRegular)?)
            .map_err(|_| BackendError::asset("failed to parse Alegreya font"))?;
    let traditional_chinese_unconfirmed_font =
        FontVec::try_from_vec(asset_provider.load_bundled_font(BundledFont::AlegreyaRegular)?)
            .map_err(|_| BackendError::asset("failed to parse Alegreya font"))?;
    let english_font =
        FontVec::try_from_vec(asset_provider.load_bundled_font(BundledFont::KalamRegular)?)
            .map_err(|_| BackendError::asset("failed to parse Kalam font"))?;

    let fonts = Fonts::new(FontBundle {
        ui: ui_font,
        japanese: japanese_font,
        japanese_ruby: japanese_ruby_font,
        japanese_unconfirmed: japanese_unconfirmed_font,
        chinese_simplified: simplified_chinese_font,
        chinese_simplified_ruby: simplified_chinese_ruby_font,
        chinese_simplified_unconfirmed: simplified_chinese_unconfirmed_font,
        traditional_chinese: traditional_chinese_font,
        traditional_chinese_ruby: traditional_chinese_ruby_font,
        traditional_chinese_unconfirmed: traditional_chinese_unconfirmed_font,
        english: english_font,
    });

    let mut stdout = stdout();
    let _terminal_guard = TerminalGuard::enter(&mut stdout)?;

    let mut app = App::new(fonts);
    app.set_available_fonts(asset_provider.list_fonts());
    app.on_event(AppEvent::Start);

    let mut previous_buffer = Vec::new();
    let mut previous_state = app.snapshot().state;
    let mut last_frame_time = Instant::now();

    while !app.snapshot().should_quit {
        let (cols, rows) = terminal::size()?;
        let (cols, rows) = (cols as usize, rows as usize);

        // ターミナルのアスペクト比に合わせて仮想的な高さを計算
        let virtual_height = if cols > 0 {
            (TUI_VIRTUAL_PIXEL_WIDTH as f32 / cols as f32
                * rows as f32
                * (1.0 / tui_renderer::TUI_CHAR_ASPECT_RATIO)) as usize
        } else {
            0
        };

        let now_time = Instant::now();
        let delta_time = now_time.duration_since(last_frame_time).as_millis() as f64;
        last_frame_time = now_time;

        // app.updateには仮想ピクセルサイズを渡す
        let display_settings = app.display_settings();
        let virtual_viewport = display_settings.viewport(TUI_VIRTUAL_PIXEL_WIDTH, virtual_height);
        app.update(virtual_viewport.width, virtual_viewport.height, delta_time);
        if let Some(request) = app.take_font_load_request() {
            match asset_provider.load_font(request.font_id) {
                Ok(bytes) => {
                    if let Err(err) = app.apply_font_bytes(request.target, request.font_name, bytes)
                    {
                        app.report_visible_error(format!("failed to apply font: {err:?}"));
                    }
                }
                Err(err) => app.report_visible_error(err.to_string()),
            }
        }

        // シーンが変更された場合、差分描画をスキップして全画面を再描画するようにする
        let snapshot = app.snapshot();
        let display_mode = snapshot.tui_display_mode;

        if snapshot.state != previous_state {
            previous_buffer.clear();
            execute!(stdout, terminal::Clear(terminal::ClearType::All))?;
        }

        let mut current_buffer = vec![Cell::default(); cols * rows];
        let viewport =
            TuiViewport::from_display_settings(display_settings, cols, rows, virtual_height);

        let fonts = app.fonts();
        // ui.build_uiにも仮想ピクセルサイズを渡す
        let render_list =
            ui::build_ui(&app, fonts, virtual_viewport.width, virtual_viewport.height);
        let primary_font = fonts.primary();
        let ui_font = fonts.ui();

        for item in render_list {
            match item {
                Renderable::Background { .. } => { /* TUIでは何もしない */ }
                Renderable::BigText {
                    text,
                    anchor,
                    shift,
                    align,
                    font_size,
                    color,
                    ..
                } => {
                    let crossterm_color = u32_to_crossterm_color(color);
                    match display_mode {
                        TuiDisplayMode::AsciiArt | TuiDisplayMode::Braille => {
                            let is_braille = display_mode == TuiDisplayMode::Braille;
                            draw_art_text(
                                &mut current_buffer,
                                ui_font,
                                &text,
                                TuiTextPlacement::new(anchor, shift, align),
                                viewport,
                                TuiArtStyle::new(font_size, is_braille, crossterm_color),
                            );
                        }
                        TuiDisplayMode::SimpleText => {
                            draw_plain_text(
                                &mut current_buffer,
                                &text,
                                TuiTextPlacement::new(anchor, shift, align),
                                viewport,
                                crossterm_color,
                            );
                        }
                    }
                }
                Renderable::Text {
                    text,
                    anchor,
                    shift,
                    align,
                    color,
                    ..
                } => {
                    draw_plain_text(
                        &mut current_buffer,
                        &text,
                        TuiTextPlacement::new(anchor, shift, align),
                        viewport,
                        u32_to_crossterm_color(color),
                    );
                }
                Renderable::TypingUpper {
                    segments,
                    anchor,
                    shift,
                    align,
                    font_size,
                    line_alignment,
                    ..
                } => match display_mode {
                    TuiDisplayMode::AsciiArt | TuiDisplayMode::Braille => {
                        let is_braille = display_mode == TuiDisplayMode::Braille;
                        let font_size_px = crate::renderer::calculate_pixel_font_size(
                            font_size,
                            virtual_viewport.width,
                            virtual_viewport.height,
                        ) * display_settings.scale.multiplier();

                        let mut render_font_size = font_size_px;
                        if is_braille {
                            render_font_size *= 2.0;
                        }
                        let renderer = if is_braille {
                            tui_renderer::render_text_to_braille_art
                        } else {
                            tui_renderer::render_text_to_art
                        };

                        let (segment_cell_widths, visible_width_cells, visible_width_pixels) =
                            upper_segments_cell_widths(
                                &segments,
                                fonts,
                                renderer,
                                render_font_size,
                                font_size_px,
                            );
                        let (total_width_cells, visible_start_cells) = visible_to_full_cells(
                            visible_width_cells,
                            visible_width_pixels,
                            line_alignment,
                        );

                        if total_width_cells == 0 {
                            continue;
                        }

                        let line_font = segments
                            .first()
                            .map(|segment| fonts.get_for_script(segment.script))
                            .unwrap_or(primary_font);
                        let line_font_size = segments
                            .first()
                            .map(|segment| {
                                fonts.scaled_size_for_script(segment.script, render_font_size)
                            })
                            .unwrap_or(render_font_size);
                        let (_, _, line_total_height, line_ascent) =
                            renderer(line_font, "|", line_font_size);

                        let anchor_pos = ui::calculate_anchor_position(
                            anchor,
                            shift,
                            viewport.width,
                            viewport.height,
                        );
                        let (mut pen_x, line_start_y) = ui::calculate_aligned_position(
                            anchor_pos,
                            total_width_cells,
                            line_total_height as u32,
                            align,
                        );
                        pen_x += visible_start_cells;
                        let line_baseline_y = line_start_y + line_ascent as i32;

                        for (seg, art_width) in segments.iter().zip(segment_cell_widths) {
                            let color = u32_to_crossterm_color(match seg.state {
                                ui::UpperSegmentState::Correct => ui::CORRECT_COLOR,
                                ui::UpperSegmentState::Incorrect => ui::INCORRECT_COLOR,
                                ui::UpperSegmentState::Active => ui::ACTIVE_COLOR,
                                ui::UpperSegmentState::Pending => ui::PENDING_COLOR,
                                ui::UpperSegmentState::Muted => 0xFF_444444,
                            });

                            let base_font_size =
                                fonts.scaled_size_for_script(seg.script, render_font_size);
                            let (art_buffer, _, _, char_ascent) = renderer(
                                fonts.get_for_script(seg.script),
                                &seg.base_text,
                                base_font_size,
                            );
                            let blit_y = line_baseline_y - char_ascent as i32;
                            blit_art(
                                &mut current_buffer,
                                viewport,
                                &art_buffer,
                                ArtBlitPlacement::new(art_width, pen_x as isize, blit_y as isize),
                                color,
                            );

                            if let Some(ruby) = &seg.ruby_text {
                                let ruby_color = color;
                                if is_braille {
                                    let ruby_font_size_px = fonts.scaled_size_for_ruby_script(
                                        seg.script,
                                        render_font_size * 0.5,
                                    );
                                    let (ruby_art_buffer, ruby_art_width, ruby_art_height, _) =
                                        tui_renderer::render_text_to_braille_art(
                                            fonts.get_ruby_for_script(seg.script),
                                            ruby,
                                            ruby_font_size_px,
                                        );
                                    let ruby_anchor_pos =
                                        (pen_x + (art_width as i32 / 2), line_start_y - 1);
                                    let (ruby_x, ruby_y) = ui::calculate_aligned_position(
                                        ruby_anchor_pos,
                                        ruby_art_width as u32,
                                        ruby_art_height as u32,
                                        Align {
                                            horizontal: HorizontalAlign::Center,
                                            vertical: VerticalAlign::Bottom,
                                        },
                                    );
                                    blit_art(
                                        &mut current_buffer,
                                        viewport,
                                        &ruby_art_buffer,
                                        ArtBlitPlacement::new(
                                            ruby_art_width,
                                            ruby_x as isize,
                                            ruby_y as isize,
                                        ),
                                        ruby_color,
                                    );
                                } else {
                                    let (ruby_width, _) = measure_plain_text(ruby);
                                    let ruby_anchor_pos =
                                        (pen_x + (art_width as i32 / 2), line_start_y - 1);
                                    let (ruby_x, ruby_y) = ui::calculate_aligned_position(
                                        ruby_anchor_pos,
                                        ruby_width,
                                        1,
                                        Align {
                                            horizontal: HorizontalAlign::Center,
                                            vertical: VerticalAlign::Bottom,
                                        },
                                    );
                                    draw_plain_text_at(
                                        &mut current_buffer,
                                        ruby,
                                        ruby_x,
                                        ruby_y,
                                        viewport,
                                        ruby_color,
                                    );
                                }
                            }
                            pen_x += art_width as i32;
                        }
                    }
                    TuiDisplayMode::SimpleText => {
                        let font_size_px = crate::renderer::calculate_pixel_font_size(
                            font_size,
                            virtual_viewport.width,
                            virtual_viewport.height,
                        ) * display_settings.scale.multiplier();
                        let (visible_chars, visible_pixels) =
                            upper_segments_char_width(&segments, fonts, font_size_px);
                        let (total_width, visible_start_chars) =
                            visible_to_full_cells(visible_chars, visible_pixels, line_alignment);
                        if total_width == 0 {
                            continue;
                        }

                        let anchor_pos = ui::calculate_anchor_position(
                            anchor,
                            shift,
                            viewport.width,
                            viewport.height,
                        );
                        let (mut pen_x, pen_y) =
                            ui::calculate_aligned_position(anchor_pos, total_width, 1, align);
                        pen_x += visible_start_chars;

                        for seg in segments {
                            let color = u32_to_crossterm_color(match seg.state {
                                ui::UpperSegmentState::Correct => ui::CORRECT_COLOR,
                                ui::UpperSegmentState::Incorrect => ui::INCORRECT_COLOR,
                                ui::UpperSegmentState::Active => ui::ACTIVE_COLOR,
                                ui::UpperSegmentState::Pending => ui::PENDING_COLOR,
                                ui::UpperSegmentState::Muted => 0xFF_444444,
                            });
                            if let Some(ruby) = &seg.ruby_text {
                                let ruby_x = pen_x
                                    + (terminal_width::text_width(&seg.base_text) as i32
                                        - terminal_width::text_width(ruby) as i32)
                                        / 2;
                                draw_plain_text_at(
                                    &mut current_buffer,
                                    ruby,
                                    ruby_x,
                                    pen_y - 1,
                                    viewport,
                                    color,
                                );
                            }
                            draw_plain_text_at(
                                &mut current_buffer,
                                &seg.base_text,
                                pen_x,
                                pen_y,
                                viewport,
                                color,
                            );
                            pen_x += terminal_width::text_width(&seg.base_text) as i32;
                        }
                    }
                },
                Renderable::TypingLower {
                    segments,
                    anchor,
                    shift,
                    align,
                    font_size,
                    line_alignment,
                } => {
                    match display_mode {
                        TuiDisplayMode::AsciiArt | TuiDisplayMode::Braille => {
                            let is_braille = display_mode == TuiDisplayMode::Braille;
                            let font_size_px = crate::renderer::calculate_pixel_font_size(
                                font_size,
                                virtual_viewport.width,
                                virtual_viewport.height,
                            ) * display_settings.scale.multiplier();
                            let mut render_font_size = font_size_px;
                            if is_braille {
                                render_font_size *= 2.0;
                            }
                            let renderer = if is_braille {
                                tui_renderer::render_text_to_braille_art
                            } else {
                                tui_renderer::render_text_to_art
                            };

                            let Some(typing_model) = app.typing_model() else {
                                continue;
                            };
                            let full_line =
                                &typing_model.content.lines[typing_model.status.line.get()];

                            let (total_width_cells, total_width_pixels) = tui_line_widths(
                                full_line,
                                fonts,
                                renderer,
                                render_font_size,
                                font_size_px,
                            );

                            if total_width_cells == 0 {
                                continue;
                            }

                            let pixels_per_cell =
                                if total_width_cells > 0 && total_width_pixels > 0.0 {
                                    total_width_pixels as f64 / total_width_cells as f64
                                } else {
                                    1.0
                                };

                            let (line_total_height, line_ascent) = lower_art_line_metrics(
                                &segments,
                                fonts,
                                renderer,
                                render_font_size,
                            );

                            let anchor_pos = ui::calculate_anchor_position(
                                anchor,
                                shift,
                                viewport.width,
                                viewport.height,
                            );
                            let (mut pen_x, line_start_y) = ui::calculate_aligned_position(
                                anchor_pos,
                                total_width_cells,
                                line_total_height as u32,
                                align,
                            );
                            pen_x += (line_alignment.visible_start_width as f64 / pixels_per_cell)
                                .round() as i32;
                            let line_baseline_y = line_start_y + line_ascent as i32;

                            for seg in segments {
                                match seg {
                                    LowerTypingSegment::Completed {
                                        base_text,
                                        ruby_text,
                                        script,
                                        is_correct,
                                        ..
                                    } => {
                                        let font = fonts.get_for_script(script);
                                        let base_font_size =
                                            fonts.scaled_size_for_script(script, render_font_size);
                                        let color = u32_to_crossterm_color(if is_correct {
                                            ui::CORRECT_COLOR
                                        } else {
                                            ui::INCORRECT_COLOR
                                        });
                                        let (art_buffer, art_width, _, char_ascent) =
                                            renderer(font, &base_text, base_font_size);
                                        let blit_y = line_baseline_y - char_ascent as i32;
                                        blit_art(
                                            &mut current_buffer,
                                            viewport,
                                            &art_buffer,
                                            ArtBlitPlacement::new(
                                                art_width,
                                                pen_x as isize,
                                                blit_y as isize,
                                            ),
                                            color,
                                        );

                                        if let Some(ruby) = ruby_text {
                                            if is_braille {
                                                let ruby_font_size_px = fonts
                                                    .scaled_size_for_ruby_script(
                                                        script,
                                                        render_font_size * 0.5,
                                                    );
                                                let (
                                                    ruby_art_buffer,
                                                    ruby_art_width,
                                                    ruby_art_height,
                                                    _,
                                                ) = tui_renderer::render_text_to_braille_art(
                                                    fonts.get_ruby_for_script(script),
                                                    &ruby,
                                                    ruby_font_size_px,
                                                );
                                                let ruby_anchor_pos = (
                                                    pen_x + (art_width as i32 / 2),
                                                    line_start_y - 1,
                                                );
                                                let (ruby_x, ruby_y) =
                                                    ui::calculate_aligned_position(
                                                        ruby_anchor_pos,
                                                        ruby_art_width as u32,
                                                        ruby_art_height as u32,
                                                        Align {
                                                            horizontal: HorizontalAlign::Center,
                                                            vertical: VerticalAlign::Bottom,
                                                        },
                                                    );
                                                blit_art(
                                                    &mut current_buffer,
                                                    viewport,
                                                    &ruby_art_buffer,
                                                    ArtBlitPlacement::new(
                                                        ruby_art_width,
                                                        ruby_x as isize,
                                                        ruby_y as isize,
                                                    ),
                                                    color,
                                                );
                                            } else {
                                                let (ruby_width, _) = measure_plain_text(&ruby);
                                                let ruby_anchor = (
                                                    pen_x + (art_width as i32 / 2),
                                                    line_start_y - 1,
                                                );
                                                let (rx, ry) = ui::calculate_aligned_position(
                                                    ruby_anchor,
                                                    ruby_width,
                                                    1,
                                                    Align {
                                                        horizontal: HorizontalAlign::Center,
                                                        vertical: VerticalAlign::Bottom,
                                                    },
                                                );
                                                draw_plain_text_at(
                                                    &mut current_buffer,
                                                    &ruby,
                                                    rx,
                                                    ry,
                                                    viewport,
                                                    color,
                                                );
                                            }
                                        }
                                        pen_x += art_width as i32;
                                    }
                                    LowerTypingSegment::Active { elements, script } => {
                                        for el in elements {
                                            let (text_to_render, color, element_script) = match &el
                                            {
                                                ActiveLowerElement::Typed {
                                                    character,
                                                    is_correct,
                                                    script,
                                                } => (
                                                    (*character).to_string(),
                                                    u32_to_crossterm_color(if *is_correct {
                                                        ui::CORRECT_COLOR
                                                    } else {
                                                        ui::INCORRECT_COLOR
                                                    }),
                                                    *script,
                                                ),
                                                ActiveLowerElement::Cursor => (
                                                    "|".to_string(),
                                                    u32_to_crossterm_color(ui::CURSOR_COLOR),
                                                    script,
                                                ),
                                                ActiveLowerElement::UnconfirmedInput {
                                                    text,
                                                    script,
                                                } => (
                                                    text.clone(),
                                                    u32_to_crossterm_color(ui::UNCONFIRMED_COLOR),
                                                    *script,
                                                ),
                                                ActiveLowerElement::LastIncorrectInput {
                                                    character,
                                                    script,
                                                } => (
                                                    (*character).to_string(),
                                                    u32_to_crossterm_color(ui::WRONG_KEY_COLOR),
                                                    *script,
                                                ),
                                            };
                                            let (font, element_font_size) = match &el {
                                                ActiveLowerElement::UnconfirmedInput { .. } => (
                                                    fonts
                                                        .get_unconfirmed_for_script(element_script),
                                                    fonts.scaled_size_for_unconfirmed_script(
                                                        element_script,
                                                        render_font_size,
                                                    ),
                                                ),
                                                _ => (
                                                    fonts.get_for_script(element_script),
                                                    fonts.scaled_size_for_script(
                                                        element_script,
                                                        render_font_size,
                                                    ),
                                                ),
                                            };

                                            if text_to_render == "|" {
                                                let cursor_height = line_total_height;
                                                for y_offset in 0..cursor_height {
                                                    let target_y = line_start_y + y_offset as i32;
                                                    if let Some((frame_x, frame_y)) =
                                                        viewport.local_to_frame(pen_x, target_y)
                                                    {
                                                        let idx = frame_y * viewport.frame_width
                                                            + frame_x;
                                                        current_buffer[idx] = Cell {
                                                            char: '|',
                                                            fg_color: color,
                                                        };
                                                    }
                                                }
                                                pen_x += 1; // カーソルは常に1セル幅
                                            } else {
                                                let (art_buffer, art_width, _, char_ascent) =
                                                    renderer(
                                                        font,
                                                        &text_to_render,
                                                        element_font_size,
                                                    );
                                                let blit_y = line_baseline_y - char_ascent as i32;
                                                blit_art(
                                                    &mut current_buffer,
                                                    viewport,
                                                    &art_buffer,
                                                    ArtBlitPlacement::new(
                                                        art_width,
                                                        pen_x as isize,
                                                        blit_y as isize,
                                                    ),
                                                    color,
                                                );
                                                pen_x += art_width as i32;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        TuiDisplayMode::SimpleText => {
                            // First, calculate the total width of the line in characters for centering.
                            let Some(typing_model) = app.typing_model() else {
                                continue;
                            };
                            let full_line_words =
                                &typing_model.content.lines[typing_model.status.line.get()].words;
                            let total_width_chars = full_line_words
                                .iter()
                                .flat_map(|w| &w.segments)
                                .map(|seg| {
                                    let text = match seg {
                                        Segment::Plain { text } => text.clone(),
                                        Segment::Annotated { base, .. } => base.clone(),
                                        Segment::Anno { inner, .. } => inner
                                            .iter()
                                            .map(|s| match s {
                                                Segment::Plain { text } => text.as_str(),
                                                Segment::Annotated { base, .. } => base.as_str(),
                                                Segment::Anno { .. } => "",
                                            })
                                            .collect(),
                                    };
                                    terminal_width::text_width(&text)
                                })
                                .sum::<usize>();

                            // Calculate the starting position for the centered line
                            let anchor_pos = ui::calculate_anchor_position(
                                anchor,
                                shift,
                                viewport.width,
                                viewport.height,
                            );
                            let (mut pen_x, pen_y) = ui::calculate_aligned_position(
                                anchor_pos,
                                total_width_chars as u32,
                                1,
                                align,
                            );
                            let visible_start_chars = if line_alignment.full_line_width > 0 {
                                (line_alignment.visible_start_width as f64
                                    / line_alignment.full_line_width as f64
                                    * total_width_chars as f64)
                                    .round() as i32
                            } else {
                                0
                            };
                            pen_x += visible_start_chars;

                            // Now, draw each segment
                            for seg in segments {
                                match seg {
                                    LowerTypingSegment::Completed {
                                        base_text,
                                        is_correct,
                                        ..
                                    } => {
                                        let color = u32_to_crossterm_color(if is_correct {
                                            ui::CORRECT_COLOR
                                        } else {
                                            ui::INCORRECT_COLOR
                                        });
                                        draw_plain_text_at(
                                            &mut current_buffer,
                                            &base_text,
                                            pen_x,
                                            pen_y,
                                            viewport,
                                            color,
                                        );
                                        pen_x += terminal_width::text_width(&base_text) as i32;
                                    }
                                    LowerTypingSegment::Active { elements, .. } => {
                                        for el in elements {
                                            let (text, color) = match el {
                                                ActiveLowerElement::Typed {
                                                    character,
                                                    is_correct,
                                                    ..
                                                } => (
                                                    character.to_string(),
                                                    u32_to_crossterm_color(if is_correct {
                                                        ui::CORRECT_COLOR
                                                    } else {
                                                        ui::INCORRECT_COLOR
                                                    }),
                                                ),
                                                ActiveLowerElement::Cursor => (
                                                    "|".to_string(),
                                                    u32_to_crossterm_color(ui::CURSOR_COLOR),
                                                ),
                                                ActiveLowerElement::UnconfirmedInput {
                                                    text,
                                                    ..
                                                } => (
                                                    text.clone(),
                                                    u32_to_crossterm_color(ui::UNCONFIRMED_COLOR),
                                                ),
                                                ActiveLowerElement::LastIncorrectInput {
                                                    character,
                                                    ..
                                                } => (
                                                    character.to_string(),
                                                    u32_to_crossterm_color(ui::WRONG_KEY_COLOR),
                                                ),
                                            };
                                            draw_plain_text_at(
                                                &mut current_buffer,
                                                &text,
                                                pen_x,
                                                pen_y,
                                                viewport,
                                                color,
                                            );
                                            pen_x += terminal_width::text_width(&text) as i32;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Renderable::ProgressBar {
                    anchor,
                    shift,
                    width_ratio,
                    height_ratio: _,
                    progress,
                    bg_color,
                    fg_color,
                } => {
                    // TUIでは高さは常に1セル
                    let bar_width_chars = (viewport.width as f32 * width_ratio) as usize;

                    let anchor_pos = ui::calculate_anchor_position(
                        anchor,
                        shift,
                        viewport.width,
                        viewport.height,
                    );
                    // anchor_posが左下を指すので、Y座標は1引く
                    let start_x = anchor_pos.0;
                    let start_y = (anchor_pos.1 - 1).max(0);

                    if start_y < 0 || start_y >= viewport.height as i32 {
                        continue;
                    }

                    let bg_crossterm_color = u32_to_crossterm_color(bg_color);
                    let fg_crossterm_color = u32_to_crossterm_color(fg_color);
                    let filled_chars = (bar_width_chars as f32 * progress).round() as usize;

                    for i in 0..bar_width_chars {
                        let x = start_x + i as i32;
                        if let Some((frame_x, frame_y)) = viewport.local_to_frame(x, start_y) {
                            let idx = frame_y * viewport.frame_width + frame_x;
                            let (char, color) = if i < filled_chars {
                                ('█', fg_crossterm_color)
                            } else {
                                ('─', bg_crossterm_color)
                            };
                            current_buffer[idx] = Cell {
                                char,
                                fg_color: color,
                            };
                        }
                    }
                }
            }
        }

        draw_buffer_to_terminal(&mut stdout, &current_buffer, &previous_buffer, cols, rows)?;

        previous_buffer = current_buffer;
        previous_state = app.snapshot().state;

        handle_input(&mut app)?;
    }

    Ok(())
}

/// ASCIIまたは点字アートをバッファに転写する
#[cfg(not(feature = "uefi"))]
fn blit_art(
    buffer: &mut [Cell],
    viewport: TuiViewport,
    art: &[char],
    placement: ArtBlitPlacement,
    color: Color,
) {
    if placement.width == 0 {
        return;
    }
    let art_h = if art.is_empty() {
        0
    } else {
        art.len() / placement.width
    };

    for y in 0..art_h {
        let local_y = placement.position.y + y as isize;
        for x in 0..placement.width {
            let local_x = placement.position.x + x as isize;
            let frame_x = viewport.x as isize + local_x;
            let frame_y = viewport.y as isize + local_y;
            if viewport.contains(frame_x, frame_y) {
                let art_char = art[y * placement.width + x];
                if art_char != ' ' {
                    // Don't blit spaces.
                    let idx = frame_y as usize * viewport.frame_width + frame_x as usize;
                    buffer[idx] = Cell {
                        char: art_char,
                        fg_color: color,
                    };
                }
            }
        }
    }
}

/// TUIでのテキストの寸法を計算する（文字数、1行）
#[cfg(not(feature = "uefi"))]
fn measure_plain_text(text: &str) -> (u32, u32) {
    (terminal_width::text_width(text) as u32, 1)
}

/// 通常のテキストを描画する
#[cfg(not(feature = "uefi"))]
fn draw_plain_text(
    buffer: &mut [Cell],
    text: &str,
    placement: TuiTextPlacement,
    viewport: TuiViewport,
    color: Color,
) {
    let (text_width, text_height) = measure_plain_text(text);
    let anchor_pos = ui::calculate_anchor_position(
        placement.anchor,
        placement.shift,
        viewport.width,
        viewport.height,
    );
    let (start_x, start_y) =
        ui::calculate_aligned_position(anchor_pos, text_width, text_height, placement.align);
    draw_plain_text_at(buffer, text, start_x, start_y, viewport, color);
}

/// 指定した座標にプレーンテキストを描画するヘルパー関数
#[cfg(not(feature = "uefi"))]
fn draw_plain_text_at(
    buffer: &mut [Cell],
    text: &str,
    x: i32,
    y: i32,
    viewport: TuiViewport,
    color: Color,
) {
    let mut pen_x = x;
    for c in text.chars() {
        let char_width = terminal_width::char_width(c);
        if char_width == 0 {
            continue;
        }
        if pen_x + char_width as i32 <= 0 {
            pen_x += char_width as i32;
            continue;
        }
        if pen_x < 0 {
            pen_x += char_width as i32;
            continue;
        }
        if pen_x >= viewport.width as i32 {
            break;
        }
        if char_width > 1 && pen_x + char_width as i32 > viewport.width as i32 {
            break;
        }

        if let Some((frame_x, frame_y)) = viewport.local_to_frame(pen_x, y) {
            let idx = frame_y * viewport.frame_width + frame_x;
            if idx < buffer.len() {
                buffer[idx] = Cell {
                    char: c,
                    fg_color: color,
                };
            }
        }
        for continuation in 1..char_width {
            if let Some((frame_x, frame_y)) =
                viewport.local_to_frame(pen_x + continuation as i32, y)
            {
                let idx = frame_y * viewport.frame_width + frame_x;
                if idx < buffer.len() {
                    buffer[idx] = Cell {
                        char: CONTINUATION_CELL,
                        fg_color: color,
                    };
                }
            }
        }
        pen_x += char_width as i32;
    }
}

/// AA化または点字化されたテキストを描画する
#[cfg(not(feature = "uefi"))]
fn draw_art_text(
    buffer: &mut [Cell],
    font: &FontVec,
    text: &str,
    placement: TuiTextPlacement,
    viewport: TuiViewport,
    style: TuiArtStyle,
) {
    let target_art_height_in_cells =
        calculate_target_art_height(style.font_size, viewport.width, viewport.height);
    if target_art_height_in_cells == 0 {
        return;
    }

    let mut font_size_px = target_art_height_in_cells as f32 * tui_renderer::ART_V_PIXELS_PER_CELL;
    if style.is_braille {
        font_size_px *= 2.0;
    }

    let (art_buffer, art_width, art_height, _) = if style.is_braille {
        tui_renderer::render_text_to_braille_art(font, text, font_size_px)
    } else {
        tui_renderer::render_text_to_art(font, text, font_size_px)
    };

    if art_width == 0 || art_height == 0 {
        return;
    }

    let anchor_pos = ui::calculate_anchor_position(
        placement.anchor,
        placement.shift,
        viewport.width,
        viewport.height,
    );
    let (start_x, start_y) = ui::calculate_aligned_position(
        anchor_pos,
        art_width as u32,
        art_height as u32,
        placement.align,
    );

    blit_art(
        buffer,
        viewport,
        &art_buffer,
        ArtBlitPlacement::new(art_width, start_x as isize, start_y as isize),
        style.color,
    );
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

#[cfg(all(test, not(feature = "uefi")))]
mod tests {
    use super::*;
    use crate::display::{DisplayAspectRatio, DisplayScale};

    #[test]
    fn tui_viewport_maps_display_aspect_to_terminal_cells() {
        let settings = DisplaySettings {
            aspect_ratio: DisplayAspectRatio::Square1x1,
            scale: DisplayScale::Percent100,
        };

        let viewport = TuiViewport::from_display_settings(settings, 80, 40, 500);

        assert_eq!(viewport.x, 20);
        assert_eq!(viewport.y, 0);
        assert_eq!(viewport.width, 40);
        assert_eq!(viewport.height, 40);
        assert_eq!(viewport.frame_width, 80);
        assert_eq!(viewport.frame_height, 40);
    }

    #[test]
    fn draw_plain_text_at_clips_to_tui_viewport() {
        let viewport = TuiViewport {
            x: 2,
            y: 1,
            width: 4,
            height: 2,
            frame_width: 8,
            frame_height: 4,
        };
        let mut buffer = vec![Cell::default(); viewport.frame_width * viewport.frame_height];

        draw_plain_text_at(&mut buffer, "abcdef", -1, 0, viewport, Color::Red);
        draw_plain_text_at(&mut buffer, "Z", 0, -1, viewport, Color::Blue);

        let row = &buffer[viewport.frame_width..viewport.frame_width * 2];
        assert_eq!(row[0].char, ' ');
        assert_eq!(row[1].char, ' ');
        assert_eq!(row[2].char, 'b');
        assert_eq!(row[3].char, 'c');
        assert_eq!(row[4].char, 'd');
        assert_eq!(row[5].char, 'e');
        assert_eq!(row[6].char, ' ');
        assert_eq!(row[2].fg_color, Color::Red);
    }

    #[test]
    fn draw_plain_text_at_marks_wide_character_continuation_cells() {
        let viewport = TuiViewport::new(8, 2);
        let mut buffer = vec![Cell::default(); viewport.frame_width * viewport.frame_height];

        draw_plain_text_at(&mut buffer, "春A", 0, 0, viewport, Color::Yellow);

        assert_eq!(buffer[0].char, '春');
        assert_eq!(buffer[1].char, CONTINUATION_CELL);
        assert_eq!(buffer[2].char, 'A');
        assert_eq!(buffer[0].fg_color, Color::Yellow);
        assert_eq!(buffer[1].fg_color, Color::Yellow);
    }
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
    let is_full_redraw =
        previous_buffer.is_empty() || current_buffer.len() != previous_buffer.len();

    if is_full_redraw {
        execute!(stdout, terminal::Clear(terminal::ClearType::All))?;
    }

    for y in 0..rows {
        let row_start = y * width;
        if row_start >= current_buffer.len() {
            break;
        }
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
            if cell.char == CONTINUATION_CELL {
                continue;
            }
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
                    KeyCode::Char(c) => app.on_event(AppEvent::Char {
                        c,
                        timestamp: crate::timestamp::now(),
                    }),
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
