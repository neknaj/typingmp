// ./src/typing.rs

#[cfg(feature = "uefi")]
extern crate alloc;

#[cfg(feature = "uefi")]
use alloc::{format, string::String, vec::Vec};
#[cfg(not(feature = "uefi"))]
use std::{string::String, vec::Vec};

use crate::model::{Content, Model, ResultModel, Segment, TypingCorrectnessChar, TypingCorrectnessContent, TypingCorrectnessLine, TypingCorrectnessSegment, TypingInput, TypingMetrics, TypingModel, TypingSession};

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
        web_sys::console::log_1(&_message.into());
    }
}


pub fn key_input(mut model: TypingModel, input: char, timestamp: f64) -> Model {
    log(&format!("\n--- key_input: '{}' ---", input));
    log(&format!("  [State Before] line: {}, seg: {}, char: {}, unconfirmed: {:?}",
        model.status.line, model.status.segment, model.status.char_, model.status.unconfirmed));

    let current_time = timestamp;
    let current_line_idx = model.status.line as usize;

    if model.content.lines.len() <= current_line_idx {
        log("  [Result] Typing already finished. No action.");
        return Model::Typing(model);
    }

    if model.user_input.is_empty() || model.user_input.last().and_then(|s| s.inputs.last()).map_or(true, |i| (current_time - i.timestamp) > 1000.0) {
        model.user_input.push(TypingSession { line: model.status.line, inputs: Vec::new() });
    }

    let mut is_correct = false;
    let mut advance_chars = 0; // How many characters to advance model.status.char_ by

    let line_content = &model.content.lines[current_line_idx];
    let segment_content = &line_content.segments[model.status.segment as usize];
    let target_reading = match segment_content {
        Segment::Plain { text } => text,
        Segment::Annotated { reading, .. } => reading,
    };

    // --- 1. Directly typed a correct character? ---
    if model.status.unconfirmed.is_empty() {
        if let Some(target_char) = target_reading.chars().nth(model.status.char_ as usize) {
            if input == target_char {
                is_correct = true;
                advance_chars = 1;
            }
        }
    }

    // --- 2. If not a direct hit, try romanji ---
    if !is_correct {
        let start_char_index = model.status.char_ as usize;
        if let Some((start_byte_index, _)) = target_reading.char_indices().nth(start_char_index) {
            let remaining_slice = &target_reading[start_byte_index..];
            let mut expect = Vec::new();

            for (key, values) in model.layout.mapping.iter() {
                if remaining_slice.starts_with(key) {
                    for v in values {
                        if v.starts_with(&model.status.unconfirmed.iter().collect::<String>()) {
                           expect.push((key.clone(), (*v).to_string()));
                        }
                    }
                }
            }
            
            log(&format!("  [Expect List] Found {} candidates:", expect.len()));
            for (key, val) in &expect { log(&format!("    - Key: '{}', Value: {}", key, val)); }

            for (key, val_str) in expect {
                let mut current_input_str = model.status.unconfirmed.iter().collect::<String>();
                current_input_str.push(input);

                if val_str == current_input_str { // Romanji completed
                    is_correct = true;
                    model.status.unconfirmed.clear();
                    advance_chars = key.chars().count();
                    break;
                } else if val_str.starts_with(&current_input_str) { // Romanji in progress
                    is_correct = true;
                    model.status.unconfirmed.push(input);
                    advance_chars = 0;
                    break;
                }
            }
        }
    }

    // --- 3. Update model state based on outcome ---
    if is_correct {
        model.status.last_wrong_keydown = None;
        let correctness_segment = &mut model.typing_correctness.lines[current_line_idx].segments[model.status.segment as usize];
        let start_char_pos = model.status.char_ as usize;

        if advance_chars > 0 { // A direct hit or completed romanji
            let mut has_error = false;
            for i in 0..advance_chars {
                if correctness_segment.chars.get(start_char_pos + i) == Some(&TypingCorrectnessChar::Incorrect) {
                    has_error = true;
                    break;
                }
            }
            let new_status = if has_error { TypingCorrectnessChar::Incorrect } else { TypingCorrectnessChar::Correct };
            
            for i in 0..advance_chars {
                 if let Some(c) = correctness_segment.chars.get_mut(start_char_pos + i) {
                    *c = new_status.clone();
                }
            }
            model.status.char_ += advance_chars as i32;
        }
    } else { // Incorrect keypress
        model.status.last_wrong_keydown = Some(input);
        model.status.unconfirmed.clear(); // Incorrect romanji input should clear the buffer.
        let char_pos = model.status.char_ as usize;
        let segment = &mut model.typing_correctness.lines[current_line_idx].segments[model.status.segment as usize];
        if let Some(c) = segment.chars.get_mut(char_pos) { *c = TypingCorrectnessChar::Incorrect; }
    }
    
    model.user_input.last_mut().unwrap().inputs.push(TypingInput { key: input, timestamp, is_correct });

    // --- 4. Check for segment/line/game completion ---
    let mut is_finished = false;
    if advance_chars > 0 { // Check only if the cursor has advanced
        if model.status.char_ as usize >= target_reading.chars().count() {
            model.status.char_ = 0;
            model.status.segment += 1;
            if model.status.segment as usize >= line_content.segments.len() {
                model.status.segment = 0;
                model.status.line += 1;
                if model.status.line as usize >= model.content.lines.len() {
                    is_finished = true;
                }
            }
        }
    }
    
    log(&format!("  [Result] is_correct: {}, is_finished: {}", is_correct, is_finished));
    log(&format!("  [State After] line: {}, seg: {}, char: {}, unconfirmed: {:?}",
        model.status.line, model.status.segment, model.status.char_, model.status.unconfirmed));

    if is_finished {
        Model::Result(ResultModel { typing_model: model })
    } else {
        Model::Typing(model)
    }
}


pub fn create_typing_correctness_model(content: &Content) -> TypingCorrectnessContent {
    let mut lines = Vec::new();
    for line in &content.lines {
        let mut segments = Vec::new();
        for segment in &line.segments {
            let target_text = match segment {
                Segment::Plain { text } => text,
                Segment::Annotated { base: _, reading } => reading,
            };
            let chars = target_text.chars()
                .map(|_| TypingCorrectnessChar::Pending)
                .collect();
            segments.push(TypingCorrectnessSegment { chars });
        }
        lines.push(TypingCorrectnessLine { segments });
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