// ./src/typing.rs

#[cfg(feature = "uefi")]
extern crate alloc;

#[cfg(feature = "uefi")]
use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};
#[cfg(not(feature = "uefi"))]
use std::{
    string::{String, ToString},
    vec::Vec,
};

use crate::model::{
    Content, Model, ResultModel, Segment, TypingCorrectnessChar, TypingCorrectnessContent,
    TypingCorrectnessLine, TypingCorrectnessSegment, TypingCorrectnessWord, TypingInput, TypingMetrics, TypingModel,
    TypingSession,
};

#[derive(Debug, Clone, Copy)]
enum KeystrokeMatch {
    Direct { advance_chars: usize },
    RomajiExact { advance_chars: usize },
    RomajiPrefix,
    Miss,
}

#[derive(Debug, Clone, Copy)]
enum TypingCompletion {
    Continue,
    Finished,
}

// Helper function for logging to handle both native and wasm targets.
fn log(_message: &str) {
    #[cfg(any(not(feature = "tui"), feature = "gui"))]
    {
        #[cfg(not(target_arch = "wasm32"))]
        {
            #[cfg(not(feature = "uefi"))]
            println!("{}", _message);
            #[cfg(feature = "uefi")]
            uefi::println!("{}", _message);
        }
        #[cfg(target_arch = "wasm32")]
        {
            #[cfg(debug_assertions)]
            crate::wasm_debug_logger::log(_message);
            #[cfg(not(debug_assertions))]
            web_sys::console::log_1(&_message.into());
        }
    }
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
    if lower >= 'ァ' && lower <= 'ヶ' {
        core::char::from_u32(lower as u32 - 0x60).unwrap_or(lower)
    } else {
        lower
    }
}

fn is_n_auto_commit_trigger(unconfirmed: &[char], input_lower: char, target_slice: &str) -> bool {
    match (unconfirmed, target_slice.chars().next()) {
        ([ 'n' ], Some('ん')) if !matches!(
            input_lower,
            'a' | 'i' | 'u' | 'e' | 'o' | 'n' | 'y' | '\''
        ) => true,
        _ => false,
    }
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
        Some(first_byte) => &layout.normalized_mapping_by_first_char[first_byte.to_ascii_lowercase() as usize],
        None => &[],
    };

    for mapping_index in candidate_indexes {
        let (key, values) = &layout.normalized_mapping[*mapping_index];
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

pub fn key_input(mut model: TypingModel, input: char, timestamp: f64) -> Model {
    log(&format!("\n--- key_input: '{}' --- typing.rs", input));
    log(&format!(
        "  [State Before] line: {}, word: {}, seg: {}, char: {}, unconfirmed: {:?}",
        model.status.line, model.status.word, model.status.segment, model.status.char_, model.status.unconfirmed
    ));

    let current_time = timestamp;
    let current_line_idx = model.status.line as usize;

    if model.content.lines.len() <= current_line_idx {
        log("  [Result] Typing already finished. No action.");
        return Model::Typing(model);
    }

    let line_content = &model.content.lines[current_line_idx];
    if line_content.words.len() <= model.status.word as usize {
        return Model::Typing(model);
    }
    let word_content = &line_content.words[model.status.word as usize];

    let current_segment = model.status.segment as usize;
    let current_char = model.status.char_ as usize;
    let max_key_len = model.layout.normalized_mapping_max_key_len.max(1);
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
        let is_n_commit_trigger =
            is_n_auto_commit_trigger(model.status.unconfirmed.as_slice(), input_lower, &target_slice);
        if is_n_commit_trigger {
            log("  [ん auto-commit] triggering 'n' then original input");
            return match key_input(model, 'n', timestamp) {
                Model::Typing(m) => key_input(m, input, timestamp),
                Model::Result(r) => Model::Result(r),
            };
        }
    }

    if model
        .user_input
        .is_empty()
        || model
            .user_input
            .last()
            .and_then(|s| s.inputs.last())
            .map_or(true, |i| (current_time - i.timestamp) > 1000.0)
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
            let mut remaining_advance = advance_chars;
            let mut current_seg_idx = model.status.segment as usize;
            let mut current_char_idx = model.status.char_ as usize;

            while remaining_advance > 0 && current_seg_idx < word_content.segments.len() {
                let Some(correctness_word) = model.typing_correctness.lines
                    .get_mut(current_line_idx)
                    .and_then(|l| l.words.get_mut(model.status.word as usize)) else { break; };
                let Some(correctness_segment) = correctness_word.segments.get_mut(current_seg_idx) else { break; };
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
            model.status.segment = current_seg_idx as i32;
            model.status.char_ = current_char_idx as i32;
        }
    } else {
        model.status.last_wrong_keydown = Some(input);
        model.status.unconfirmed.clear();
        let Some(correctness_segment) = model.typing_correctness.lines
            .get_mut(current_line_idx)
            .and_then(|l| l.words.get_mut(model.status.word as usize))
            .and_then(|w| w.segments.get_mut(model.status.segment as usize)) else {
            return Model::Typing(model);
        };
        if let Some(c) = correctness_segment.chars.get_mut(model.status.char_ as usize) {
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
    if model.first_input_time.map_or(true, |first| timestamp < first) {
        model.first_input_time = Some(timestamp);
    }
    model.last_input_time = Some(model.last_input_time.map_or(timestamp, |last| last.max(timestamp)));

    // 4. セグメント、単語、行、全体の完了チェック
    let completion = if model.status.segment as usize >= word_content.segments.len() {
        model.status.segment = 0;
        model.status.char_ = 0;
        model.status.word += 1;
        if model.status.word as usize >= line_content.words.len() {
            model.status.word = 0;
            model.status.line += 1;
            if model.status.line as usize >= model.content.lines.len() {
                TypingCompletion::Finished
            } else {
                TypingCompletion::Continue
            }
        } else {
            TypingCompletion::Continue
        }
    } else {
        TypingCompletion::Continue
    };

    log(&format!(
        "  [Result] is_correct: {}, is_finished: {}",
        is_correct,
        matches!(completion, TypingCompletion::Finished)
    ));
    log(&format!(
        "  [State After] line: {}, word: {}, seg: {}, char: {}, unconfirmed: {:?}",
        model.status.line, model.status.word, model.status.segment, model.status.char_, model.status.unconfirmed
    ));

    if matches!(completion, TypingCompletion::Finished) {
        Model::Result(ResultModel {
            typing_model: model,
        })
    } else {
        Model::Typing(model)
    }
}

// セグメントのタイプ対象文字列を返す（Anno は inner を再帰的に連結）
fn segment_target_reading_static(seg: &Segment) -> String {
    match seg {
        Segment::Plain { text } => text.clone(),
        Segment::Annotated { reading, .. } => reading.clone(),
        Segment::Anno { inner, .. } => inner.iter().map(|s| segment_target_reading_static(s)).collect(),
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
                let chars = target_text.chars()
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
    if let (Some(first_input_time), Some(last_input_time)) = (model.first_input_time, model.last_input_time) {
        if last_input_time > first_input_time {
            metrics.total_time = last_input_time - first_input_time;
        }
    }

    metrics.calculate();
    metrics
}
