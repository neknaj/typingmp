// src/renderer.rs
// uefi featureが有効な場合、標準のallocクレートをインポート
#[cfg(feature = "uefi")]
extern crate alloc;

// uefi と std で使用する Vec と vec! を切り替える
#[cfg(feature = "uefi")]
use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
#[cfg(not(feature = "uefi"))]
use std::{
    string::{String, ToString},
    vec::Vec,
};

#[cfg(feature = "uefi")]
use core_maths::CoreFloat;

use crate::display::{DisplaySettings, DisplayViewport};
use crate::font::Fonts;
use crate::ui::{
    self, ActiveLowerElement, FontSize, LowerTypingSegment, Renderable, UpperSegmentState,
};
use ab_glyph::{point, Font, OutlinedGlyph, PxScale, ScaleFont};

/// 背景の描画色
pub const BG_COLOR: u32 = 0xFF_000000;

/// ピクセルバッファに線形グラデーションを描画する
pub fn draw_linear_gradient(
    buffer: &mut [u32],
    width: usize,
    height: usize,
    start_color: u32,
    end_color: u32,
    start_point: (f32, f32),
    end_point: (f32, f32),
) {
    let (x0, y0) = start_point;
    let (x1, y1) = end_point;

    let dx = x1 - x0;
    let dy = y1 - y0;
    let len_sq = dx * dx + dy * dy;

    for y in 0..height {
        for x in 0..width {
            let p_x = x as f32;
            let p_y = y as f32;

            let dot_product = (p_x - x0) * dx + (p_y - y0) * dy;
            let ratio = if len_sq == 0.0 {
                0.0
            } else {
                (dot_product / len_sq).clamp(0.0, 1.0)
            };

            let r = (((start_color >> 16) & 0xFF) as f32 * (1.0 - ratio)
                + ((end_color >> 16) & 0xFF) as f32 * ratio) as u32;
            let g = (((start_color >> 8) & 0xFF) as f32 * (1.0 - ratio)
                + ((end_color >> 8) & 0xFF) as f32 * ratio) as u32;
            let b = (((start_color) & 0xFF) as f32 * (1.0 - ratio)
                + ((end_color) & 0xFF) as f32 * ratio) as u32;
            let interpolated_color = (0xFF << 24) | (r << 16) | (g << 8) | b;

            let index = y * width + x;
            buffer[index] = interpolated_color;
        }
    }
}

