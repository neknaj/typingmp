// ./src/typing.rs

extern crate alloc;

use alloc::{string::String, vec::Vec};

use crate::model::{
    CharIndex, Content, Segment, SegmentIndex, TypingCorrectnessChar, TypingCorrectnessContent,
    TypingCorrectnessLine, TypingCorrectnessSegment, TypingCorrectnessWord, TypingInput,
    TypingMetrics, TypingModel, TypingSession,
};

#[derive(Debug, Clone, Copy)]
enum KeystrokeMatch {
    Direct { advance_chars: usize },
    RomajiExact { advance_chars: usize },
    RomajiPrefix,
    Miss,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypingTransition {
    Ignored,
    Continue,
    Finished,
}

fn normalize_typing_char(c: char) -> char {
    // 古典かなの代替入力: ゐ・ヰ は「い」、ゑ・ヱ は「え」として扱う。
    // フリックキーボードには ゐ/ゑ キーが存在しないため、通常の い/え フリックで
    // 代替入力できるようにする。ヰ・ヱ（カタカナ版）も同様に対応する。
    match c {
        'ゐ' | 'ヰ' => return 'い',
        'ゑ' | 'ヱ' => return 'え',
        _ => {}
    }
    let lower = c.to_lowercase().next().unwrap_or(c);
    if ('ァ'..='ヶ').contains(&lower) {
        core::char::from_u32(lower as u32 - 0x60).unwrap_or(lower)
    } else {
        lower
    }
}

fn is_n_auto_commit_trigger(unconfirmed: &[char], input_lower: char, target_slice: &str) -> bool {
    matches!(
        (unconfirmed, target_slice.chars().next()),
        (['n'], Some('ん'))
            if !matches!(input_lower, 'a' | 'i' | 'u' | 'e' | 'o' | 'n' | 'y' | '\'')
    )
}

fn build_current_input_lower(unconfirmed: &[char], input_lower: char) -> String {
    let mut current_input_str = String::with_capacity(unconfirmed.len() + 1);
    for &ch in unconfirmed {
        current_input_str.push(ch);
    }
    current_input_str.push(input_lower);
    current_input_str
}

fn match_romaji_mapping(
    target_slice: &str,
    current_input: &str,
    layout: &crate::model::Layout,
) -> KeystrokeMatch {
    let candidate_indexes: &[usize] = match target_slice.as_bytes().first() {
        Some(first_byte) => {
            layout.normalized_mapping_by_first_byte(first_byte.to_ascii_lowercase())
        }
        None => &[],
    };

    for mapping_index in candidate_indexes {
        let Some((key, values)) = layout.normalized_mapping_at(*mapping_index) else {
            continue;
        };
        if !target_slice.starts_with(key) {
            continue;
        }
        let key_chars_count = key.chars().count();

        for value in values {
            match value.as_str() {
                v if v == current_input => {
                    return KeystrokeMatch::RomajiExact {
                        advance_chars: key_chars_count,
                    };
                }
                v if v.starts_with(current_input) => {
                    return KeystrokeMatch::RomajiPrefix;
                }
                _ => {}
            }
        }
    }

    KeystrokeMatch::Miss
}

fn classify_keystroke(
    target_slice: &str,
    input: char,
    input_lower: char,
    unconfirmed: &[char],
    layout: &crate::model::Layout,
) -> KeystrokeMatch {
    let input_normalized = normalize_typing_char(input);
    match target_slice.chars().next() {
        Some(target_char) if normalize_typing_char(target_char) == input_normalized => {
            return KeystrokeMatch::Direct { advance_chars: 1 };
        }
        Some(_) | None => {}
    }

    let current_input = build_current_input_lower(unconfirmed, input_lower);
    match_romaji_mapping(target_slice, &current_input, layout)
}

fn segment_prefix_chars(
    segments: &[Segment],
    start_segment: usize,
    start_char: usize,
    max_chars: usize,
) -> String {
    if max_chars == 0 || start_segment >= segments.len() {
        return String::new();
    }

    let mut result = String::with_capacity(max_chars);
    let mut skipped = start_char;
    let mut remaining = max_chars;

    for seg in segments.iter().skip(start_segment) {
        if remaining == 0 {
            break;
        }
        skipped = segment_prefix_chars_inner(seg, skipped, &mut remaining, &mut result);
    }

    result
}

fn segment_prefix_chars_inner(
    segment: &Segment,
    mut skipped: usize,
    remaining: &mut usize,
    output: &mut String,
) -> usize {
    if *remaining == 0 {
        return skipped;
    }

    match segment {
        Segment::Plain { text } | Segment::Annotated { reading: text, .. } => {
            for c in text.chars() {
                if skipped > 0 {
                    skipped -= 1;
                    continue;
                }

                if *remaining == 0 {
                    break;
                }

                output.push(c);
                *remaining -= 1;
            }
        }
        Segment::Anno { inner, .. } => {
            for seg in inner {
                skipped = segment_prefix_chars_inner(seg, skipped, remaining, output);
                if *remaining == 0 {
                    break;
                }
            }
        }
    }

    skipped
}

fn apply_correct_advance(
    model: &mut TypingModel,
    current_line_idx: usize,
    current_word_idx: usize,
    advance_chars: usize,
) {
    let mut remaining_advance = advance_chars;
    let mut current_seg_idx = model.status.segment.get();
    let mut current_char_idx = model.status.char_.get();

    while remaining_advance > 0 {
        let Some(correctness_word) = model
            .typing_correctness
            .lines
            .get_mut(current_line_idx)
            .and_then(|line| line.words.get_mut(current_word_idx))
        else {
            break;
        };
        let Some(correctness_segment) = correctness_word.segments.get_mut(current_seg_idx) else {
            break;
        };
        let current_seg_len = correctness_segment.chars.len();

        let chars_to_advance_in_seg = (current_seg_len - current_char_idx).min(remaining_advance);

        // 正誤情報を更新
        for i in 0..chars_to_advance_in_seg {
            if correctness_segment.chars[current_char_idx + i] != TypingCorrectnessChar::Incorrect {
                correctness_segment.chars[current_char_idx + i] = TypingCorrectnessChar::Correct;
            }
        }

        remaining_advance -= chars_to_advance_in_seg;
        current_char_idx += chars_to_advance_in_seg;

        if current_char_idx >= current_seg_len {
            current_seg_idx += 1;
            current_char_idx = 0;
        }
    }

    model.status.segment = SegmentIndex::new(current_seg_idx);
    model.status.char_ = CharIndex::new(current_char_idx);
}

fn update_completion(model: &mut TypingModel) -> TypingTransition {
    let current_line_idx = model.status.line.get();
    let Some(line_content) = model.content.lines.get(current_line_idx) else {
        return TypingTransition::Ignored;
    };
    let current_word_idx = model.status.word.get();
    let Some(word_content) = line_content.words.get(current_word_idx) else {
        return TypingTransition::Ignored;
    };

    if model.status.segment.get() < word_content.segments.len() {
        return TypingTransition::Continue;
    }

    model.status.segment.reset();
    model.status.char_.reset();
    model.status.word.advance();
    if model.status.word.get() < line_content.words.len() {
        return TypingTransition::Continue;
    }

    model.status.word.reset();
    model.status.line.advance();
    if model.status.line.get() >= model.content.lines.len() {
        TypingTransition::Finished
    } else {
        TypingTransition::Continue
    }
}

pub fn key_input(model: &mut TypingModel, input: char, timestamp: f64) -> TypingTransition {
    let current_time = timestamp;
    let current_line_idx = model.status.line.get();

    if model.content.lines.len() <= current_line_idx {
        return TypingTransition::Ignored;
    }

    let line_content = &model.content.lines[current_line_idx];
    let current_word_idx = model.status.word.get();
    if line_content.words.len() <= current_word_idx {
        return TypingTransition::Ignored;
    }
    let word_content = &line_content.words[current_word_idx];

    let current_segment = model.status.segment.get();
    let current_char = model.status.char_.get();
    let max_key_len = model.layout.normalized_mapping_max_key_len().max(1);
    let target_slice = segment_prefix_chars(
        &word_content.segments,
        current_segment,
        current_char,
        max_key_len,
    );

    // MS IME 方式の「ん」自動確定:
    // unconfirmed が ['n'] の状態で、次の目標文字が「ん」であり、
    // 入力が あ/い/う/え/お/n/y/' でない子音の場合、
    // 「n」をもう一度入力して「nn」→「ん」を確定させてから
    // 元の子音入力を処理する。
    {
        let input_lower = input.to_ascii_lowercase();
        let is_n_commit_trigger = is_n_auto_commit_trigger(
            model.status.unconfirmed.as_slice(),
            input_lower,
            &target_slice,
        );
        if is_n_commit_trigger {
            model.status.unconfirmed.clear();
            model.status.last_wrong_keydown = None;
            apply_correct_advance(model, current_line_idx, current_word_idx, 1);
            if matches!(update_completion(model), TypingTransition::Finished) {
                return TypingTransition::Finished;
            }
            return key_input(model, input, timestamp);
        }
    }

    if model.user_input.is_empty()
        || model
            .user_input
            .last()
            .and_then(|s| s.inputs.last())
            .is_none_or(|i| (current_time - i.timestamp) > 1000.0)
    {
        model.user_input.push(TypingSession {
            line: model.status.line,
            inputs: Vec::new(),
        });
    }

    let input_lower = input.to_ascii_lowercase();
    let keystroke_match = classify_keystroke(
        &target_slice,
        input,
        input_lower,
        model.status.unconfirmed.as_slice(),
        &model.layout,
    );

    let (is_correct, is_romaji_in_progress, advance_chars) = match keystroke_match {
        KeystrokeMatch::Direct { advance_chars } => {
            model.status.unconfirmed.clear();
            (true, false, advance_chars)
        }
        KeystrokeMatch::RomajiExact { advance_chars } => {
            model.status.unconfirmed.clear();
            (true, false, advance_chars)
        }
        KeystrokeMatch::RomajiPrefix => {
            model.status.unconfirmed.push(input_lower);
            (true, true, 0)
        }
        KeystrokeMatch::Miss => (false, false, 0),
    };

    // 3. 結果に基づいてモデルの状態を更新
    if is_correct {
        model.status.last_wrong_keydown = None;
        if !is_romaji_in_progress {
            apply_correct_advance(model, current_line_idx, current_word_idx, advance_chars);
        }
    } else {
        model.status.last_wrong_keydown = Some(input);
        model.status.unconfirmed.clear();
        let Some(correctness_segment) = model
            .typing_correctness
            .lines
            .get_mut(current_line_idx)
            .and_then(|l| l.words.get_mut(current_word_idx))
            .and_then(|w| w.segments.get_mut(model.status.segment.get()))
        else {
            return TypingTransition::Continue;
        };
        if let Some(c) = correctness_segment.chars.get_mut(model.status.char_.get()) {
            *c = TypingCorrectnessChar::Incorrect;
        }
    }

    model
        .user_input
        .last_mut()
        .unwrap()
        .inputs
        .push(TypingInput {
            key: input,
            timestamp,
            is_correct,
        });
    if is_correct {
        model.total_type_count += 1;
    } else {
        model.total_miss_count += 1;
    }
    if model.first_input_time.is_none_or(|first| timestamp < first) {
        model.first_input_time = Some(timestamp);
    }
    model.last_input_time = Some(
        model
            .last_input_time
            .map_or(timestamp, |last| last.max(timestamp)),
    );

    // 4. セグメント、単語、行、全体の完了チェック
    update_completion(model)
}

// セグメントのタイプ対象文字列を返す（Anno は inner を再帰的に連結）
fn segment_target_reading_static(seg: &Segment) -> String {
    match seg {
        Segment::Plain { text } => text.clone(),
        Segment::Annotated { reading, .. } => reading.clone(),
        Segment::Anno { inner, .. } => inner.iter().map(segment_target_reading_static).collect(),
    }
}

pub fn create_typing_correctness_model(content: &Content) -> TypingCorrectnessContent {
    let mut lines = Vec::new();
    for line in &content.lines {
        let mut words = Vec::new();
        for word in &line.words {
            let mut segments = Vec::new();
            for segment in &word.segments {
                let target_text = segment_target_reading_static(segment);
                let chars = target_text
                    .chars()
                    .map(|_| TypingCorrectnessChar::Pending)
                    .collect();
                segments.push(TypingCorrectnessSegment { chars });
            }
            words.push(TypingCorrectnessWord { segments });
        }
        lines.push(TypingCorrectnessLine { words });
    }
    TypingCorrectnessContent { lines }
}

impl TypingMetrics {
    fn new() -> Self {
        TypingMetrics {
            miss_count: 0,
            type_count: 0,
            total_time: 0.0,
            accuracy: 0.0,
            speed: 0.0,
        }
    }

