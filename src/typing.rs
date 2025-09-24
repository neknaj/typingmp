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

    let line_content = &model.content.lines[current_line_idx];
    let word_content = &line_content.words[model.status.word as usize];

    // 現在の単語内で、まだタイプされていない全てのセグメントの「読み」を連結してターゲット文字列を生成
    let target_reading: String = word_content.segments[model.status.segment as usize..]
        .iter()
        .map(|seg| match seg {
            Segment::Plain { text } => text.clone(),
            Segment::Annotated { reading, .. } => reading.clone(),
        })
        .collect();
    
    // 現在の文字位置から始まる部分文字列を取得
    let target_slice = target_reading.chars().skip(model.status.char_ as usize).collect::<String>();

    fn normalize_char(c: char) -> char {
        let lower = c.to_lowercase().next().unwrap_or(c);
        if lower >= 'ァ' && lower <= 'ヶ' {
            core::char::from_u32(lower as u32 - 0x60).unwrap_or(lower)
        } else {
            lower
        }
    }

    // 1. フリック入力などによる直接の文字一致を優先
    if let Some(target_char) = target_slice.chars().next() {
        if normalize_char(input) == normalize_char(target_char) {
            is_correct = true;
            advance_chars = 1;
            model.status.unconfirmed.clear();
        }
    }

    // 2. 直接一致しない場合、ローマ字入力として処理を試みる
    if !is_correct {
        let mut expect = Vec::new();
        for (key, values) in model.layout.mapping.iter() {
            if target_slice.starts_with(key) {
                for v in values {
                    if v.starts_with(&model.status.unconfirmed.iter().collect::<String>()) {
                        expect.push((key.clone(), (*v).to_string()));
                    }
                }
            }
        }

        if !expect.is_empty() {
            let mut current_input_str = model.status.unconfirmed.iter().collect::<String>();
            current_input_str.push(input);

            for (key, val_str) in expect {
                let lower_val_str = val_str.to_lowercase();
                let lower_current_input_str = current_input_str.to_lowercase();

                if lower_val_str == lower_current_input_str {
                    is_correct = true;
                    model.status.unconfirmed.clear();
                    advance_chars = key.chars().count();
                    break;
                } else if lower_val_str.starts_with(&lower_current_input_str) {
                    is_correct = true;
                    is_romaji_in_progress = true;
                    model.status.unconfirmed.push(input);
                    break;
                }
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
                let correctness_segment = &mut model.typing_correctness.lines[current_line_idx].words[model.status.word as usize].segments[current_seg_idx];
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
        let correctness_segment = &mut model.typing_correctness.lines[current_line_idx].words[model.status.word as usize].segments[model.status.segment as usize];
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

pub fn create_typing_correctness_model(content: &Content) -> TypingCorrectnessContent {
    let mut lines = Vec::new();
    for line in &content.lines {
        let mut words = Vec::new();
        for word in &line.words {
            let mut segments = Vec::new();
            for segment in &word.segments {
                let target_text = match segment {
                    Segment::Plain { text } => text,
                    Segment::Annotated { base: _, reading } => reading,
                };
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
    let mut total_type_count = 0;
    let mut total_miss_count = 0;
    
    let mut first_input_time = f64::MAX;
    let mut last_input_time = f64::MIN;

    for session in &model.user_input {
        if session.inputs.is_empty() { continue; }

        if let Some(first) = session.inputs.first() {
            if first.timestamp < first_input_time {
                first_input_time = first.timestamp;
            }
        }
        if let Some(last) = session.inputs.last() {
            if last.timestamp > last_input_time {
                last_input_time = last.timestamp;
            }
        }

        for input in &session.inputs {
            if input.is_correct {
                total_type_count += 1;
            } else {
                total_miss_count +=1;
            }
        }
    }
    
    metrics.type_count = total_type_count;
    metrics.miss_count = total_miss_count;
    if last_input_time > first_input_time {
        metrics.total_time = last_input_time - first_input_time;
    }

    metrics.calculate();
    metrics
}