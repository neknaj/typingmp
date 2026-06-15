use crate::app::{App, AppEvent, FontBundle, Fonts};
use crate::backend::BackendError;
use crate::io::{bundled_font_data, bundled_font_entries};
use crate::model::Segment;
use crate::ui::{self, ActiveLowerElement, Align, Anchor, LowerTypingSegment, Renderable, Shift};
use ab_glyph::FontVec;
use std::io::{self, Write};

const VIRTUAL_PIXEL_WIDTH: usize = 1000;
const DEFAULT_COLUMNS: usize = 100;
const DEFAULT_LINES: usize = 30;
const TUI_CHAR_ASPECT_RATIO: f32 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnsiColor {
    Reset,
    Rgb(u32),
}

impl AnsiColor {
    const fn from_argb(value: u32) -> Self {
        Self::Rgb(value & 0x00FF_FFFF)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Cell {
    character: char,
    color: AnsiColor,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            character: ' ',
            color: AnsiColor::Reset,
        }
    }
}

#[derive(Clone, Copy)]
struct CellViewport {
    columns: usize,
    rows: usize,
}

impl CellViewport {
    const fn new(columns: usize, rows: usize) -> Self {
        Self { columns, rows }
    }
}

#[derive(Clone, Copy)]
struct TextPlacement {
    anchor: Anchor,
    shift: Shift,
    align: Align,
}

impl TextPlacement {
    const fn new(anchor: Anchor, shift: Shift, align: Align) -> Self {
        Self {
            anchor,
            shift,
            align,
        }
    }
}

#[derive(Clone, Copy)]
struct ProgressPlacement {
    anchor: Anchor,
    shift: Shift,
    width_ratio: f32,
    progress: f32,
}

#[derive(Clone, Copy)]
struct ProgressColors {
    background: AnsiColor,
    foreground: AnsiColor,
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let _terminal_guard = AnsiTerminalGuard;
    let fonts = bundled_fonts()?;
    let mut app = App::new(fonts);
    app.set_available_fonts(bundled_font_entries());
    app.on_event(AppEvent::Start);

    let mut stdout = io::stdout();
    let stdin = io::stdin();
    let mut last_frame_time = crate::timestamp::now();

    loop {
        let viewport = terminal_size();
        let virtual_height = virtual_height(viewport);
        let display_settings = app.display_settings();
        let virtual_viewport = display_settings.viewport(VIRTUAL_PIXEL_WIDTH, virtual_height);
        let now = crate::timestamp::now();
        let delta_time = (now - last_frame_time).max(0.0);
        last_frame_time = now;
        app.update(virtual_viewport.width, virtual_viewport.height, delta_time);
        if let Some(request) = app.take_font_load_request() {
            match bundled_font_data(request.font_id) {
                Some(bytes) => {
                    if let Err(err) =
                        app.apply_font_bytes(request.target, request.font_name, bytes.to_vec())
                    {
                        app.report_visible_error(format!("failed to apply font: {err:?}"));
                    }
                }
                None => app.report_visible_error("selected bundled font was not found"),
            }
        }

        let frame = render_frame(
            &app,
            viewport,
            virtual_viewport.width,
            virtual_viewport.height,
        );
        write_frame(&mut stdout, &frame, viewport)?;
        if app.snapshot().should_quit {
            break;
        }

        write!(stdout, "\n> ")?;
        stdout.flush()?;

        let mut line = String::new();
        if stdin.read_line(&mut line)? == 0 {
            break;
        }
        if !handle_line_input(&mut app, &line) {
            break;
        }
    }

    Ok(())
}

struct AnsiTerminalGuard;

impl Drop for AnsiTerminalGuard {
    fn drop(&mut self) {
        let mut stdout = io::stdout();
        let _ = write!(stdout, "\x1b[0m\x1b[?25h");
        let _ = stdout.flush();
    }
}

