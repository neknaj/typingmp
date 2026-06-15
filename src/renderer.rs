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

    let colors = GradientColors::new(start_color, end_color);
    for y in 0..height {
        for x in 0..width {
            let p_x = x as f32;
            let p_y = y as f32;
            let dot_product = (p_x - x0) * dx + (p_y - y0) * dy;
            let ratio = gradient_ratio(dot_product, len_sq);
            buffer[y * width + x] = colors.interpolate(ratio);
        }
    }
}

#[derive(Clone, Copy)]
struct GradientColors {
    start_r: f32,
    start_g: f32,
    start_b: f32,
    end_r: f32,
    end_g: f32,
    end_b: f32,
}

impl GradientColors {
    fn new(start_color: u32, end_color: u32) -> Self {
        Self {
            start_r: ((start_color >> 16) & 0xFF) as f32,
            start_g: ((start_color >> 8) & 0xFF) as f32,
            start_b: (start_color & 0xFF) as f32,
            end_r: ((end_color >> 16) & 0xFF) as f32,
            end_g: ((end_color >> 8) & 0xFF) as f32,
            end_b: (end_color & 0xFF) as f32,
        }
    }

    fn interpolate(self, ratio: f32) -> u32 {
        let inverse = 1.0 - ratio;
        let r = (self.start_r * inverse + self.end_r * ratio) as u32;
        let g = (self.start_g * inverse + self.end_g * ratio) as u32;
        let b = (self.start_b * inverse + self.end_b * ratio) as u32;
        (0xFF << 24) | (r << 16) | (g << 8) | b
    }
}

