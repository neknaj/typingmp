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

fn segment_target_text(seg: &Segment) -> String {
    match seg {
        Segment::Plain { text } => text.clone(),
        Segment::Annotated { reading, .. } => reading.clone(),
        Segment::Anno { inner, .. } => inner.iter().map(|s| segment_target_text(s)).collect(),
    }
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

    let mut result = String::new();
    let mut skipped = start_char;
    let mut remaining = max_chars;

    for seg in segments.iter().skip(start_segment) {
        if remaining == 0 {
            break;
        }

        let seg_text = segment_target_text(seg);
        if seg_text.is_empty() {
            continue;
        }

        let seg_len = seg_text.chars().count();
        if skipped >= seg_len {
            skipped -= seg_len;
            continue;
        }

        let mut chars = seg_text.chars().skip(skipped);
        skipped = 0;

        for ch in chars.by_ref().take(remaining) {
            result.push(ch);
            remaining -= 1;
            if remaining == 0 {
                break;
            }
        }
    }

    result
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
        let input_lower = input.to_lowercase().next().unwrap_or(input);
        let is_n_commit_trigger = model.status.unconfirmed.as_slice() == ['n']
            && !matches!(input_lower, 'a' | 'i' | 'u' | 'e' | 'o' | 'n' | 'y' | '\'')
            && target_slice.chars().next() == Some('ん');
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

    let mut is_correct = false;
    let mut advance_chars = 0;
    let mut is_romaji_in_progress = false;

    // 1. フリック入力などによる直接の文字一致を優先
    let input_normalized = normalize_typing_char(input);
    if let Some(target_char) = target_slice.chars().next() {
        if normalize_typing_char(target_char) == input_normalized {
            is_correct = true;
            advance_chars = 1;
            model.status.unconfirmed.clear();
        }
    }

    // 2. 直接一致しない場合、ローマ字入力として処理を試みる
    if !is_correct {
        let input_lower = input.to_ascii_lowercase();
        let mut current_input_str = String::with_capacity(model.status.unconfirmed.len() + 1);
        for unconfirmed_char in model.status.unconfirmed.iter() {
            current_input_str.push(*unconfirmed_char);
        }
        current_input_str.push(input_lower);

        let mut candidate_indexes: &[usize] = &[];
        if let Some(first_byte) = target_slice.as_bytes().first().copied() {
            let bucket = first_byte.to_ascii_lowercase() as usize;
            candidate_indexes = &model.layout.normalized_mapping_by_first_char[bucket];
        }

        for mapping_index in candidate_indexes {
            let (key, values) = &model.layout.normalized_mapping[*mapping_index];
            if !target_slice.starts_with(key) {
                continue;
            }
            let key_chars_count = key.len();

            for value in values {
                if value == &current_input_str {
                    is_correct = true;
                    model.status.unconfirmed.clear();
                    advance_chars = key_chars_count;
                    break;
                } else if value.starts_with(&current_input_str) {
                    is_correct = true;
                    is_romaji_in_progress = true;
                    model.status.unconfirmed.push(input_lower);
                    break;
                }
            }

            if is_correct {
                break;
            }
        }
    }

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
    let mut is_finished = false;
    if model.status.segment as usize >= word_content.segments.len() {
        model.status.segment = 0;
        model.status.char_ = 0;
        model.status.word += 1;
        if model.status.word as usize >= line_content.words.len() {
            model.status.word = 0;
            model.status.line += 1;
            if model.status.line as usize >= model.content.lines.len() {
                is_finished = true;
            }
        }
    }

    log(&format!(
        "  [Result] is_correct: {}, is_finished: {}",
        is_correct, is_finished
    ));
    log(&format!(
        "  [State After] line: {}, word: {}, seg: {}, char: {}, unconfirmed: {:?}",
        model.status.line, model.status.word, model.status.segment, model.status.char_, model.status.unconfirmed
    ));

    if is_finished {
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