fn bundled_fonts() -> Result<Fonts, BackendError> {
    let ui_font =
        FontVec::try_from_vec(include_bytes!("../fonts/NotoSerifJP-Regular.ttf").to_vec())
            .map_err(|_| BackendError::asset("failed to parse Noto Serif JP font"))?;
    let japanese_font =
        FontVec::try_from_vec(include_bytes!("../fonts/YujiSyuku-Regular.ttf").to_vec())
            .map_err(|_| BackendError::asset("failed to parse Yuji Syuku font"))?;
    let japanese_ruby_font =
        FontVec::try_from_vec(include_bytes!("../fonts/YujiSyuku-Regular.ttf").to_vec())
            .map_err(|_| BackendError::asset("failed to parse Yuji Syuku font"))?;
    let simplified_chinese_font =
        FontVec::try_from_vec(include_bytes!("../fonts/MaShanZheng-Regular.ttf").to_vec())
            .map_err(|_| BackendError::asset("failed to parse Ma Shan Zheng font"))?;
    let simplified_chinese_ruby_font =
        FontVec::try_from_vec(include_bytes!("../fonts/MaShanZheng-Regular.ttf").to_vec())
            .map_err(|_| BackendError::asset("failed to parse Ma Shan Zheng font"))?;
    let traditional_chinese_font =
        FontVec::try_from_vec(include_bytes!("../fonts/MaShanZheng-Regular.ttf").to_vec())
            .map_err(|_| BackendError::asset("failed to parse Ma Shan Zheng font"))?;
    let traditional_chinese_ruby_font =
        FontVec::try_from_vec(include_bytes!("../fonts/MaShanZheng-Regular.ttf").to_vec())
            .map_err(|_| BackendError::asset("failed to parse Ma Shan Zheng font"))?;
    let english_font = FontVec::try_from_vec(include_bytes!("../fonts/Kalam-Regular.ttf").to_vec())
        .map_err(|_| BackendError::asset("failed to parse Kalam font"))?;

    Ok(Fonts::new(FontBundle {
        ui: ui_font,
        japanese: japanese_font,
        japanese_ruby: japanese_ruby_font,
        chinese_simplified: simplified_chinese_font,
        chinese_simplified_ruby: simplified_chinese_ruby_font,
        traditional_chinese: traditional_chinese_font,
        traditional_chinese_ruby: traditional_chinese_ruby_font,
        english: english_font,
    }))
}

fn terminal_size() -> CellViewport {
    let columns = env_usize("COLUMNS").unwrap_or(DEFAULT_COLUMNS).max(1);
    let rows = env_usize("LINES").unwrap_or(DEFAULT_LINES).max(1);
    CellViewport::new(columns, rows)
}

fn env_usize(name: &str) -> Option<usize> {
    std::env::var(name).ok()?.parse().ok()
}

fn virtual_height(viewport: CellViewport) -> usize {
    if viewport.columns == 0 {
        return 0;
    }
    (VIRTUAL_PIXEL_WIDTH as f32 / viewport.columns as f32
        * viewport.rows as f32
        * (1.0 / TUI_CHAR_ASPECT_RATIO)) as usize
}

fn render_frame(
    app: &App,
    viewport: CellViewport,
    virtual_width: usize,
    virtual_height: usize,
) -> Vec<Cell> {
    let mut cells = vec![Cell::default(); viewport.columns * viewport.rows];
    let render_list = ui::build_ui(app, app.fonts(), virtual_width, virtual_height);

    for item in render_list {
        match item {
            Renderable::Background { .. } => {}
            Renderable::BigText {
                text,
                anchor,
                shift,
                align,
                color,
                ..
            }
            | Renderable::Text {
                text,
                anchor,
                shift,
                align,
                color,
                ..
            } => draw_plain_text(
                &mut cells,
                &text,
                TextPlacement::new(anchor, shift, align),
                viewport,
                AnsiColor::from_argb(color),
            ),
            Renderable::TypingUpper {
                segments,
                anchor,
                shift,
                align,
                line_alignment,
                ..
            } => {
                let visible_width_chars = upper_segments_char_count(&segments);
                let total_width_chars = if line_alignment.visible_start_width > 0 {
                    current_line_base_char_count(app)
                        .max(visible_width_chars)
                        .max(1)
                } else {
                    visible_width_chars.max(1)
                };
                let anchor_pos =
                    ui::calculate_anchor_position(anchor, shift, viewport.columns, viewport.rows);
                let (mut pen_x, pen_y) =
                    ui::calculate_aligned_position(anchor_pos, total_width_chars as u32, 1, align);
                pen_x += visible_start_chars(line_alignment, total_width_chars);

                for segment in segments {
                    if let Some(ruby) = &segment.ruby_text {
                        let ruby_x = pen_x
                            + (segment.base_text.chars().count() as i32
                                - ruby.chars().count() as i32)
                                / 2;
                        draw_plain_text_at(
                            &mut cells,
                            ruby,
                            ruby_x,
                            pen_y - 1,
                            viewport.columns,
                            upper_segment_color(segment.state),
                        );
                    }
                    draw_plain_text_at(
                        &mut cells,
                        &segment.base_text,
                        pen_x,
                        pen_y,
                        viewport.columns,
                        upper_segment_color(segment.state),
                    );
                    pen_x += segment.base_text.chars().count() as i32;
                }
            }
            Renderable::TypingLower {
                segments,
                anchor,
                shift,
                align,
                line_alignment,
                ..
            } => {
                let total_width_chars = current_line_base_char_count(app).max(1);
                let anchor_pos =
                    ui::calculate_anchor_position(anchor, shift, viewport.columns, viewport.rows);
                let (mut pen_x, pen_y) =
                    ui::calculate_aligned_position(anchor_pos, total_width_chars as u32, 1, align);
                pen_x += visible_start_chars(line_alignment, total_width_chars);

                for segment in segments {
                    match segment {
                        LowerTypingSegment::Completed {
                            base_text,
                            ruby_text,
                            is_correct,
                            ..
                        } => {
                            let color = if is_correct {
                                AnsiColor::from_argb(ui::CORRECT_COLOR)
                            } else {
                                AnsiColor::from_argb(ui::INCORRECT_COLOR)
                            };
                            if let Some(ruby) = ruby_text {
                                let ruby_x = pen_x
                                    + (base_text.chars().count() as i32
                                        - ruby.chars().count() as i32)
                                        / 2;
                                draw_plain_text_at(
                                    &mut cells,
                                    &ruby,
                                    ruby_x,
                                    pen_y - 1,
                                    viewport.columns,
                                    color,
                                );
                            }
                            draw_plain_text_at(
                                &mut cells,
                                &base_text,
                                pen_x,
                                pen_y,
                                viewport.columns,
                                color,
                            );
                            pen_x += base_text.chars().count() as i32;
                        }
                        LowerTypingSegment::Active { elements, .. } => {
                            for element in elements {
                                let (text, color) = active_lower_text_and_color(&element);
                                draw_plain_text_at(
                                    &mut cells,
                                    &text,
                                    pen_x,
                                    pen_y,
                                    viewport.columns,
                                    color,
                                );
                                pen_x += text.chars().count() as i32;
                            }
                        }
                    }
                }
            }
            Renderable::ProgressBar {
                anchor,
                shift,
                width_ratio,
                progress,
                bg_color,
                fg_color,
                ..
            } => draw_progress_bar(
                &mut cells,
                viewport,
                ProgressPlacement {
                    anchor,
                    shift,
                    width_ratio,
                    progress,
                },
                ProgressColors {
                    background: AnsiColor::from_argb(bg_color),
                    foreground: AnsiColor::from_argb(fg_color),
                },
            ),
        }
    }

    cells
}