fn gradient_ratio(dot_product: f32, len_sq: f32) -> f32 {
    if len_sq == 0.0 {
        0.0
    } else {
        (dot_product / len_sq).clamp(0.0, 1.0)
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
    hash: usize,
    metrics: TextMetrics,
}

struct GradientCache {
    frame_width: usize,
    frame_height: usize,
    viewport: DisplayViewport,
    start_color: u32,
    end_color: u32,
    pixels: Vec<u32>,
}

struct TextBitmapCacheEntry {
    font_key: usize,
    text: String,
    size_bits: u32,
    frac_x_key: u8,
    frac_y_key: u8,
    hash: usize,
    bitmap: RasterizedText,
}

struct RasterizedText {
    offset_x: i32,
    offset_y: i32,
    width: usize,
    height: usize,
    alpha: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FrameCacheKey {
    buffer_ptr: usize,
    buffer_len: usize,
    width: usize,
    height: usize,
    viewport_x: i32,
    viewport_y: i32,
    viewport_width: usize,
    viewport_height: usize,
    viewport_scale_bits: u32,
    font_generation: u64,
    render_hash: u64,
}

/// Shared render cache used by pixel backends.
pub struct RenderCache {
    font_generation: u64,
    text_metrics: Vec<Option<TextMeasureCacheEntry>>,
    text_metric_count: usize,
    gradient: Option<GradientCache>,
    text_bitmaps: Vec<Option<TextBitmapCacheEntry>>,
    text_bitmap_count: usize,
    text_bitmap_bytes: usize,
    previous_frame: Option<FrameCacheKey>,
    #[cfg(test)]
    frame_cache_hits: usize,
}

impl RenderCache {
    const TEXT_MEASURE_CACHE_LIMIT: usize = 2048;
    const TEXT_MEASURE_CACHE_PROBE_LIMIT: usize = 4;
    const TEXT_BITMAP_CACHE_LIMIT: usize = 512;
    const TEXT_BITMAP_CACHE_PROBE_LIMIT: usize = 4;
    const TEXT_BITMAP_CACHE_MAX_BYTES: usize = 4 * 1024 * 1024;
    const TEXT_BITMAP_MAX_SINGLE_BYTES: usize = 512 * 1024;
    const SUBPIXEL_BUCKETS: f32 = 64.0;

    pub const fn new() -> Self {
        Self {
            font_generation: u64::MAX,
            text_metrics: Vec::new(),
            text_metric_count: 0,
            gradient: None,
            text_bitmaps: Vec::new(),
            text_bitmap_count: 0,
            text_bitmap_bytes: 0,
            previous_frame: None,
            #[cfg(test)]
            frame_cache_hits: 0,
        }
    }

    fn prepare_fonts(&mut self, fonts: &Fonts) {
        let font_generation = fonts.generation();
        if self.font_generation != font_generation {
            self.font_generation = font_generation;
            self.text_metrics.clear();
            self.text_metric_count = 0;
            self.clear_text_bitmaps();
            self.previous_frame = None;
        }
    }

    fn ensure_text_metric_slots(&mut self) {
        if self.text_metrics.len() != Self::TEXT_MEASURE_CACHE_LIMIT {
            self.text_metrics.clear();
            self.text_metrics
                .resize_with(Self::TEXT_MEASURE_CACHE_LIMIT, || None);
            self.text_metric_count = 0;
        }
    }

    fn ensure_text_bitmap_slots(&mut self) {
        if self.text_bitmaps.len() != Self::TEXT_BITMAP_CACHE_LIMIT {
            self.clear_text_bitmaps();
            self.text_bitmaps
                .resize_with(Self::TEXT_BITMAP_CACHE_LIMIT, || None);
        }
    }

    fn clear_text_bitmaps(&mut self) {
        self.text_bitmaps.clear();
        self.text_bitmap_count = 0;
        self.text_bitmap_bytes = 0;
    }

    fn measure_text<F: Font>(&mut self, font: &F, text: &str, size: f32) -> TextMetrics {
        let font_key = core::ptr::from_ref(font).cast::<()>() as usize;
        let size_bits = size.to_bits();
        let hash = text_metric_hash(font_key, size_bits, text);
        self.ensure_text_metric_slots();
        let primary_slot = hash % Self::TEXT_MEASURE_CACHE_LIMIT;
        let mut insert_slot = primary_slot;
        for offset in 0..Self::TEXT_MEASURE_CACHE_PROBE_LIMIT {
            let slot = (primary_slot + offset) % Self::TEXT_MEASURE_CACHE_LIMIT;
            match &self.text_metrics[slot] {
                Some(entry)
                    if entry.hash == hash
                        && entry.font_key == font_key
                        && entry.size_bits == size_bits
                        && entry.text == text =>
                {
                    return entry.metrics;
                }
                None => {
                    insert_slot = slot;
                    break;
                }
                Some(_) => {}
            }
        }

        let (width, height, _) = gui_renderer::measure_text(font, text, size);
        let metrics = TextMetrics { width, height };
        if self.text_metrics[insert_slot].is_none() {
            self.text_metric_count += 1;
        }
        self.text_metrics[insert_slot] = Some(TextMeasureCacheEntry {
            font_key,
            text: text.to_string(),
            size_bits,
            hash,
            metrics,
        });
        metrics
    }

    fn rasterized_text<F: Font>(
        &mut self,
        font: &F,
        text: &str,
        size: f32,
        pos: (f32, f32),
    ) -> Option<&RasterizedText> {
        if text.is_empty() {
            return None;
        }

        let font_key = core::ptr::from_ref(font).cast::<()>() as usize;
        let size_bits = size.to_bits();
        let frac_x_key = subpixel_key(pos.0);
        let frac_y_key = subpixel_key(pos.1);
        let hash = text_bitmap_hash(font_key, size_bits, frac_x_key, frac_y_key, text);
        self.ensure_text_bitmap_slots();

        let primary_slot = hash % Self::TEXT_BITMAP_CACHE_LIMIT;
        let mut insert_slot = primary_slot;
        for offset in 0..Self::TEXT_BITMAP_CACHE_PROBE_LIMIT {
            let slot = (primary_slot + offset) % Self::TEXT_BITMAP_CACHE_LIMIT;
            match &self.text_bitmaps[slot] {
                Some(entry)
                    if entry.hash == hash
                        && entry.font_key == font_key
                        && entry.size_bits == size_bits
                        && entry.frac_x_key == frac_x_key
                        && entry.frac_y_key == frac_y_key
                        && entry.text == text =>
                {
                    return self.text_bitmaps[slot].as_ref().map(|entry| &entry.bitmap);
                }
                None => {
                    insert_slot = slot;
                    break;
                }
                Some(_) => {}
            }
        }

        let bitmap = rasterize_text_alpha(
            font,
            text,
            size,
            subpixel_value(frac_x_key),
            subpixel_value(frac_y_key),
        )?;
        let bitmap_bytes = bitmap.alpha.len();
        if bitmap_bytes > Self::TEXT_BITMAP_MAX_SINGLE_BYTES {
            return None;
        }
        if self.text_bitmap_bytes + bitmap_bytes > Self::TEXT_BITMAP_CACHE_MAX_BYTES {
            self.clear_text_bitmaps();
            self.ensure_text_bitmap_slots();
            insert_slot = primary_slot;
        }

        if let Some(existing) = &self.text_bitmaps[insert_slot] {
            self.text_bitmap_bytes = self
                .text_bitmap_bytes
                .saturating_sub(existing.bitmap.alpha.len());
        } else {
            self.text_bitmap_count += 1;
        }

        self.text_bitmap_bytes += bitmap_bytes;
        self.text_bitmaps[insert_slot] = Some(TextBitmapCacheEntry {
            font_key,
            text: text.to_string(),
            size_bits,
            frac_x_key,
            frac_y_key,
            hash,
            bitmap,
        });

        self.text_bitmaps[insert_slot]
            .as_ref()
            .map(|entry| &entry.bitmap)
    }

    fn gradient_pixels(
        &mut self,
        frame_width: usize,
        frame_height: usize,
        viewport: DisplayViewport,
        start_color: u32,
        end_color: u32,
    ) -> &[u32] {
        let needs_rebuild = self.gradient.as_ref().is_none_or(|cache| {
            cache.frame_width != frame_width
                || cache.frame_height != frame_height
                || cache.viewport != viewport
                || cache.start_color != start_color
                || cache.end_color != end_color
        });
        if needs_rebuild {
            let mut pixels = vec![BG_COLOR; frame_width.saturating_mul(frame_height)];
            draw_gradient_into_frame(
                &mut pixels,
                frame_width,
                frame_height,
                viewport,
                start_color,
                end_color,
            );
            self.gradient = Some(GradientCache {
                frame_width,
                frame_height,
                viewport,
                start_color,
                end_color,
                pixels,
            });
        }

        self.gradient
            .as_ref()
            .map(|cache| cache.pixels.as_slice())
            .unwrap_or(&[])
    }

    #[cfg(test)]
    fn text_measure_cache_len(&self) -> usize {
        self.text_metric_count
    }

    #[cfg(test)]
    fn text_bitmap_cache_len(&self) -> usize {
        self.text_bitmap_count
    }

    #[cfg(test)]
    fn frame_cache_hits(&self) -> usize {
        self.frame_cache_hits
    }
}

fn text_metric_hash(font_key: usize, size_bits: u32, text: &str) -> usize {
    let mut hash = 2_166_136_261usize;
    for byte in font_key.to_ne_bytes() {
        hash = hash.wrapping_mul(16_777_619) ^ usize::from(byte);
    }
    for byte in size_bits.to_ne_bytes() {
        hash = hash.wrapping_mul(16_777_619) ^ usize::from(byte);
    }
    for byte in text.as_bytes() {
        hash = hash.wrapping_mul(16_777_619) ^ usize::from(*byte);
    }
    hash
}

fn text_bitmap_hash(
    font_key: usize,
    size_bits: u32,
    frac_x_key: u8,
    frac_y_key: u8,
    text: &str,
) -> usize {
    let mut hash = text_metric_hash(font_key, size_bits, text);
    hash = hash.wrapping_mul(16_777_619) ^ usize::from(frac_x_key);
    hash = hash.wrapping_mul(16_777_619) ^ usize::from(frac_y_key);
    hash
}

fn subpixel_key(value: f32) -> u8 {
    let floor = value.floor();
    let fraction = (value - floor).clamp(0.0, 0.999_999);
    (fraction * RenderCache::SUBPIXEL_BUCKETS) as u8
}

fn subpixel_value(key: u8) -> f32 {
    f32::from(key) / RenderCache::SUBPIXEL_BUCKETS
}

fn rasterize_text_alpha<F: Font>(
    font: &F,
    text: &str,
    size: f32,
    frac_x: f32,
    frac_y: f32,
) -> Option<RasterizedText> {
    let scale = PxScale::from(size);
    let scaled_font = font.as_scaled(scale);
    let ascent = scaled_font.ascent();
    let pen_y = frac_y + ascent;
    let mut pen_x = frac_x;
    let mut last_glyph = None;
    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;

    for character in text.chars() {
        let glyph_id = font.glyph_id(character);
        if let Some(last) = last_glyph {
            pen_x += scaled_font.kern(last, glyph_id);
        }
        if let Some(outlined) =
            font.outline_glyph(glyph_id.with_scale_and_position(scale, point(pen_x, pen_y)))
        {
            let bounds = outlined.px_bounds();
            let left = bounds.min.x as i32;
            let top = bounds.min.y as i32;
            min_x = min_x.min(left);
            min_y = min_y.min(top);
            max_x = max_x.max(bounds.max.x.ceil() as i32);
            max_y = max_y.max(bounds.max.y.ceil() as i32);
        }
        pen_x += scaled_font.h_advance(glyph_id);
        last_glyph = Some(glyph_id);
    }

    if min_x >= max_x || min_y >= max_y {
        return None;
    }

    let width = usize::try_from(max_x - min_x).ok()?;
    let height = usize::try_from(max_y - min_y).ok()?;
    let len = width.checked_mul(height)?;
    if len == 0 {
        return None;
    }
    if len > RenderCache::TEXT_BITMAP_MAX_SINGLE_BYTES {
        return None;
    }
    let mut alpha = vec![0_u8; len];

    pen_x = frac_x;
    last_glyph = None;
    for character in text.chars() {
        let glyph_id = font.glyph_id(character);
        if let Some(last) = last_glyph {
            pen_x += scaled_font.kern(last, glyph_id);
        }
        let glyph = glyph_id.with_scale_and_position(scale, point(pen_x, pen_y));
        if let Some(outlined) = font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            let bounds_left = bounds.min.x as i32;
            let bounds_top = bounds.min.y as i32;
            outlined.draw(|x, y, coverage| {
                if coverage < 0.004 {
                    return;
                }
                let local_x = bounds_left + x as i32 - min_x;
                let local_y = bounds_top + y as i32 - min_y;
                if local_x < 0 || local_y < 0 {
                    return;
                }
                let local_x = local_x as usize;
                let local_y = local_y as usize;
                if local_x >= width || local_y >= height {
                    return;
                }
                let index = local_y * width + local_x;
                let value = if coverage > 0.996 {
                    u8::MAX
                } else {
                    (coverage * 255.0) as u8
                };
                alpha[index] = alpha[index].max(value);
            });
        }
        pen_x += scaled_font.h_advance(glyph_id);
        last_glyph = Some(glyph_id);
    }

    Some(RasterizedText {
        offset_x: min_x,
        offset_y: min_y,
        width,
        height,
        alpha,
    })
}

fn frame_cache_key(
    pixels: &[u32],
    width: usize,
    height: usize,
    viewport: DisplayViewport,
    font_generation: u64,
    render_list: &[Renderable],
) -> FrameCacheKey {
    FrameCacheKey {
        buffer_ptr: pixels.as_ptr() as usize,
        buffer_len: pixels.len(),
        width,
        height,
        viewport_x: viewport.x,
        viewport_y: viewport.y,
        viewport_width: viewport.width,
        viewport_height: viewport.height,
        viewport_scale_bits: viewport.scale.to_bits(),
        font_generation,
        render_hash: render_list_hash(render_list),
    }
}

fn render_list_hash(render_list: &[Renderable]) -> u64 {
    let mut hash = 14_695_981_039_346_656_037u64;
    hash_usize(&mut hash, render_list.len());
    for item in render_list {
        match item {
            Renderable::Background { gradient } => {
                hash_usize(&mut hash, 1);
                hash_u32(&mut hash, gradient.start_color);
                hash_u32(&mut hash, gradient.end_color);
            }
            Renderable::Text {
                text,
                anchor,
                shift,
                align,
                font_size,
                color,
            } => {
                hash_usize(&mut hash, 2);
                hash_text_renderable(&mut hash, text, *anchor, *shift, *align, *font_size, *color);
            }
            Renderable::BigText {
                text,
                anchor,
                shift,
                align,
                font_size,
                color,
            } => {
                hash_usize(&mut hash, 3);
                hash_text_renderable(&mut hash, text, *anchor, *shift, *align, *font_size, *color);
            }
            Renderable::TypingUpper {
                segments,
                anchor,
                shift,
                align,
                font_size,
                line_alignment,
            } => {
                hash_usize(&mut hash, 4);
                hash_anchor(&mut hash, *anchor);
                hash_shift(&mut hash, *shift);
                hash_align(&mut hash, *align);
                hash_font_size(&mut hash, *font_size);
                hash_typing_line_alignment(&mut hash, *line_alignment);
                hash_usize(&mut hash, segments.len());
                for segment in segments {
                    hash_str(&mut hash, &segment.base_text);
                    hash_option_str(&mut hash, segment.ruby_text.as_deref());
                    hash_option_str(&mut hash, segment.anno_text.as_deref());
                    hash_usize(&mut hash, segment.anno_group_run_count);
                    hash_option_font_script(&mut hash, segment.anno_script);
                    hash_font_script(&mut hash, segment.script);
                    hash_upper_segment_state(&mut hash, segment.state);
                }
            }
            Renderable::TypingLower {
                segments,
                anchor,
                shift,
                align,
                font_size,
                line_alignment,
            } => {
                hash_usize(&mut hash, 5);
                hash_anchor(&mut hash, *anchor);
                hash_shift(&mut hash, *shift);
                hash_align(&mut hash, *align);
                hash_font_size(&mut hash, *font_size);
                hash_typing_line_alignment(&mut hash, *line_alignment);
                hash_usize(&mut hash, segments.len());
                for segment in segments {
                    hash_lower_typing_segment(&mut hash, segment);
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
                hash_usize(&mut hash, 6);
                hash_anchor(&mut hash, *anchor);
                hash_shift(&mut hash, *shift);
                hash_u32(&mut hash, width_ratio.to_bits());
                hash_u32(&mut hash, height_ratio.to_bits());
                hash_u32(&mut hash, progress.to_bits());
                hash_u32(&mut hash, *bg_color);
                hash_u32(&mut hash, *fg_color);
            }
        }
    }
    hash
}

fn hash_text_renderable(
    hash: &mut u64,
    text: &str,
    anchor: ui::Anchor,
    shift: ui::Shift,
    align: ui::Align,
    font_size: FontSize,
    color: u32,
) {
    hash_str(hash, text);
    hash_anchor(hash, anchor);
    hash_shift(hash, shift);
    hash_align(hash, align);
    hash_font_size(hash, font_size);
    hash_u32(hash, color);
}

fn hash_lower_typing_segment(hash: &mut u64, segment: &LowerTypingSegment) {
    match segment {
        LowerTypingSegment::Completed {
            base_text,
            ruby_text,
            script,
            is_correct,
            width,
        } => {
            hash_usize(hash, 1);
            hash_str(hash, base_text);
            hash_option_str(hash, ruby_text.as_deref());
            hash_font_script(hash, *script);
            hash_bool(hash, *is_correct);
            hash_u32(hash, *width);
        }
        LowerTypingSegment::Active { elements, script } => {
            hash_usize(hash, 2);
            hash_font_script(hash, *script);
            hash_usize(hash, elements.len());
            for element in elements {
                hash_active_lower_element(hash, element);
            }
        }
    }
}

fn hash_active_lower_element(hash: &mut u64, element: &ActiveLowerElement) {
    match element {
        ActiveLowerElement::Typed {
            character,
            is_correct,
            script,
        } => {
            hash_usize(hash, 1);
            hash_char(hash, *character);
            hash_bool(hash, *is_correct);
            hash_font_script(hash, *script);
        }
        ActiveLowerElement::Cursor => hash_usize(hash, 2),
        ActiveLowerElement::UnconfirmedInput { text, script } => {
            hash_usize(hash, 3);
            hash_str(hash, text);
            hash_font_script(hash, *script);
        }
        ActiveLowerElement::LastIncorrectInput { character, script } => {
            hash_usize(hash, 4);
            hash_char(hash, *character);
            hash_font_script(hash, *script);
        }
    }
}

fn hash_anchor(hash: &mut u64, anchor: ui::Anchor) {
    let value = match anchor {
        ui::Anchor::TopLeft => 1,
        ui::Anchor::TopCenter => 2,
        ui::Anchor::TopRight => 3,
        ui::Anchor::CenterLeft => 4,
        ui::Anchor::Center => 5,
        ui::Anchor::CenterRight => 6,
        ui::Anchor::BottomLeft => 7,
        ui::Anchor::BottomCenter => 8,
        ui::Anchor::BottomRight => 9,
    };
    hash_usize(hash, value);
}

fn hash_shift(hash: &mut u64, shift: ui::Shift) {
    hash_u32(hash, shift.x.to_bits());
    hash_u32(hash, shift.y.to_bits());
}

fn hash_align(hash: &mut u64, align: ui::Align) {
    let horizontal = match align.horizontal {
        ui::HorizontalAlign::Left => 1,
        ui::HorizontalAlign::Center => 2,
        ui::HorizontalAlign::Right => 3,
    };
    let vertical = match align.vertical {
        ui::VerticalAlign::Top => 1,
        ui::VerticalAlign::Center => 2,
        ui::VerticalAlign::Bottom => 3,
    };
    hash_usize(hash, horizontal);
    hash_usize(hash, vertical);
}

fn hash_font_size(hash: &mut u64, font_size: FontSize) {
    match font_size {
        FontSize::WindowHeight(value) => {
            hash_usize(hash, 1);
            hash_u32(hash, value.to_bits());
        }
        FontSize::WindowAreaSqrt(value) => {
            hash_usize(hash, 2);
            hash_u32(hash, value.to_bits());
        }
    }
}

fn hash_typing_line_alignment(hash: &mut u64, alignment: ui::TypingLineAlignment) {
    hash_u32(hash, alignment.full_line_width);
    hash_u32(hash, alignment.visible_start_width);
}

fn hash_upper_segment_state(hash: &mut u64, state: ui::UpperSegmentState) {
    let value = match state {
        ui::UpperSegmentState::Correct => 1,
        ui::UpperSegmentState::Incorrect => 2,
        ui::UpperSegmentState::Pending => 3,
        ui::UpperSegmentState::Active => 4,
        ui::UpperSegmentState::Muted => 5,
    };
    hash_usize(hash, value);
}

fn hash_option_font_script(hash: &mut u64, script: Option<crate::font::FontScript>) {
    match script {
        Some(script) => {
            hash_bool(hash, true);
            hash_font_script(hash, script);
        }
        None => hash_bool(hash, false),
    }
}

fn hash_font_script(hash: &mut u64, script: crate::font::FontScript) {
    let value = match script {
        crate::font::FontScript::Japanese => 1,
        crate::font::FontScript::ChineseSimplified => 2,
        crate::font::FontScript::TraditionalChinese => 3,
        crate::font::FontScript::English => 4,
    };
    hash_usize(hash, value);
}

fn hash_option_str(hash: &mut u64, value: Option<&str>) {
    match value {
        Some(value) => {
            hash_bool(hash, true);
            hash_str(hash, value);
        }
        None => hash_bool(hash, false),
    }
}

fn hash_str(hash: &mut u64, value: &str) {
    hash_usize(hash, value.len());
    for byte in value.as_bytes() {
        hash_byte(hash, *byte);
    }
}

fn hash_char(hash: &mut u64, value: char) {
    hash_u32(hash, value as u32);
}

fn hash_bool(hash: &mut u64, value: bool) {
    hash_usize(hash, usize::from(value));
}

fn hash_u32(hash: &mut u64, value: u32) {
    for byte in value.to_ne_bytes() {
        hash_byte(hash, byte);
    }
}

fn hash_usize(hash: &mut u64, value: usize) {
    for byte in value.to_ne_bytes() {
        hash_byte(hash, byte);
    }
}

fn hash_byte(hash: &mut u64, byte: u8) {
    *hash ^= u64::from(byte);
    *hash = hash.wrapping_mul(1_099_511_628_211u64);
}

fn frame_clip_for_viewport(
    frame_width: usize,
    frame_height: usize,
    viewport: DisplayViewport,
) -> PixelClip {
    let left = (viewport.x as i64).clamp(0, frame_width as i64) as i32;
    let top = (viewport.y as i64).clamp(0, frame_height as i64) as i32;
    let right = (viewport.x as i64 + viewport.width as i64).clamp(0, frame_width as i64) as i32;
    let bottom = (viewport.y as i64 + viewport.height as i64).clamp(0, frame_height as i64) as i32;
    PixelClip::new(left, top, right, bottom)
}

fn draw_gradient_into_frame(
    pixels: &mut [u32],
    frame_width: usize,
    frame_height: usize,
    viewport: DisplayViewport,
    start_color: u32,
    end_color: u32,
) {
    if frame_width == 0 || frame_height == 0 {
        return;
    }

    let clip = frame_clip_for_viewport(frame_width, frame_height, viewport);
    if clip.is_empty() {
        return;
    }

    let dx = viewport.width as f32;
    let dy = viewport.height as f32;
    let len_sq = dx * dx + dy * dy;
    let inv_len_sq = if len_sq == 0.0 { 0.0 } else { 1.0 / len_sq };
    let colors = GradientColors::new(start_color, end_color);

    for y in clip.top..clip.bottom {
        let local_y = (y - viewport.y) as f32;
        let mut dot = (clip.left - viewport.x) as f32 * dx + local_y * dy;
        let row_start = y as usize * frame_width;
        for x in clip.left..clip.right {
            let ratio = if len_sq == 0.0 {
                0.0
            } else {
                (dot * inv_len_sq).clamp(0.0, 1.0)
            };
            pixels[row_start + x as usize] = colors.interpolate(ratio);
            dot += dx;
        }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderOutcome {
    changed: bool,
}

impl RenderOutcome {
    pub fn changed(self) -> bool {
        self.changed
    }
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
    ) -> RenderOutcome {
        cache.prepare_fonts(fonts);
        let viewport = display_settings.viewport(self.width, self.height);
        let changed = render_argb(
            self.pixels,
            self.width,
            self.height,
            viewport,
            fonts,
            render_list,
            cache,
        );
        RenderOutcome { changed }
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
) -> bool {
    let frame_key = frame_cache_key(
        pixels,
        width,
        height,
        viewport,
        fonts.generation(),
        render_list,
    );
    if cache.previous_frame == Some(frame_key) {
        #[cfg(test)]
        {
            cache.frame_cache_hits += 1;
        }
        return false;
    }

    let mut frame = PixelFrame {
        pixels,
        stride: width,
        frame_height: height,
        viewport,
    };
    let has_background = render_list
        .iter()
        .any(|item| matches!(item, Renderable::Background { .. }));
    if !has_background {
        frame.pixels.fill(BG_COLOR);
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
    cache.previous_frame = Some(frame_key);
    true
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
        frame_clip_for_viewport(self.stride, self.frame_height, self.viewport)
    }

    fn visible_local_rect(&self) -> (i32, i32, i32, i32) {
        let clip = self.frame_clip();
        (
            clip.left - self.viewport.x,
            clip.top - self.viewport.y,
            clip.right - self.viewport.x,
            clip.bottom - self.viewport.y,
        )
    }

    fn anchor_position(&self, anchor: ui::Anchor, shift: ui::Shift) -> (i32, i32) {
        let (visible_left, visible_top, visible_right, visible_bottom) = self.visible_local_rect();
        let visible_width = visible_right.saturating_sub(visible_left);
        let virtual_width = self.viewport.width as i32;
        let virtual_height = self.viewport.height as i32;
        let base_pos = match anchor {
            ui::Anchor::TopLeft => (visible_left, visible_top),
            ui::Anchor::TopCenter => (visible_left + visible_width / 2, visible_top),
            ui::Anchor::TopRight => (visible_right, visible_top),
            ui::Anchor::CenterLeft => (visible_left, virtual_height / 2),
            ui::Anchor::Center => (virtual_width / 2, virtual_height / 2),
            ui::Anchor::CenterRight => (visible_right, virtual_height / 2),
            ui::Anchor::BottomLeft => (visible_left, visible_bottom),
            ui::Anchor::BottomCenter => (visible_left + visible_width / 2, visible_bottom),
            ui::Anchor::BottomRight => (visible_right, visible_bottom),
        };
        (
            base_pos.0 + (shift.x * self.viewport.width as f32) as i32,
            base_pos.1 + (shift.y * self.viewport.height as f32) as i32,
        )
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

    fn draw_text_bitmap(&mut self, bitmap: &RasterizedText, pos: (f32, f32), color: u32) {
        if bitmap.width == 0 || bitmap.height == 0 || bitmap.alpha.is_empty() {
            return;
        }

        let base_x = pos.0.floor() as i32 + self.viewport.x + bitmap.offset_x;
        let base_y = pos.1.floor() as i32 + self.viewport.y + bitmap.offset_y;
        let clip = self.frame_clip();
        if clip.is_empty() {
            return;
        }

        let left = base_x.max(clip.left);
        let top = base_y.max(clip.top);
        let right = (base_x + bitmap.width as i32).min(clip.right);
        let bottom = (base_y + bitmap.height as i32).min(clip.bottom);
        if left >= right || top >= bottom {
            return;
        }

        let text_r = (color >> 16) & 0xFF;
        let text_g = (color >> 8) & 0xFF;
        let text_b = color & 0xFF;

        for y in top..bottom {
            let src_y = (y - base_y) as usize;
            let src_row = src_y * bitmap.width;
            let dst_row = y as usize * self.stride;
            for x in left..right {
                let alpha = u32::from(bitmap.alpha[src_row + (x - base_x) as usize]);
                if alpha == 0 {
                    continue;
                }
                let dst_index = dst_row + x as usize;
                if alpha == 255 {
                    self.pixels[dst_index] = 0xFF_000000 | (text_r << 16) | (text_g << 8) | text_b;
                    continue;
                }

                let inverse = 255 - alpha;
                let bg = self.pixels[dst_index];
                let bg_r = (bg >> 16) & 0xFF;
                let bg_g = (bg >> 8) & 0xFF;
                let bg_b = bg & 0xFF;
                let r = (text_r * alpha + bg_r * inverse) / 255;
                let g = (text_g * alpha + bg_g * inverse) / 255;
                let b = (text_b * alpha + bg_b * inverse) / 255;
                self.pixels[dst_index] = 0xFF_000000 | (r << 16) | (g << 8) | b;
            }
        }
    }

    fn fill_frame_rect(&mut self, rect: PixelClip, color: u32) {
        let frame_clip = self.frame_clip();
        let left = rect.left.max(frame_clip.left);
        let top = rect.top.max(frame_clip.top);
        let right = rect.right.min(frame_clip.right);
        let bottom = rect.bottom.min(frame_clip.bottom);
        if left >= right || top >= bottom {
            return;
        }

        gui_renderer::draw_rect(
            self.pixels,
            self.stride,
            left as usize,
            top as usize,
            (right - left) as usize,
            (bottom - top) as usize,
            color,
        );
    }

    fn fill_local_rect(&mut self, x: i32, y: i32, width: usize, height: usize, color: u32) {
        let left = self.viewport.x.saturating_add(x);
        let top = self.viewport.y.saturating_add(y);
        let right = (left as i64 + width as i64).clamp(i32::MIN as i64, i32::MAX as i64) as i32;
        let bottom = (top as i64 + height as i64).clamp(i32::MIN as i64, i32::MAX as i64) as i32;
        self.fill_frame_rect(PixelClip::new(left, top, right, bottom), color);
    }

    fn draw_gradient(&mut self, cache: &mut RenderCache, start_color: u32, end_color: u32) {
        if self.viewport.width == 0 || self.viewport.height == 0 {
            return;
        }

        let clip = self.frame_clip();
        if clip.is_empty() {
            return;
        }

        let gradient = cache.gradient_pixels(
            self.stride,
            self.frame_height,
            self.viewport,
            start_color,
            end_color,
        );
        if gradient.len() == self.pixels.len() {
            self.pixels.copy_from_slice(gradient);
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

#[derive(Clone, Copy)]
struct VisibleTextPlacement {
    pos: (f32, f32),
    size: f32,
    metrics: TextMetrics,
}

fn draw_text_if_visible<F: Font>(
    frame: &mut PixelFrame<'_>,
    font: &F,
    text: &str,
    placement: VisibleTextPlacement,
    color: u32,
    cache: &mut RenderCache,
) {
    let VisibleTextPlacement { pos, size, metrics } = placement;
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
        if should_cache_rasterized_text(metrics) {
            if let Some(bitmap) = cache.rasterized_text(font, text, size, pos) {
                frame.draw_text_bitmap(bitmap, pos, color);
                return;
            }
        }
        frame.draw_text_clipped(font, text, pos, size, color);
    }
}

fn should_cache_rasterized_text(metrics: TextMetrics) -> bool {
    (metrics.width as usize)
        .checked_mul(metrics.height as usize)
        .is_some_and(|bytes| bytes <= RenderCache::TEXT_BITMAP_MAX_SINGLE_BYTES)
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
    let anchor_pos = frame.anchor_position(placement.anchor, placement.shift);
    let (x, y) =
        ui::calculate_aligned_position(anchor_pos, metrics.width, metrics.height, placement.align);
    draw_text_if_visible(
        frame,
        font,
        text,
        VisibleTextPlacement {
            pos: (x as f32, y as f32),
            size: pixel_font_size,
            metrics,
        },
        color,
        cache,
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
    let segment_widths = adjusted_upper_segment_widths(fonts, segments, pixel_font_size, cache);
    let total_height = upper_typing_total_height(fonts, segments, pixel_font_size, cache);
    let anchor_pos = frame.anchor_position(placement.text.anchor, placement.text.shift);
    let (mut pen_x, y) = ui::calculate_aligned_position(
        anchor_pos,
        placement.line_alignment.full_line_width,
        total_height,
        placement.text.align,
    );
    pen_x += placement.line_alignment.visible_start_width as i32;
    let mut segment_x_positions = Vec::with_capacity(segment_widths.len());
    let mut segment_x = pen_x;
    for width in &segment_widths {
        segment_x_positions.push(segment_x);
        segment_x += *width as i32;
    }

    for (segment_index, (segment, segment_width)) in segments
        .iter()
        .zip(segment_widths.iter().copied())
        .enumerate()
    {
        let color = upper_segment_color(segment.state);
        let font = fonts.get_for_script(segment.script);
        let base_size = fonts.scaled_size_for_script(segment.script, pixel_font_size);
        let base_metrics = cache.measure_text(font, &segment.base_text, base_size);
        let base_x = pen_x as f32 + (segment_width as f32 - base_metrics.width as f32) / 2.0;
        draw_text_if_visible(
            frame,
            font,
            &segment.base_text,
            VisibleTextPlacement {
                pos: (base_x, y as f32),
                size: base_size,
                metrics: base_metrics,
            },
            color,
            cache,
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
                VisibleTextPlacement {
                    pos: (ruby_x, ruby_y),
                    size: ruby_pixel_font_size,
                    metrics: ruby_metrics,
                },
                color,
                cache,
            );
        }

        if let Some(anno) = &segment.anno_text {
            let anno_script = segment.anno_script.unwrap_or(segment.script);
            let anno_font = fonts.get_ruby_for_script(anno_script);
            let anno_pixel_font_size =
                fonts.scaled_size_for_ruby_script(anno_script, pixel_font_size * 0.3);
            let anno_metrics = cache.measure_text(anno_font, anno, anno_pixel_font_size);
            let (anno_group_x, anno_group_width) = upper_annotation_group_bounds(
                segment_index,
                segment,
                &segment_widths,
                &segment_x_positions,
            );
            let anno_x =
                anno_group_x as f32 + (anno_group_width as f32 - anno_metrics.width as f32) / 2.0;
            let anno_y = y as f32 + base_metrics.height as f32 + anno_pixel_font_size * 0.15;
            draw_text_if_visible(
                frame,
                anno_font,
                anno,
                VisibleTextPlacement {
                    pos: (anno_x, anno_y),
                    size: anno_pixel_font_size,
                    metrics: anno_metrics,
                },
                color,
                cache,
            );
        }

        pen_x += segment_width as i32;
    }
}

fn adjusted_upper_segment_widths(
    fonts: &Fonts,
    segments: &[ui::UpperTypingSegment],
    pixel_font_size: f32,
    cache: &mut RenderCache,
) -> Vec<u32> {
    let mut widths = segments
        .iter()
        .map(|segment| {
            upper_typing_segment_width_without_annotation(fonts, segment, pixel_font_size, cache)
        })
        .collect::<Vec<_>>();

    for (index, segment) in segments.iter().enumerate() {
        let Some(anno_text) = segment.anno_text.as_deref() else {
            continue;
        };
        let anno_script = segment.anno_script.unwrap_or(segment.script);
        let group_start = annotation_group_start(index, segment.anno_group_run_count);
        let group_width = widths[group_start..=index].iter().copied().sum::<u32>();
        let anno_pixel_font_size =
            fonts.scaled_size_for_ruby_script(anno_script, pixel_font_size * 0.3);
        let anno_width = cache
            .measure_text(
                fonts.get_ruby_for_script(anno_script),
                anno_text,
                anno_pixel_font_size,
            )
            .width;
        if anno_width > group_width {
            widths[index] += anno_width - group_width;
        }
    }

    widths
}

fn annotation_group_start(segment_index: usize, group_run_count: usize) -> usize {
    segment_index + 1 - group_run_count.max(1).min(segment_index + 1)
}

fn upper_annotation_group_bounds(
    segment_index: usize,
    segment: &ui::UpperTypingSegment,
    segment_widths: &[u32],
    segment_x_positions: &[i32],
) -> (i32, u32) {
    let group_start = annotation_group_start(segment_index, segment.anno_group_run_count);
    let group_x = segment_x_positions
        .get(group_start)
        .copied()
        .unwrap_or_else(|| segment_x_positions.get(segment_index).copied().unwrap_or(0));
    let group_width = segment_widths[group_start..=segment_index]
        .iter()
        .copied()
        .sum::<u32>();

    (group_x, group_width)
}

fn upper_typing_segment_width_without_annotation(
    fonts: &Fonts,
    segment: &ui::UpperTypingSegment,
    pixel_font_size: f32,
    cache: &mut RenderCache,
) -> u32 {
    let base_size = fonts.scaled_size_for_script(segment.script, pixel_font_size);
    let base_width = cache
        .measure_text(
            fonts.get_for_script(segment.script),
            &segment.base_text,
            base_size,
        )
        .width;
    let ruby_width = segment.ruby_text.as_deref().map_or(0, |ruby| {
        let ruby_pixel_font_size =
            fonts.scaled_size_for_ruby_script(segment.script, pixel_font_size * 0.4);
        cache
            .measure_text(
                fonts.get_ruby_for_script(segment.script),
                ruby,
                ruby_pixel_font_size,
            )
            .width
    });

    base_width.max(ruby_width)
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
            let anno_extra = if let Some(anno_text) = segment.anno_text.as_deref() {
                let anno_script = segment.anno_script.unwrap_or(segment.script);
                let anno_size =
                    fonts.scaled_size_for_ruby_script(anno_script, pixel_font_size * 0.3);
                let anno_height = cache
                    .measure_text(fonts.get_ruby_for_script(anno_script), anno_text, anno_size)
                    .height as f32;
                anno_size * 0.15 + anno_height
            } else {
                0.0
            };
            (-ruby_y + base_height.max(ruby_y + ruby_height) + anno_extra).ceil() as u32
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
    let anchor_pos = frame.anchor_position(placement.text.anchor, placement.text.shift);
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
                    let base_metrics = cache.measure_text(font, base_text, base_size);
                    let base_x =
                        pen_x as f32 + (*segment_width as f32 - base_metrics.width as f32) / 2.0;
                    draw_text_if_visible(
                        frame,
                        font,
                        base_text,
                        VisibleTextPlacement {
                            pos: (base_x, y as f32),
                            size: base_size,
                            metrics: base_metrics,
                        },
                        color,
                        cache,
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
                                VisibleTextPlacement {
                                    pos: (ruby_x, ruby_y),
                                    size: ruby_pixel_font_size,
                                    metrics: ruby_metrics,
                                },
                                color,
                                cache,
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
                            VisibleTextPlacement {
                                pos: (pen_x as f32, y as f32),
                                size,
                                metrics: text_metrics,
                            },
                            color,
                            cache,
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
    let anchor_pos = frame.anchor_position(placement.anchor, placement.shift);
    let start_x = anchor_pos.0.max(0);
    let start_y = (anchor_pos.1 - bar_height as i32).max(0);
    frame.fill_local_rect(
        start_x,
        start_y,
        bar_width as usize,
        bar_height as usize,
        colors.background,
    );

    let fg_width = (bar_width as f32 * placement.progress.clamp(0.0, 1.0)) as usize;
    if fg_width > 0 {
        frame.fill_local_rect(
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
            let advance = scaled_font.h_advance(glyph_id);
            let horizontal_guard = options.font_size;
            if pen_x + advance + horizontal_guard < options.clip.left as f32 {
                pen_x += advance;
                last_glyph = Some(glyph_id);
                continue;
            }
            if pen_x - horizontal_guard > options.clip.right as f32 {
                break;
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
            pen_x += advance;
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
        let mut render_list = vec![
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
            assert!(surface
                .render(&fonts, DisplaySettings::default(), &render_list, &mut cache)
                .changed());
        }
        let cache_len_after_first_render = cache.text_measure_cache_len();
        let bitmap_cache_len_after_first_render = cache.text_bitmap_cache_len();
        let first_pixels = pixels.clone();
        assert!(cache_len_after_first_render > 0);
        assert!(bitmap_cache_len_after_first_render > 0);
        assert!(pixels.iter().any(|pixel| *pixel != 0));

        {
            let mut surface =
                ArgbSurface::new(160, 90, &mut pixels).expect("surface dimensions should be valid");
            assert!(!surface
                .render(&fonts, DisplaySettings::default(), &render_list, &mut cache)
                .changed());
        }
        assert_eq!(cache.text_measure_cache_len(), cache_len_after_first_render);
        assert_eq!(
            cache.text_bitmap_cache_len(),
            bitmap_cache_len_after_first_render
        );
        assert_eq!(cache.frame_cache_hits(), 1);
        assert_eq!(pixels, first_pixels);

        if let Renderable::Text { text, .. } = &mut render_list[1] {
            *text = "Changed".to_string();
        }
        {
            let mut surface =
                ArgbSurface::new(160, 90, &mut pixels).expect("surface dimensions should be valid");
            assert!(surface
                .render(&fonts, DisplaySettings::default(), &render_list, &mut cache)
                .changed());
        }
        assert_eq!(cache.frame_cache_hits(), 1);
        assert_ne!(pixels, first_pixels);
    }

    #[test]
    fn argb_surface_uses_full_frame_width_when_aspect_extends_vertically() {
        let fonts = test_fonts();
        let render_list = vec![Renderable::Background {
            gradient: crate::ui::Gradient {
                start_color: 0xFF_222222,
                end_color: 0xFF_222222,
            },
        }];
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
            for x in 0..200 {
                assert_eq!(
                    pixels[y * 200 + x],
                    0xFF_222222,
                    "aspect-expanded background should cover frame pixel ({x},{y})"
                );
            }
        }
    }

    #[test]
    fn oversized_text_uses_clipped_rendering_without_bitmap_cache() {
        let fonts = test_fonts();
        let long_text = "W".repeat(4096);
        let render_list = vec![
            Renderable::Background {
                gradient: crate::ui::Gradient {
                    start_color: 0xFF_000000,
                    end_color: 0xFF_000000,
                },
            },
            Renderable::Text {
                text: long_text,
                anchor: crate::ui::Anchor::TopLeft,
                shift: crate::ui::Shift { x: 0.0, y: 0.1 },
                align: crate::ui::Align {
                    horizontal: crate::ui::HorizontalAlign::Left,
                    vertical: crate::ui::VerticalAlign::Top,
                },
                font_size: FontSize::WindowHeight(0.2),
                color: 0xFF_FFFFFF,
            },
        ];
        let mut cache = RenderCache::new();
        let mut pixels = vec![0u32; 240 * 120];
        let mut surface = ArgbSurface::new(240, 120, &mut pixels).expect("surface should be valid");

        assert!(surface
            .render(&fonts, DisplaySettings::default(), &render_list, &mut cache)
            .changed());
        assert_eq!(cache.text_bitmap_cache_len(), 0);
        assert!(pixels.iter().any(|pixel| *pixel != 0xFF_000000));
    }
}