    fn calculate(&mut self) {
        if self.type_count + self.miss_count > 0 {
            self.accuracy = self.type_count as f64 / (self.type_count + self.miss_count) as f64;
        }
        if self.total_time > 0.0 {
            self.speed = (self.type_count as f64) / (self.total_time / 1000.0);
        }
    }
}

pub fn calculate_total_metrics(model: &TypingModel) -> TypingMetrics {
    let mut metrics = TypingMetrics::new();
    metrics.type_count = model.total_type_count;
    metrics.miss_count = model.total_miss_count;
    if let (Some(first_input_time), Some(last_input_time)) =
        (model.first_input_time, model.last_input_time)
    {
        if last_input_time > first_input_time {
            metrics.total_time = last_input_time - first_input_time;
        }
    }

    metrics.calculate();
    metrics
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        CharIndex, LineIndex, Scroll, SegmentIndex, TypingCorrectnessChar, TypingModel,
        TypingStatus, WordIndex,
    };

    fn typing_model_from_problem(source: &str) -> TypingModel {
        let content = crate::parser::parse_problem(source).expect("test problem should parse");
        let typing_correctness = create_typing_correctness_model(&content);

        TypingModel {
            content,
            status: TypingStatus {
                line: LineIndex::ZERO,
                word: WordIndex::ZERO,
                segment: SegmentIndex::ZERO,
                char_: CharIndex::ZERO,
                unconfirmed: Vec::new(),
                last_wrong_keydown: None,
            },
            user_input: Vec::new(),
            total_type_count: 0,
            total_miss_count: 0,
            first_input_time: None,
            last_input_time: None,
            typing_correctness,
            layout: Default::default(),
            scroll: Scroll {
                scroll: 0.0,
                max: 0.0,
            },
        }
    }