fn visible_start_chars(line_alignment: ui::TypingLineAlignment, total_width_chars: usize) -> i32 {
    if line_alignment.full_line_width == 0 {
        return 0;
    }

    (line_alignment.visible_start_width as f64 / line_alignment.full_line_width as f64
        * total_width_chars as f64)
        .round() as i32
}

fn current_line_base_char_count(app: &App) -> usize {
    let Some(model) = app.typing_model() else {
        return 0;
    };
    model
        .content
        .lines
        .get(model.status.line.get())
        .map(|line| {
            line.words
                .iter()
                .flat_map(|word| &word.segments)
                .map(segment_base_text)
                .map(|text| text.chars().count())
                .sum()
        })
        .unwrap_or(0)
}

fn upper_segments_char_count(segments: &[ui::UpperTypingSegment]) -> usize {
    segments
        .iter()
        .map(|segment| segment.base_text.chars().count())
        .sum()
}

fn segment_base_text(segment: &Segment) -> String {
    match segment {
        Segment::Plain { text } => text.clone(),
        Segment::Annotated { base, .. } => base.clone(),
        Segment::Anno { inner, .. } => inner.iter().map(segment_base_text).collect(),
    }
}

fn draw_plain_text(
    buffer: &mut [Cell],
    text: &str,
    placement: TextPlacement,
    viewport: CellViewport,
    color: AnsiColor,
) {
    let text_width = text.chars().count() as u32;
    let anchor_pos = ui::calculate_anchor_position(
        placement.anchor,
        placement.shift,
        viewport.columns,
        viewport.rows,
    );
    let (x, y) = ui::calculate_aligned_position(anchor_pos, text_width, 1, placement.align);
    draw_plain_text_at(buffer, text, x, y, viewport.columns, color);
}

fn draw_plain_text_at(
    buffer: &mut [Cell],
    text: &str,
    x: i32,
    y: i32,
    width: usize,
    color: AnsiColor,
) {
    if width == 0 || y < 0 || y >= (buffer.len() / width) as i32 {
        return;
    }

    for (offset, character) in text.chars().enumerate() {
        let current_x = x + offset as i32;
        if current_x < 0 || current_x >= width as i32 {
            continue;
        }
        let index = y as usize * width + current_x as usize;
        if let Some(cell) = buffer.get_mut(index) {
            *cell = Cell { character, color };
        }
    }
}

