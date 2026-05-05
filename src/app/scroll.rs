extern crate alloc;

use crate::model::{CharIndex, Line, LineIndex, Segment, SegmentIndex, WordIndex};
use crate::renderer::gui_renderer;
use ab_glyph::FontVec;
use alloc::{string::String, vec::Vec};

// セグメントの base テキストを返す（Anno は inner を連結）
fn seg_base_text_owned(seg: &Segment) -> String {
    match seg {
        Segment::Plain { text } => text.clone(),
        Segment::Annotated { base, .. } => base.clone(),
        Segment::Anno { inner, .. } => inner.iter().map(seg_base_text_owned).collect(),
    }
}

// セグメントの reading テキストを返す（Anno は inner を連結）
fn seg_reading_text_owned(seg: &Segment) -> String {
    match seg {
        Segment::Plain { text } => text.clone(),
        Segment::Annotated { reading, .. } => reading.clone(),
        Segment::Anno { inner, .. } => inner.iter().map(seg_reading_text_owned).collect(),
    }
}

fn seg_ruby_text_owned(seg: &Segment) -> Option<String> {
    match seg {
        Segment::Plain { .. } => None,
        Segment::Annotated { reading, .. } => {
            if reading.is_empty() {
                None
            } else {
                Some(reading.clone())
            }
        }
        Segment::Anno { inner, .. } => {
            let reading = inner.iter().map(seg_reading_text_owned).collect::<String>();
            if reading.is_empty() {
                None
            } else {
                Some(reading)
            }
        }
    }
}

#[derive(Clone)]
pub(crate) struct ScrollLineSegmentCache {
    pub base_text: String,
    pub ruby_text: Option<String>,
    pub base_width: f32,
    pub reading_width_prefix: Vec<f32>,
    pub word_index: usize,
    pub segment_index: usize,
}

#[derive(Clone)]
pub(crate) struct ScrollLineCache {
    pub line: LineIndex,
    pub total_width: f32,
    pub segment_prefix_width: Vec<f32>,
    pub word_segment_starts: Vec<usize>,
    pub segments: Vec<ScrollLineSegmentCache>,
}

#[derive(Clone)]
pub(crate) struct ScrollCacheState {
    pub width: usize,
    pub height: usize,
    pub font_pixel_size: f32,
    pub line_origin: f32,
    pub cursor_in_line: f32,
    pub cursor_world: f32,
    pub current: ScrollLineCache,
}

#[derive(Clone)]
pub(crate) enum ScrollCache {
    Ready(ScrollCacheState),
}

pub(crate) const TYPING_FOCUS_X_RATIO: f32 = 0.42;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TypingLineScrollPosition {
    pub target: f64,
    pub min: f64,
    pub max: f64,
}

pub(crate) fn typing_focus_x(viewport_width: usize) -> f32 {
    viewport_width as f32 * TYPING_FOCUS_X_RATIO
}

pub(crate) fn typing_line_scroll_offset(
    line_width: f32,
    cursor_in_line: f32,
    viewport_width: usize,
) -> f32 {
    let viewport_width_f32 = viewport_width as f32;
    if viewport_width_f32 <= 0.0 {
        return 0.0;
    }

    let line_width = line_width.max(0.0);
    let cursor_in_line = cursor_in_line.max(0.0).min(line_width);
    if line_width <= viewport_width_f32 {
        return -((viewport_width_f32 - line_width) * 0.5);
    }

    let focus_x = typing_focus_x(viewport_width);
    let min_offset = -focus_x;
    let max_offset = line_width - viewport_width_f32;
    (cursor_in_line - focus_x).max(min_offset).min(max_offset)
}