    #[test]
    fn direct_kana_input_finishes_single_character_problem() {
        let mut model = typing_model_from_problem("#title Test\nあ");

        let result = key_input(&mut model, 'あ', 10.0);

        assert_eq!(result, TypingTransition::Finished);
        assert_eq!(model.total_type_count, 1);
        assert_eq!(model.total_miss_count, 0);
        assert_eq!(
            model.typing_correctness.lines[0].words[0].segments[0].chars[0],
            TypingCorrectnessChar::Correct
        );
    }

    #[test]
    fn romaji_prefix_keeps_cursor_until_mapping_is_complete() {
        let mut model = typing_model_from_problem("#title Test\n[色/し]あ");

        let result = key_input(&mut model, 's', 10.0);

        assert_eq!(result, TypingTransition::Continue);
        assert_eq!(model.status.unconfirmed, ['s']);
        assert_eq!(model.status.segment, SegmentIndex::ZERO);
        assert_eq!(model.status.char_, CharIndex::ZERO);

        let result = key_input(&mut model, 'i', 20.0);

        assert_eq!(result, TypingTransition::Continue);
        assert!(model.status.unconfirmed.is_empty());
        assert_eq!(model.status.word, WordIndex::new(1));
        assert_eq!(model.status.segment, SegmentIndex::ZERO);
        assert_eq!(model.status.char_, CharIndex::ZERO);
        assert_eq!(
            model.typing_correctness.lines[0].words[0].segments[0].chars[0],
            TypingCorrectnessChar::Correct
        );
    }