/// Calculates the actual pixel font size based on the FontSize enum and window dimensions.
pub fn calculate_pixel_font_size(font_size: FontSize, width: usize, height: usize) -> f32 {
    match font_size {
        FontSize::WindowHeight(ratio) => height as f32 * ratio,
        FontSize::WindowAreaSqrt(ratio) => {
            let area = (width * height) as f32;
            area.sqrt() * ratio
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TextMetrics {
    width: u32,
    height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PixelClip {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

impl PixelClip {
    const fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    fn for_buffer(stride: usize, height: usize) -> Self {
        Self::new(0, 0, stride as i32, height as i32)
    }

    fn is_empty(self) -> bool {
        self.left >= self.right || self.top >= self.bottom
    }

    fn intersects_f32(self, left: f32, top: f32, right: f32, bottom: f32) -> bool {
        !self.is_empty()
            && right > self.left as f32
            && left < self.right as f32
            && bottom > self.top as f32
            && top < self.bottom as f32
    }
}

#[derive(Debug, Clone, Copy)]
struct TextDrawOptions {
    pos: (f32, f32),
    font_size: f32,
    color: u32,
    clip: PixelClip,
}

impl TextDrawOptions {
    const fn new(pos: (f32, f32), font_size: f32, color: u32, clip: PixelClip) -> Self {
        Self {
            pos,
            font_size,
            color,
            clip,
        }
    }
}

#[derive(Debug, Clone)]
struct TextMeasureCacheEntry {
    font_key: usize,
    text: String,
    size_bits: u32,
    metrics: TextMetrics,
}

#[derive(Debug, Clone)]
struct BackgroundCache {
    start_color: u32,
    end_color: u32,
    width: usize,
    height: usize,
    pixels: Vec<u32>,
}

/// Shared render cache used by pixel backends.
pub struct RenderCache {
    font_generation: u64,
    text_metrics: Vec<TextMeasureCacheEntry>,
    background: Option<BackgroundCache>,
}

impl RenderCache {
    const TEXT_MEASURE_CACHE_LIMIT: usize = 512;

    pub const fn new() -> Self {
        Self {
            font_generation: u64::MAX,
            text_metrics: Vec::new(),
            background: None,
        }
    }

    fn prepare_fonts(&mut self, fonts: &Fonts) {
        let font_generation = fonts.generation();
        if self.font_generation != font_generation {
            self.font_generation = font_generation;
            self.text_metrics.clear();
        }
    }

    fn measure_text<F: Font>(&mut self, font: &F, text: &str, size: f32) -> TextMetrics {
        let font_key = core::ptr::from_ref(font).cast::<()>() as usize;
        let size_bits = size.to_bits();
        if let Some(entry) = self.text_metrics.iter().find(|entry| {
            entry.font_key == font_key && entry.size_bits == size_bits && entry.text == text
        }) {
            return entry.metrics;
        }

        let (width, height, _) = gui_renderer::measure_text(font, text, size);
        let metrics = TextMetrics { width, height };
        if self.text_metrics.len() >= Self::TEXT_MEASURE_CACHE_LIMIT {
            self.text_metrics.clear();
        }
        self.text_metrics.push(TextMeasureCacheEntry {
            font_key,
            text: text.to_string(),
            size_bits,
            metrics,
        });
        metrics
    }

    fn draw_gradient(
        &mut self,
        buffer: &mut [u32],
        width: usize,
        height: usize,
        start_color: u32,
        end_color: u32,
    ) {
        if let Some(cache) = &self.background {
            if cache.start_color == start_color
                && cache.end_color == end_color
                && cache.width == width
                && cache.height == height
                && cache.pixels.len() == buffer.len()
            {
                buffer.copy_from_slice(&cache.pixels);
                return;
            }
        }

        draw_linear_gradient(
            buffer,
            width,
            height,
            start_color,
            end_color,
            (0.0, 0.0),
            (width as f32, height as f32),
        );
        self.background = Some(BackgroundCache {
            start_color,
            end_color,
            width,
            height,
            pixels: buffer.to_vec(),
        });
    }

    #[cfg(test)]
    fn text_measure_cache_len(&self) -> usize {
        self.text_metrics.len()
    }
}

impl Default for RenderCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Mutable ARGB frame surface shared by GUI, WASM, mobile, and UEFI adapters.
pub struct ArgbSurface<'a> {
    width: usize,
    height: usize,
    pixels: &'a mut [u32],
}

impl<'a> ArgbSurface<'a> {
    pub fn new(width: usize, height: usize, pixels: &'a mut [u32]) -> Option<Self> {
        let required = width.checked_mul(height)?;
        if width == 0 || height == 0 || pixels.len() < required {
            return None;
        }

        Some(Self {
            width,
            height,
            pixels: &mut pixels[..required],
        })
    }

    pub fn render(
        &mut self,
        fonts: &Fonts,
        display_settings: DisplaySettings,
        render_list: &[Renderable],
        cache: &mut RenderCache,
    ) {
        cache.prepare_fonts(fonts);
        let viewport = display_settings.viewport(self.width, self.height);
        render_argb(
            self.pixels,
            self.width,
            self.height,
            viewport,
            fonts,
            render_list,
            cache,
        );
    }

    pub fn pixels(&self) -> &[u32] {
        self.pixels
    }
}

fn render_argb(
    pixels: &mut [u32],
    width: usize,
    height: usize,
    viewport: DisplayViewport,
    fonts: &Fonts,
    render_list: &[Renderable],
    cache: &mut RenderCache,
) {
    let mut frame = PixelFrame {
        pixels,
        stride: width,
        frame_height: height,
        viewport,
    };
    frame.pixels.fill(BG_COLOR);
    let has_background = render_list
        .iter()
        .any(|item| matches!(item, Renderable::Background { .. }));
    if !has_background {
        frame.fill_viewport(BG_COLOR);
    }

    for item in render_list {
        match item {
            Renderable::Background { gradient } => {
                frame.draw_gradient(cache, gradient.start_color, gradient.end_color);
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
            } => draw_aligned_text(
                &mut frame,
                fonts,
                fonts.ui(),
                text,
                TextPlacement {
                    anchor: *anchor,
                    shift: *shift,
                    align: *align,
                    font_size: *font_size,
                },
                *color,
                cache,
            ),
            Renderable::TypingUpper {
                segments,
                anchor,
                shift,
                align,
                font_size,
                line_alignment,
            } => draw_typing_upper(
                &mut frame,
                fonts,
                segments,
                TypingUpperPlacement {
                    text: TextPlacement {
                        anchor: *anchor,
                        shift: *shift,
                        align: *align,
                        font_size: *font_size,
                    },
                    line_alignment: *line_alignment,
                },
                cache,
            ),
            Renderable::TypingLower {
                segments,
                anchor,
                shift,
                align,
                font_size,
                line_alignment,
            } => draw_typing_lower(
                &mut frame,
                fonts,
                segments,
                TypingLowerPlacement {
                    text: TextPlacement {
                        anchor: *anchor,
                        shift: *shift,
                        align: *align,
                        font_size: *font_size,
                    },
                    line_alignment: *line_alignment,
                },
                cache,
            ),
            Renderable::ProgressBar {
                anchor,
                shift,
                width_ratio,
                height_ratio,
                progress,
                bg_color,
                fg_color,
            } => draw_progress_bar(
                &mut frame,
                ProgressPlacement {
                    anchor: *anchor,
                    shift: *shift,
                    width_ratio: *width_ratio,
                    height_ratio: *height_ratio,
                    progress: *progress,
                },
                ProgressColors {
                    background: *bg_color,
                    foreground: *fg_color,
                },
            ),
        }
    }
}

struct PixelFrame<'a> {
    pixels: &'a mut [u32],
    stride: usize,
    frame_height: usize,
    viewport: DisplayViewport,
}

impl PixelFrame<'_> {
    fn width(&self) -> usize {
        self.viewport.width
    }

    fn height(&self) -> usize {
        self.viewport.height
    }

    fn scale(&self) -> f32 {
        self.viewport.scale
    }

    fn offset_position(&self, pos: (f32, f32)) -> (f32, f32) {
        (
            pos.0 + self.viewport.x as f32,
            pos.1 + self.viewport.y as f32,
        )
    }

    fn frame_clip(&self) -> PixelClip {
        let left = self.viewport.x.min(self.stride) as i32;
        let top = self.viewport.y.min(self.frame_height) as i32;
        let right = (self.viewport.x + self.viewport.width).min(self.stride) as i32;
        let bottom = (self.viewport.y + self.viewport.height).min(self.frame_height) as i32;
        PixelClip::new(left, top, right, bottom)
    }

    fn local_rect_intersects_viewport(&self, left: f32, top: f32, right: f32, bottom: f32) -> bool {
        right > 0.0 && left < self.width() as f32 && bottom > 0.0 && top < self.height() as f32
    }

    fn draw_text_clipped<F: Font>(
        &mut self,
        font: &F,
        text: &str,
        pos: (f32, f32),
        font_size: f32,
        color: u32,
    ) {
        let frame_pos = self.offset_position(pos);
        let clip = self.frame_clip();
        gui_renderer::draw_text_clipped(
            self.pixels,
            self.stride,
            font,
            text,
            TextDrawOptions::new(frame_pos, font_size, color, clip),
        );
    }

    fn fill_viewport(&mut self, color: u32) {
        for y in self.viewport.y..(self.viewport.y + self.viewport.height).min(self.frame_height) {
            let row_start = y * self.stride;
            let start = row_start + self.viewport.x.min(self.stride);
            let end = row_start + (self.viewport.x + self.viewport.width).min(self.stride);
            self.pixels[start..end].fill(color);
        }
    }

    fn draw_gradient(&mut self, cache: &mut RenderCache, start_color: u32, end_color: u32) {
        if self.viewport.width == 0 || self.viewport.height == 0 {
            return;
        }

        let mut viewport_pixels = vec![0; self.viewport.width * self.viewport.height];
        cache.draw_gradient(
            &mut viewport_pixels,
            self.viewport.width,
            self.viewport.height,
            start_color,
            end_color,
        );

        for row in 0..self.viewport.height {
            let dest_y = self.viewport.y + row;
            if dest_y >= self.frame_height {
                break;
            }
            let dest_start = dest_y * self.stride + self.viewport.x;
            let dest_end = dest_start + self.viewport.width.min(self.stride - self.viewport.x);
            let src_start = row * self.viewport.width;
            let src_end = src_start + (dest_end - dest_start);
            self.pixels[dest_start..dest_end].copy_from_slice(&viewport_pixels[src_start..src_end]);
        }
    }
}

#[derive(Clone, Copy)]
struct TextPlacement {
    anchor: ui::Anchor,
    shift: ui::Shift,
    align: ui::Align,
    font_size: FontSize,
}

#[derive(Clone, Copy)]
struct TypingUpperPlacement {
    text: TextPlacement,
    line_alignment: ui::TypingLineAlignment,
}

#[derive(Clone, Copy)]
struct TypingLowerPlacement {
    text: TextPlacement,
    line_alignment: ui::TypingLineAlignment,
}

#[derive(Clone, Copy)]
struct ProgressPlacement {
    anchor: ui::Anchor,
    shift: ui::Shift,
    width_ratio: f32,
    height_ratio: f32,
    progress: f32,
}

#[derive(Clone, Copy)]
struct ProgressColors {
    background: u32,
    foreground: u32,
}

fn draw_text_if_visible<F: Font>(
    frame: &mut PixelFrame<'_>,
    font: &F,
    text: &str,
    pos: (f32, f32),
    size: f32,
    color: u32,
    metrics: TextMetrics,
) {
    if metrics.width == 0 || metrics.height == 0 {
        return;
    }

    let (x, y) = pos;
    if frame.local_rect_intersects_viewport(
        x,
        y - size * 0.2,
        x + metrics.width as f32,
        y + metrics.height as f32 + size * 0.2,
    ) {
        frame.draw_text_clipped(font, text, pos, size, color);
    }
}

fn draw_aligned_text<F: Font>(
    frame: &mut PixelFrame<'_>,
    fonts: &Fonts,
    font: &F,
    text: &str,
    placement: TextPlacement,
    color: u32,
    cache: &mut RenderCache,
) {
    let pixel_font_size =
        calculate_pixel_font_size(placement.font_size, frame.width(), frame.height())
            * frame.scale();
    let pixel_font_size = fonts.scaled_size_for_ui(pixel_font_size);
    let metrics = cache.measure_text(font, text, pixel_font_size);
    let anchor_pos = ui::calculate_anchor_position(
        placement.anchor,
        placement.shift,
        frame.width(),
        frame.height(),
    );
    let (x, y) =
        ui::calculate_aligned_position(anchor_pos, metrics.width, metrics.height, placement.align);
    draw_text_if_visible(
        frame,
        font,
        text,
        (x as f32, y as f32),
        pixel_font_size,
        color,
        metrics,
    );
}

fn draw_typing_upper(
    frame: &mut PixelFrame<'_>,
    fonts: &Fonts,
    segments: &[ui::UpperTypingSegment],
    placement: TypingUpperPlacement,
    cache: &mut RenderCache,
) {
    let pixel_font_size =
        calculate_pixel_font_size(placement.text.font_size, frame.width(), frame.height())
            * frame.scale();
    let segment_widths: Vec<u32> = segments
        .iter()
        .map(|segment| {
            let font = fonts.get_for_script(segment.script);
            let size = fonts.scaled_size_for_script(segment.script, pixel_font_size);
            cache.measure_text(font, &segment.base_text, size).width
        })
        .collect();
    let total_height = upper_typing_total_height(fonts, segments, pixel_font_size, cache);
    let anchor_pos = ui::calculate_anchor_position(
        placement.text.anchor,
        placement.text.shift,
        frame.width(),
        frame.height(),
    );
    let (mut pen_x, y) = ui::calculate_aligned_position(
        anchor_pos,
        placement.line_alignment.full_line_width,
        total_height,
        placement.text.align,
    );
    pen_x += placement.line_alignment.visible_start_width as i32;

    for (segment, segment_width) in segments.iter().zip(segment_widths.iter().copied()) {
        let color = upper_segment_color(segment.state);
        let font = fonts.get_for_script(segment.script);
        let base_size = fonts.scaled_size_for_script(segment.script, pixel_font_size);
        let base_metrics = TextMetrics {
            width: segment_width,
            height: total_height,
        };
        draw_text_if_visible(
            frame,
            font,
            &segment.base_text,
            (pen_x as f32, y as f32),
            base_size,
            color,
            base_metrics,
        );

        if let Some(ruby) = &segment.ruby_text {
            let ruby_font = fonts.get_ruby_for_script(segment.script);
            let ruby_pixel_font_size =
                fonts.scaled_size_for_ruby_script(segment.script, pixel_font_size * 0.4);
            let ruby_metrics = cache.measure_text(ruby_font, ruby, ruby_pixel_font_size);
            let ruby_width = ruby_metrics.width;
            let ruby_x = pen_x as f32 + (segment_width as f32 - ruby_width as f32) / 2.0;
            let ruby_y = y as f32 - ruby_pixel_font_size * 0.5;
            draw_text_if_visible(
                frame,
                ruby_font,
                ruby,
                (ruby_x, ruby_y),
                ruby_pixel_font_size,
                color,
                ruby_metrics,
            );
        }

        pen_x += segment_width as i32;
    }
}

fn upper_typing_total_height(
    fonts: &Fonts,
    segments: &[ui::UpperTypingSegment],
    pixel_font_size: f32,
    cache: &mut RenderCache,
) -> u32 {
    let fallback = cache
        .measure_text(fonts.primary(), " ", pixel_font_size)
        .height;
    segments
        .iter()
        .map(|segment| {
            let base_size = fonts.scaled_size_for_script(segment.script, pixel_font_size);
            let ruby_size =
                fonts.scaled_size_for_ruby_script(segment.script, pixel_font_size * 0.4);
            let base_height = cache
                .measure_text(fonts.get_for_script(segment.script), " ", base_size)
                .height as f32;
            let ruby_text = segment.ruby_text.as_deref().unwrap_or(" ");
            let ruby_height = cache
                .measure_text(
                    fonts.get_ruby_for_script(segment.script),
                    ruby_text,
                    ruby_size,
                )
                .height as f32;
            let ruby_y = -ruby_size * 0.5;
            (-ruby_y + base_height.max(ruby_y + ruby_height)).ceil() as u32
        })
        .max()
        .unwrap_or(fallback)
}

fn draw_typing_lower(
    frame: &mut PixelFrame<'_>,
    fonts: &Fonts,
    segments: &[LowerTypingSegment],
    placement: TypingLowerPlacement,
    cache: &mut RenderCache,
) {
    let pixel_font_size =
        calculate_pixel_font_size(placement.text.font_size, frame.width(), frame.height())
            * frame.scale();
    let total_height = lower_typing_total_height(fonts, segments, pixel_font_size, cache);
    let anchor_pos = ui::calculate_anchor_position(
        placement.text.anchor,
        placement.text.shift,
        frame.width(),
        frame.height(),
    );
    let (mut pen_x, y) = ui::calculate_aligned_position(
        anchor_pos,
        placement.line_alignment.full_line_width,
        total_height,
        placement.text.align,
    );
    pen_x += placement.line_alignment.visible_start_width as i32;

    for segment in segments {
        match segment {
            LowerTypingSegment::Completed {
                base_text,
                ruby_text,
                script,
                is_correct,
                width: segment_width,
            } => {
                let font = fonts.get_for_script(*script);
                let base_size = fonts.scaled_size_for_script(*script, pixel_font_size);
                let color = if *is_correct {
                    ui::CORRECT_COLOR
                } else {
                    ui::INCORRECT_COLOR
                };
                let segment_width_px = *segment_width as i32;
                if segment_width_px > 0 {
                    let base_metrics = TextMetrics {
                        width: *segment_width,
                        height: total_height,
                    };
                    draw_text_if_visible(
                        frame,
                        font,
                        base_text,
                        (pen_x as f32, y as f32),
                        base_size,
                        color,
                        base_metrics,
                    );

                    if let Some(ruby) = ruby_text {
                        let ruby_font = fonts.get_ruby_for_script(*script);
                        let ruby_pixel_font_size =
                            fonts.scaled_size_for_ruby_script(*script, pixel_font_size * 0.3);
                        let ruby_metrics =
                            cache.measure_text(ruby_font, ruby, ruby_pixel_font_size);
                        if ruby_metrics.width > 0 {
                            let ruby_x = pen_x as f32
                                + (*segment_width as f32 - ruby_metrics.width as f32) / 2.0;
                            let ruby_y = y as f32 - ruby_pixel_font_size * 0.5;
                            draw_text_if_visible(
                                frame,
                                ruby_font,
                                ruby,
                                (ruby_x, ruby_y),
                                ruby_pixel_font_size,
                                color,
                                ruby_metrics,
                            );
                        }
                    }
                }
                pen_x += segment_width_px;
            }
            LowerTypingSegment::Active { elements, script } => {
                for element in elements {
                    let (text, color, element_script) =
                        active_lower_text_and_color(element, *script);
                    let (font, size) = match element {
                        ActiveLowerElement::UnconfirmedInput { .. } => (
                            fonts.get_unconfirmed_for_script(element_script),
                            fonts.scaled_size_for_unconfirmed_script(
                                element_script,
                                pixel_font_size,
                            ),
                        ),
                        _ => (
                            fonts.get_for_script(element_script),
                            fonts.scaled_size_for_script(element_script, pixel_font_size),
                        ),
                    };
                    let text_metrics = cache.measure_text(font, &text, size);
                    let text_width = text_metrics.width as i32;
                    if text_width > 0 {
                        draw_text_if_visible(
                            frame,
                            font,
                            &text,
                            (pen_x as f32, y as f32),
                            size,
                            color,
                            text_metrics,
                        );
                    }
                    pen_x += text_width;
                }
            }
        }
    }
}

fn lower_typing_total_height(
    fonts: &Fonts,
    segments: &[LowerTypingSegment],
    pixel_font_size: f32,
    cache: &mut RenderCache,
) -> u32 {
    let fallback = cache
        .measure_text(fonts.primary(), " ", pixel_font_size)
        .height;
    segments
        .iter()
        .map(|segment| {
            let script = match segment {
                LowerTypingSegment::Completed { script, .. } => *script,
                LowerTypingSegment::Active { script, .. } => *script,
            };
            let base_size = fonts.scaled_size_for_script(script, pixel_font_size);
            let ruby_size = fonts.scaled_size_for_ruby_script(script, pixel_font_size * 0.3);
            let mut base_height = cache
                .measure_text(fonts.get_for_script(script), " ", base_size)
                .height as f32;
            let ruby_text = match segment {
                LowerTypingSegment::Completed { ruby_text, .. } => {
                    ruby_text.as_deref().unwrap_or(" ")
                }
                LowerTypingSegment::Active { .. } => " ",
            };
            let ruby_height = cache
                .measure_text(fonts.get_ruby_for_script(script), ruby_text, ruby_size)
                .height as f32;
            if let LowerTypingSegment::Active { elements, .. } = segment {
                for element in elements {
                    match element {
                        ActiveLowerElement::Typed {
                            character, script, ..
                        }
                        | ActiveLowerElement::LastIncorrectInput { character, script } => {
                            let size = fonts.scaled_size_for_script(*script, pixel_font_size);
                            let mut text = String::new();
                            text.push(*character);
                            let height = cache
                                .measure_text(fonts.get_for_script(*script), &text, size)
                                .height as f32;
                            base_height = base_height.max(height);
                        }
                        ActiveLowerElement::Cursor => {
                            let size = fonts.scaled_size_for_script(script, pixel_font_size);
                            let height = cache
                                .measure_text(fonts.get_for_script(script), "|", size)
                                .height as f32;
                            base_height = base_height.max(height);
                        }
                        ActiveLowerElement::UnconfirmedInput { text, script } => {
                            let size =
                                fonts.scaled_size_for_unconfirmed_script(*script, pixel_font_size);
                            let height = cache
                                .measure_text(fonts.get_unconfirmed_for_script(*script), text, size)
                                .height as f32;
                            base_height = base_height.max(height);
                        }
                    }
                }
            }
            let ruby_y = -ruby_size * 0.5;
            (-ruby_y + base_height.max(ruby_y + ruby_height)).ceil() as u32
        })
        .max()
        .unwrap_or(fallback)
}

fn draw_progress_bar(
    frame: &mut PixelFrame<'_>,
    placement: ProgressPlacement,
    colors: ProgressColors,
) {
    let bar_width = (frame.width() as f32 * placement.width_ratio) as u32;
    let bar_height = (frame.height() as f32 * placement.height_ratio) as u32;
    let anchor_pos = ui::calculate_anchor_position(
        placement.anchor,
        placement.shift,
        frame.width(),
        frame.height(),
    );
    let start_x = frame.viewport.x + anchor_pos.0.max(0) as usize;
    let start_y = frame.viewport.y + (anchor_pos.1 - bar_height as i32).max(0) as usize;

    gui_renderer::draw_rect(
        frame.pixels,
        frame.stride,
        start_x,
        start_y,
        bar_width as usize,
        bar_height as usize,
        colors.background,
    );

    let fg_width = (bar_width as f32 * placement.progress.clamp(0.0, 1.0)) as usize;
    if fg_width > 0 {
        gui_renderer::draw_rect(
            frame.pixels,
            frame.stride,
            start_x,
            start_y,
            fg_width,
            bar_height as usize,
            colors.foreground,
        );
    }
}

fn upper_segment_color(state: UpperSegmentState) -> u32 {
    match state {
        UpperSegmentState::Correct => ui::CORRECT_COLOR,
        UpperSegmentState::Incorrect => ui::INCORRECT_COLOR,
        UpperSegmentState::Active => ui::ACTIVE_COLOR,
        UpperSegmentState::Pending => ui::PENDING_COLOR,
        UpperSegmentState::Muted => 0xFF_444444,
    }
}

fn active_lower_text_and_color(
    element: &ActiveLowerElement,
    fallback_script: crate::font::FontScript,
) -> (String, u32, crate::font::FontScript) {
    match element {
        ActiveLowerElement::Typed {
            character,
            is_correct,
            script,
        } => (
            character.to_string(),
            if *is_correct {
                ui::CORRECT_COLOR
            } else {
                ui::INCORRECT_COLOR
            },
            *script,
        ),
        ActiveLowerElement::Cursor => ("|".to_string(), ui::CURSOR_COLOR, fallback_script),
        ActiveLowerElement::UnconfirmedInput { text, script } => {
            (text.clone(), ui::UNCONFIRMED_COLOR, *script)
        }
        ActiveLowerElement::LastIncorrectInput { character, script } => {
            (character.to_string(), ui::WRONG_KEY_COLOR, *script)
        }
    }
}

/// GUI/WASMバックエンド用のピクセルベースレンダラ
pub mod gui_renderer {
    use super::*;

    /// 指定されたピクセルバッファの指定位置にテキストを描画する
    pub fn draw_text<F: Font>(
        buffer: &mut [u32],
        stride: usize,
        font: &F,
        text: &str,
        pos: (f32, f32),
        font_size: f32,
        color: u32,
    ) {
        if stride == 0 {
            return;
        }
        let height = buffer.len() / stride;
        draw_text_clipped(
            buffer,
            stride,
            font,
            text,
            TextDrawOptions::new(pos, font_size, color, PixelClip::for_buffer(stride, height)),
        );
    }

    /// 指定されたclip内に収まるピクセルだけにテキストを描画する
    pub(super) fn draw_text_clipped<F: Font>(
        buffer: &mut [u32],
        stride: usize,
        font: &F,
        text: &str,
        options: TextDrawOptions,
    ) {
        if stride == 0 || options.clip.is_empty() {
            return;
        }

        let scale = PxScale::from(options.font_size);
        let scaled_font = font.as_scaled(scale);
        let ascent = scaled_font.ascent();
        let mut pen_x = options.pos.0;
        let pen_y = options.pos.1 + ascent;

        let mut last_glyph = None;
        for character in text.chars() {
            let glyph_id = font.glyph_id(character);
            if let Some(last) = last_glyph {
                pen_x += scaled_font.kern(last, glyph_id);
            }
            let glyph = glyph_id.with_scale_and_position(scale, point(pen_x, pen_y));
            if let Some(outlined) = font.outline_glyph(glyph) {
                let bounds = outlined.px_bounds();
                if options.clip.intersects_f32(
                    bounds.min.x,
                    bounds.min.y,
                    bounds.max.x,
                    bounds.max.y,
                ) {
                    draw_glyph_to_pixel_buffer(
                        buffer,
                        stride,
                        &outlined,
                        options.color,
                        options.clip,
                    );
                }
            }
            pen_x += scaled_font.h_advance(glyph_id);
            last_glyph = Some(glyph_id);
        }
    }

    /// アウトライン化されたグリフをピクセルバッファに描画する（内部関数）
    fn draw_glyph_to_pixel_buffer(
        buffer: &mut [u32],
        stride: usize,
        outlined: &OutlinedGlyph,
        color: u32,
        clip: PixelClip,
    ) {
        let bounds = outlined.px_bounds();
        // バッファ高さとテキスト色チャンネルをクロージャ外で一度だけ計算する。
        // これらはグリフの全ピクセルで共通の値であり、内側で計算すると
        // ピクセルごとに無駄な除算・シフト・キャストが繰り返されていた。
        let buf_height = buffer.len() / stride;
        let text_r = ((color >> 16) & 0xFF) as f32;
        let text_g = ((color >> 8) & 0xFF) as f32;
        let text_b = (color & 0xFF) as f32;

        outlined.draw(|x, y, c| {
            // カバレッジがほぼゼロなら背景が透けて見えるだけなのでスキップする。
            // アンチエイリアス端部には c ≈ 0 のピクセルが多く、分岐コストを補って余りある。
            if c < 0.004 {
                return;
            }

            let buffer_x = bounds.min.x as i32 + x as i32;
            let buffer_y = bounds.min.y as i32 + y as i32;
            if buffer_x < clip.left
                || buffer_x >= clip.right
                || buffer_y < clip.top
                || buffer_y >= clip.bottom
                || buffer_x < 0
                || buffer_x >= stride as i32
                || buffer_y < 0
                || buffer_y >= buf_height as i32
            {
                return;
            }
            let index = (buffer_y as usize) * stride + (buffer_x as usize);

            // カバレッジがほぼ 1.0 なら背景を読まずに直接書き込む高速パス
            if c > 0.996 {
                buffer[index] = (0xFF << 24) | color;
                return;
            }

            let c_inv = 1.0 - c;
            let bg = buffer[index];
            let bg_r = ((bg >> 16) & 0xFF) as f32;
            let bg_g = ((bg >> 8) & 0xFF) as f32;
            let bg_b = (bg & 0xFF) as f32;
            let r = (text_r * c + bg_r * c_inv) as u32;
            let g = (text_g * c + bg_g * c_inv) as u32;
            let b = (text_b * c + bg_b * c_inv) as u32;
            buffer[index] = (0xFF << 24) | (r << 16) | (g << 8) | b;
        });
    }

    /// ピクセルバッファに単色の矩形を描画する
    pub fn draw_rect(
        buffer: &mut [u32],
        width: usize,
        rect_x: usize,
        rect_y: usize,
        rect_w: usize,
        rect_h: usize,
        color: u32,
    ) {
        let height = buffer.len() / width;

        for y in rect_y..(rect_y + rect_h).min(height) {
            let row_start = y * width;
            let row_end = (rect_x + rect_w).min(width);
            for x in rect_x..row_end {
                let index = row_start + x;
                buffer[index] = color;
            }
        }
    }

    /// テキストの描画サイズ（幅と高さ）を計算する
    pub fn measure_text<F: Font>(font: &F, text: &str, size: f32) -> (u32, u32, f32) {
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
}

/// TUIバックエンド用の文字ベースレンダラ
pub mod tui_renderer {
    use super::*;
    #[cfg(not(feature = "uefi"))]
    use std::convert::TryFrom;

    // TUIの1文字の縦横比をおよそ2:1と仮定
    pub const TUI_CHAR_ASPECT_RATIO: f32 = 2.0;
    // アートの1セルを構成する仮想ピクセル数。小さいほど高解像度（大きく）なる
    pub const ART_V_PIXELS_PER_CELL: f32 = 2.0;

    /// 指定されたテキストをASCIIアート化し、(文字バッファ, 幅, 高さ, アセント)を返す
    pub fn render_text_to_art<F: Font>(
        font: &F,
        text: &str,
        font_size_px: f32,
    ) -> (Vec<char>, usize, usize, usize) {
        if text.is_empty() {
            return (Vec::new(), 0, 0, 0);
        }

        let scale = PxScale::from(font_size_px);
        let scaled_font = font.as_scaled(scale);
        let ascent = scaled_font.ascent();

        // アート全体のピクセル単位でのバウンディングボックスを計算
        let mut min_x = f32::MAX;
        let mut max_x = f32::MIN;
        let mut min_y = f32::MAX;
        let mut max_y = f32::MIN;
        let mut pen_x = 0.0;
        let mut last_glyph = None;

        for c in text.chars() {
            let glyph_id = font.glyph_id(c);
            if let Some(last) = last_glyph {
                pen_x += scaled_font.kern(last, glyph_id);
            }
            if let Some(outlined) = font.outline_glyph(glyph_id.with_scale(scale)) {
                let bounds = outlined.px_bounds();
                min_x = min_x.min(pen_x + bounds.min.x);
                max_x = max_x.max(pen_x + bounds.max.x);
                min_y = min_y.min(ascent + bounds.min.y);
                max_y = max_y.max(ascent + bounds.max.y);
            }
            pen_x += scaled_font.h_advance(glyph_id);
            last_glyph = Some(glyph_id);
        }
        max_x = max_x.max(pen_x); // 最後の文字の右端も考慮

        if min_x > max_x {
            // テキストに描画可能なグリフがなかった場合
            return (Vec::new(), 0, 0, 0);
        }

        let art_cell_height = ART_V_PIXELS_PER_CELL;
        let art_cell_width = art_cell_height / TUI_CHAR_ASPECT_RATIO;

        let art_width = ((max_x - min_x) / art_cell_width).ceil() as usize;
        let art_height = ((max_y - min_y) / art_cell_height).ceil() as usize;

        if art_width == 0 || art_height == 0 {
            return (Vec::new(), 0, 0, 0);
        }

        // --- アセント計算 ---
        let ascent_in_pixels = ascent - min_y;
        let ascent_in_cells = (ascent_in_pixels / art_cell_height).floor().max(0.0) as usize;
        // --- ここまで ---

        let mut coverage_buffer = vec![0.0f32; art_width * art_height];

        // グリフを描画し、各セルのカバレッジを計算
        pen_x = 0.0;
        last_glyph = None;
        for c in text.chars() {
            let glyph_id = font.glyph_id(c);
            if let Some(last) = last_glyph {
                pen_x += scaled_font.kern(last, glyph_id);
            }
            let glyph = glyph_id.with_scale_and_position(scale, point(pen_x, ascent));
            if let Some(outlined) = font.outline_glyph(glyph) {
                let bounds = outlined.px_bounds();
                outlined.draw(|x, y, v| {
                    let px = bounds.min.x + x as f32 - min_x;
                    let py = bounds.min.y + y as f32 - min_y;

                    let cell_x = (px / art_cell_width) as i32;
                    let cell_y = (py / art_cell_height) as i32;

                    if cell_x >= 0
                        && cell_x < art_width as i32
                        && cell_y >= 0
                        && cell_y < art_height as i32
                    {
                        let index = cell_y as usize * art_width + cell_x as usize;
                        coverage_buffer[index] = (coverage_buffer[index] + v).min(1.0);
                    }
                });
            }
            pen_x += scaled_font.h_advance(glyph_id);
            last_glyph = Some(glyph_id);
        }

        // カバレッジを文字に変換
        let char_buffer = coverage_buffer
            .into_iter()
            .map(|c| match (c * 4.99) as u8 {
                0 => ' ',
                1 => '.',
                2 => '*',
                3 => '#',
                _ => '@',
            })
            .collect();

        (char_buffer, art_width, art_height, ascent_in_cells)
    }

    /// 指定されたテキストを点字アート化し、(文字バッファ, 幅, 高さ, アセント)を返す
    pub fn render_text_to_braille_art<F: Font>(
        font: &F,
        text: &str,
        font_size_px: f32,
    ) -> (Vec<char>, usize, usize, usize) {
        if text.is_empty() {
            return (Vec::new(), 0, 0, 0);
        }

        let scale = PxScale::from(font_size_px);
        let scaled_font = font.as_scaled(scale);
        let ascent = scaled_font.ascent();

        // アート全体のピクセル単位でのバウンディングボックスを計算 (ASCIIアート版と同じ)
        let mut min_x = f32::MAX;
        let mut max_x = f32::MIN;
        let mut min_y = f32::MAX;
        let mut max_y = f32::MIN;
        let mut pen_x = 0.0;
        let mut last_glyph = None;

        for c in text.chars() {
            let glyph_id = font.glyph_id(c);
            if let Some(last) = last_glyph {
                pen_x += scaled_font.kern(last, glyph_id);
            }
            if let Some(outlined) = font.outline_glyph(glyph_id.with_scale(scale)) {
                let bounds = outlined.px_bounds();
                min_x = min_x.min(pen_x + bounds.min.x);
                max_x = max_x.max(pen_x + bounds.max.x);
                min_y = min_y.min(ascent + bounds.min.y);
                max_y = max_y.max(ascent + bounds.max.y);
            }
            pen_x += scaled_font.h_advance(glyph_id);
            last_glyph = Some(glyph_id);
        }
        max_x = max_x.max(pen_x);

        if min_x > max_x {
            return (Vec::new(), 0, 0, 0);
        }

        // 点字は 4x2 のグリッド。1文字セル(高さ=幅*2)の比率に合わせる
        let art_cell_height = 4.0;
        // FIX: アスペクト比の計算を修正
        let art_cell_width = art_cell_height / TUI_CHAR_ASPECT_RATIO;

        let art_width = ((max_x - min_x) / art_cell_width).ceil() as usize;
        let art_height = ((max_y - min_y) / art_cell_height).ceil() as usize;

        if art_width == 0 || art_height == 0 {
            return (Vec::new(), 0, 0, 0);
        }

        let ascent_in_pixels = ascent - min_y;
        let ascent_in_cells = (ascent_in_pixels / art_cell_height).floor().max(0.0) as usize;

        // グリフのピクセルカバレッジを計算するための高解像度バッファ
        // 点字の各ドットに対応させるため、TUIセルの2x4倍の解像度にする
        let sub_w = art_width * 2;
        let sub_h = art_height * 4;
        let mut sub_pixel_buffer = vec![0.0f32; sub_w * sub_h];

        pen_x = 0.0;
        last_glyph = None;
        for c in text.chars() {
            let glyph_id = font.glyph_id(c);
            if let Some(last) = last_glyph {
                pen_x += scaled_font.kern(last, glyph_id);
            }
            let glyph = glyph_id.with_scale_and_position(scale, point(pen_x, ascent));
            if let Some(outlined) = font.outline_glyph(glyph) {
                let bounds = outlined.px_bounds();
                outlined.draw(|x, y, v| {
                    let px = bounds.min.x + x as f32 - min_x;
                    let py = bounds.min.y + y as f32 - min_y;

                    // 高解像度バッファのどのサブピクセルに対応するか計算
                    let sub_x = (px / art_cell_width * 2.0) as i32;
                    let sub_y = (py / art_cell_height * 4.0) as i32;

                    if sub_x >= 0 && sub_x < sub_w as i32 && sub_y >= 0 && sub_y < sub_h as i32 {
                        let index = sub_y as usize * sub_w + sub_x as usize;
                        sub_pixel_buffer[index] = (sub_pixel_buffer[index] + v).min(1.0);
                    }
                });
            }
            pen_x += scaled_font.h_advance(glyph_id);
            last_glyph = Some(glyph_id);
        }

        // 高解像度バッファから点字文字バッファを生成
        let mut char_buffer = Vec::with_capacity(art_width * art_height);
        // 点字ドットとビットのマッピング
        // 1 • • 4  -> bit 0, 3
        // 2 • • 5  -> bit 1, 4
        // 3 • • 6  -> bit 2, 5
        // 7 • • 8  -> bit 6, 7
        const BIT_MAP: [[u8; 2]; 4] = [[0, 3], [1, 4], [2, 5], [6, 7]];

        for y in 0..art_height {
            for x in 0..art_width {
                let mut braille_byte: u32 = 0;
                // 2x4 のサブピクセルをチェック
                for (dy, row) in BIT_MAP.iter().enumerate() {
                    for (dx, bit) in row.iter().enumerate() {
                        let sub_x = x * 2 + dx;
                        let sub_y = y * 4 + dy;
                        let index = sub_y * sub_w + sub_x;
                        if sub_pixel_buffer[index] > 0.3 {
                            // カバレッジの閾値
                            braille_byte |= 1u32 << u32::from(*bit);
                        }
                    }
                }
                // Unicodeの点字パターンは U+2800 から始まる
                let braille_char = char::try_from(0x2800 + braille_byte).unwrap_or(' ');
                char_buffer.push(braille_char);
            }
        }

        (char_buffer, art_width, art_height, ascent_in_cells)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::{DisplayAspectRatio, DisplaySettings};
    use crate::font::{FontBundle, Fonts};
    use crate::ui::{Align, Anchor, FontSize, HorizontalAlign, Renderable, Shift, VerticalAlign};
    use ab_glyph::FontVec;

    fn test_font() -> FontVec {
        FontVec::try_from_vec(include_bytes!("../fonts/YujiSyuku-Regular.ttf").to_vec())
            .expect("test font should parse")
    }

    fn test_fonts() -> Fonts {
        Fonts::new(FontBundle {
            ui: test_font(),
            japanese: test_font(),
            japanese_ruby: test_font(),
            japanese_unconfirmed: test_font(),
            chinese_simplified: test_font(),
            chinese_simplified_ruby: test_font(),
            chinese_simplified_unconfirmed: test_font(),
            traditional_chinese: test_font(),
            traditional_chinese_ruby: test_font(),
            traditional_chinese_unconfirmed: test_font(),
            english: test_font(),
        })
    }

    #[test]
    fn argb_surface_renders_and_reuses_text_measurements() {
        let fonts = test_fonts();
        let render_list = vec![
            Renderable::Background {
                gradient: crate::ui::Gradient {
                    start_color: 0xFF_220000,
                    end_color: 0xFF_000022,
                },
            },
            Renderable::Text {
                text: "Test".to_string(),
                anchor: Anchor::Center,
                shift: Shift { x: 0.0, y: 0.0 },
                align: Align {
                    horizontal: HorizontalAlign::Center,
                    vertical: VerticalAlign::Center,
                },
                font_size: FontSize::WindowHeight(0.2),
                color: 0xFF_FFFFFF,
            },
        ];
        let mut cache = RenderCache::new();
        let mut pixels = vec![0u32; 160 * 90];

        {
            let mut surface =
                ArgbSurface::new(160, 90, &mut pixels).expect("surface dimensions should be valid");
            surface.render(&fonts, DisplaySettings::default(), &render_list, &mut cache);
        }
        let cache_len_after_first_render = cache.text_measure_cache_len();
        assert!(cache_len_after_first_render > 0);
        assert!(pixels.iter().any(|pixel| *pixel != 0));

        {
            let mut surface =
                ArgbSurface::new(160, 90, &mut pixels).expect("surface dimensions should be valid");
            surface.render(&fonts, DisplaySettings::default(), &render_list, &mut cache);
        }
        assert_eq!(cache.text_measure_cache_len(), cache_len_after_first_render);
    }

    #[test]
    fn argb_surface_clips_text_to_display_viewport() {
        let fonts = test_fonts();
        let render_list = vec![
            Renderable::Background {
                gradient: crate::ui::Gradient {
                    start_color: 0xFF_222222,
                    end_color: 0xFF_222222,
                },
            },
            Renderable::Text {
                text: "WWWW".to_string(),
                anchor: Anchor::TopLeft,
                shift: Shift { x: -0.45, y: 0.25 },
                align: Align {
                    horizontal: HorizontalAlign::Left,
                    vertical: VerticalAlign::Top,
                },
                font_size: FontSize::WindowHeight(0.3),
                color: 0xFF_FFFFFF,
            },
        ];
        let settings = DisplaySettings {
            aspect_ratio: DisplayAspectRatio::Square1x1,
            scale: crate::display::DisplayScale::Percent100,
        };
        let mut cache = RenderCache::new();
        let mut pixels = vec![0u32; 200 * 100];

        {
            let mut surface = ArgbSurface::new(200, 100, &mut pixels)
                .expect("surface dimensions should be valid");
            surface.render(&fonts, settings, &render_list, &mut cache);
        }

        for y in 0..100 {
            for x in 0..50 {
                assert_eq!(
                    pixels[y * 200 + x],
                    BG_COLOR,
                    "left letterbox pixel ({x},{y}) should stay clipped"
                );
            }
        }
    }
}