pub(crate) fn typing_line_scroll_position(
    line_origin: f32,
    line_width: f32,
    cursor_in_line: f32,
    viewport_width: usize,
) -> TypingLineScrollPosition {
    let line_width = line_width.max(0.0);
    let viewport_width_f32 = viewport_width as f32;
    if viewport_width_f32 <= 0.0 {
        let target = f64::from(line_origin);
        return TypingLineScrollPosition {
            target,
            min: target,
            max: target,
        };
    }

    if line_width <= viewport_width_f32 {
        let target = f64::from(line_origin - ((viewport_width_f32 - line_width) * 0.5));
        return TypingLineScrollPosition {
            target,
            min: target,
            max: target,
        };
    }

    let focus_x = typing_focus_x(viewport_width);
    let min = f64::from(line_origin - focus_x);
    let max = f64::from(line_origin + line_width - viewport_width_f32);
    let target = f64::from(
        line_origin + typing_line_scroll_offset(line_width, cursor_in_line, viewport_width),
    );

    TypingLineScrollPosition {
        target: target.max(min).min(max),
        min,
        max,
    }
}

fn build_reading_width_prefix(font: &FontVec, text: &str, font_pixel_size: f32) -> Vec<f32> {
    let mut prefix = Vec::with_capacity(text.chars().count() + 1);
    prefix.push(0.0);
    let mut total = 0.0f32;
    for character in text.chars() {
        let mut buf = [0u8; 4];
        let ch = character.encode_utf8(&mut buf);
        total += gui_renderer::measure_text(font, ch, font_pixel_size).0 as f32;
        prefix.push(total);
    }
    prefix
}

pub(crate) fn build_scroll_line_cache(
    line: &Line,
    font: &FontVec,
    font_pixel_size: f32,
    line_index: LineIndex,
) -> ScrollLineCache {
    let mut segments = Vec::new();
    let mut segment_prefix_width = Vec::new();
    segment_prefix_width.push(0.0);
    let mut word_segment_starts = Vec::with_capacity(line.words.len());
    let mut total_width = 0.0f32;

    for word in &line.words {
        word_segment_starts.push(segments.len());
        for segment in &word.segments {
            let base_text = seg_base_text_owned(segment);
            let reading_text = seg_reading_text_owned(segment);
            let ruby_text = seg_ruby_text_owned(segment);
            let base_width = gui_renderer::measure_text(font, &base_text, font_pixel_size).0 as f32;
            let reading_width_prefix =
                build_reading_width_prefix(font, &reading_text, font_pixel_size);
            total_width += base_width;
            segment_prefix_width.push(total_width);
            segments.push(ScrollLineSegmentCache {
                base_text,
                ruby_text,
                base_width,
                reading_width_prefix,
                word_index: 0,
                segment_index: 0,
            });
        }
    }

    for word_index in 0..line.words.len() {
        let start = word_segment_starts.get(word_index).copied().unwrap_or(0);
        let end = word_segment_starts
            .get(word_index + 1)
            .copied()
            .unwrap_or(segments.len());
        for segment_index in start..end {
            if let Some(item) = segments.get_mut(segment_index) {
                item.word_index = word_index;
                item.segment_index = segment_index - start;
            }
        }
    }

    ScrollLineCache {
        line: line_index,
        total_width,
        segment_prefix_width,
        word_segment_starts,
        segments,
    }
}

fn line_total_width(line: &Line, font: &FontVec, font_pixel_size: f32) -> f32 {
    let mut total = 0.0f32;
    for word in &line.words {
        for segment in &word.segments {
            total +=
                gui_renderer::measure_text(font, &seg_base_text_owned(segment), font_pixel_size).0
                    as f32;
        }
    }
    total
}

pub(crate) fn line_origin_from_start(
    target_line: usize,
    lines: &[Line],
    font: &FontVec,
    font_pixel_size: f32,
    gap_width: f32,
) -> f32 {
    let mut origin = 0.0f32;
    let max_line = target_line.min(lines.len());
    for line_idx in 0..max_line {
        let line = if let Some(line) = lines.get(line_idx) {
            line
        } else {
            return origin;
        };
        origin += line_total_width(line, font, font_pixel_size) + gap_width;
    }
    origin
}