    #[test]
    fn miss_marks_current_character_without_advancing_cursor() {
        let mut model = typing_model_from_problem("#title Test\nあ");

        let result = key_input(&mut model, 'x', 10.0);

        assert_eq!(result, TypingTransition::Continue);
        assert_eq!(model.total_type_count, 0);
        assert_eq!(model.total_miss_count, 1);
        assert_eq!(model.status.last_wrong_keydown, Some('x'));
        assert_eq!(model.status.segment, SegmentIndex::ZERO);
        assert_eq!(model.status.char_, CharIndex::ZERO);
        assert_eq!(
            model.typing_correctness.lines[0].words[0].segments[0].chars[0],
            TypingCorrectnessChar::Incorrect
        );
    }

    #[test]
    fn n_auto_commit_mutates_in_place_before_original_key() {
        let mut model = typing_model_from_problem("#title Test\nんか");

        assert_eq!(key_input(&mut model, 'n', 10.0), TypingTransition::Continue);
        assert_eq!(model.status.unconfirmed, ['n']);
        assert_eq!(model.status.char_, CharIndex::ZERO);

        assert_eq!(key_input(&mut model, 'k', 20.0), TypingTransition::Continue);
        assert_eq!(model.status.unconfirmed, ['k']);
        assert_eq!(model.status.char_, CharIndex::new(1));
        assert_eq!(model.total_type_count, 2);
    }
}