fn draw_progress_bar(
    buffer: &mut [Cell],
    viewport: CellViewport,
    placement: ProgressPlacement,
    colors: ProgressColors,
) {
    let width = (viewport.columns as f32 * placement.width_ratio).max(0.0) as usize;
    if width == 0 {
        return;
    }

    let anchor_pos = ui::calculate_anchor_position(
        placement.anchor,
        placement.shift,
        viewport.columns,
        viewport.rows,
    );
    let start_x = anchor_pos.0;
    let y = (anchor_pos.1 - 1).max(0);
    if y < 0 || y >= viewport.rows as i32 {
        return;
    }

    let filled = (width as f32 * placement.progress.clamp(0.0, 1.0)).round() as usize;
    for offset in 0..width {
        let x = start_x + offset as i32;
        if x < 0 || x >= viewport.columns as i32 {
            continue;
        }
        let index = y as usize * viewport.columns + x as usize;
        if let Some(cell) = buffer.get_mut(index) {
            *cell = if offset < filled {
                Cell {
                    character: '#',
                    color: colors.foreground,
                }
            } else {
                Cell {
                    character: '-',
                    color: colors.background,
                }
            };
        }
    }
}

fn write_frame(stdout: &mut impl Write, frame: &[Cell], viewport: CellViewport) -> io::Result<()> {
    write!(stdout, "\x1b[?25l\x1b[2J\x1b[H")?;
    let mut current_color = AnsiColor::Reset;

    for row in 0..viewport.rows {
        let row_start = row * viewport.columns;
        let row_end = (row_start + viewport.columns).min(frame.len());
        for cell in &frame[row_start..row_end] {
            if cell.color != current_color {
                write_color(stdout, cell.color)?;
                current_color = cell.color;
            }
            write!(stdout, "{}", cell.character)?;
        }
        if row + 1 < viewport.rows {
            writeln!(stdout)?;
        }
    }
    write!(stdout, "\x1b[0m")?;
    stdout.flush()
}

fn write_color(stdout: &mut impl Write, color: AnsiColor) -> io::Result<()> {
    match color {
        AnsiColor::Reset => write!(stdout, "\x1b[0m"),
        AnsiColor::Rgb(value) => {
            let r = (value >> 16) & 0xFF;
            let g = (value >> 8) & 0xFF;
            let b = value & 0xFF;
            write!(stdout, "\x1b[38;2;{r};{g};{b}m")
        }
    }
}

fn upper_segment_color(state: ui::UpperSegmentState) -> AnsiColor {
    match state {
        ui::UpperSegmentState::Correct => AnsiColor::from_argb(ui::CORRECT_COLOR),
        ui::UpperSegmentState::Incorrect => AnsiColor::from_argb(ui::INCORRECT_COLOR),
        ui::UpperSegmentState::Active => AnsiColor::from_argb(ui::ACTIVE_COLOR),
        ui::UpperSegmentState::Pending => AnsiColor::from_argb(ui::PENDING_COLOR),
        ui::UpperSegmentState::Muted => AnsiColor::from_argb(0xFF_444444),
    }
}

fn active_lower_text_and_color(element: &ActiveLowerElement) -> (String, AnsiColor) {
    match element {
        ActiveLowerElement::Typed {
            character,
            is_correct,
            ..
        } => (
            character.to_string(),
            if *is_correct {
                AnsiColor::from_argb(ui::CORRECT_COLOR)
            } else {
                AnsiColor::from_argb(ui::INCORRECT_COLOR)
            },
        ),
        ActiveLowerElement::Cursor => ("|".to_string(), AnsiColor::from_argb(ui::CURSOR_COLOR)),
        ActiveLowerElement::UnconfirmedInput { text, .. } => {
            (text.clone(), AnsiColor::from_argb(ui::UNCONFIRMED_COLOR))
        }
        ActiveLowerElement::LastIncorrectInput { character, .. } => (
            character.to_string(),
            AnsiColor::from_argb(ui::WRONG_KEY_COLOR),
        ),
    }
}

fn handle_line_input(app: &mut App, line: &str) -> bool {
    let input = line.trim_end_matches(['\r', '\n']);
    match input {
        "" | "/enter" => app.on_event(AppEvent::Enter),
        "/q" | "/quit" => return false,
        "/esc" => app.on_event(AppEvent::Escape),
        "/tab" => app.on_event(AppEvent::CycleTuiMode),
        "/up" => app.on_event(AppEvent::Up),
        "/down" => app.on_event(AppEvent::Down),
        "/bs" | "/backspace" => app.on_event(AppEvent::Backspace),
        text => {
            for character in text.chars() {
                app.on_event(AppEvent::Char {
                    c: character,
                    timestamp: crate::timestamp::now(),
                });
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_commands_map_to_app_events() {
        let fonts = bundled_fonts().expect("bundled fonts should parse");
        let mut app = App::new(fonts);
        app.on_event(AppEvent::Start);

        assert!(handle_line_input(&mut app, "/down"));
        assert!(handle_line_input(&mut app, "/up"));
        assert!(handle_line_input(&mut app, ""));
        assert!(!handle_line_input(&mut app, "/q"));
    }
}