pub(crate) fn line_origin_from_previous(
    previous: &ScrollCacheState,
    target_line: usize,
    lines: &[Line],
    font: &FontVec,
    font_pixel_size: f32,
    gap_width: f32,
) -> f32 {
    let previous_line = previous.current.line.get();

    if target_line == previous_line {
        return previous.line_origin;
    }

    let mut origin = previous.line_origin;
    if target_line > previous_line {
        for line_idx in previous_line..target_line {
            let line = if let Some(line) = lines.get(line_idx) {
                line
            } else {
                return line_origin_from_start(
                    target_line,
                    lines,
                    font,
                    font_pixel_size,
                    gap_width,
                );
            };
            let width = if line_idx == previous_line {
                previous.current.total_width
            } else {
                line_total_width(line, font, font_pixel_size)
            };
            origin += width + gap_width;
        }
    } else {
        for line_idx in target_line..previous_line {
            let line = if let Some(line) = lines.get(line_idx) {
                line
            } else {
                return line_origin_from_start(
                    target_line,
                    lines,
                    font,
                    font_pixel_size,
                    gap_width,
                );
            };
            let width = line_total_width(line, font, font_pixel_size);
            origin -= width + gap_width;
        }
    }

    origin
}

pub(crate) fn cursor_position_from_status(
    cache: &ScrollLineCache,
    status_word: WordIndex,
    status_segment: SegmentIndex,
    status_char: CharIndex,
) -> f32 {
    let status_word_usize = status_word.get();
    let status_segment_usize = status_segment.get();
    let status_char_usize = status_char.get();

    if status_word_usize < cache.word_segment_starts.len() {
        let segment_start = cache.word_segment_starts[status_word_usize];
        let segment_end = cache
            .word_segment_starts
            .get(status_word_usize + 1)
            .copied()
            .unwrap_or(cache.segments.len());
        let segment_count = segment_end.saturating_sub(segment_start);
        let segment_idx = if segment_count == 0 {
            segment_start
        } else {
            segment_start + status_segment_usize.min(segment_count - 1)
        };
        let base = cache
            .segment_prefix_width
            .get(segment_idx)
            .copied()
            .unwrap_or(0.0);
        let typed_width = if let Some(seg_cache) = cache.segments.get(segment_idx) {
            let typed_len =
                status_char_usize.min(seg_cache.reading_width_prefix.len().saturating_sub(1));
            seg_cache
                .reading_width_prefix
                .get(typed_len)
                .copied()
                .unwrap_or(0.0)
        } else {
            0.0
        };
        (base + typed_width).min(cache.total_width)
    } else {
        cache.total_width
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 0.001,
            "actual {actual} should be close to expected {expected}"
        );
    }

    #[test]
    fn scroll_position_centers_short_lines() {
        let position = typing_line_scroll_position(100.0, 200.0, 0.0, 800);

        assert_close(position.target, -200.0);
        assert_close(100.0 - position.target, 300.0);
        assert_eq!(position.min, position.target);
        assert_eq!(position.max, position.target);
    }

    #[test]
    fn scroll_position_places_long_line_start_at_focus() {
        let position = typing_line_scroll_position(0.0, 2_000.0, 0.0, 1_000);
        let focus_x = f64::from(typing_focus_x(1_000));

        assert_close(position.target, -focus_x);
        assert_close(0.0 - position.target, focus_x);
    }

    #[test]
    fn scroll_position_keeps_long_line_cursor_near_focus() {
        let position = typing_line_scroll_position(0.0, 2_000.0, 900.0, 1_000);
        let focus_x = f64::from(typing_focus_x(1_000));

        assert_close(900.0 - position.target, focus_x);
    }

    #[test]
    fn scroll_position_clamps_long_line_end_to_viewport_right() {
        let position = typing_line_scroll_position(0.0, 2_000.0, 2_000.0, 1_000);

        assert_close(position.target, 1_000.0);
        assert_close(2_000.0 - position.target, 1_000.0);
    }
}
