// src/typing.rs

#[cfg(feature = "uefi")]
extern crate alloc;

#[cfg(feature = "uefi")]
use alloc::{format, vec::Vec};

#[cfg(not(feature = "uefi"))]
use std::vec::Vec;

use crate::model::{Model, TypingModel, ResultModel, TypingCorrectnessContent, TypingSession, TypingInput, TypingCorrectnessLine, TypingCorrectnessSegment, TypingCorrectnessChar, TypingMetrics};
use crate::model::{Content, Segment};
use crate::timestamp::now;

// Helper function for logging to handle both native and wasm targets.
fn log(message: &str) {
    // TUIモード (`tui` featureが有効で `gui` featureが無効) の場合はログを出力しない
    #[cfg(any(not(feature = "tui"), feature = "gui"))]
    {
        #[cfg(not(target_arch = "wasm32"))]
        {
            #[cfg(not(feature = "uefi"))]
            println!("{}", message);
            #[cfg(feature = "uefi")]
            uefi::println!("{}", message);
        }
        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&message.into());
    }
}


pub fn key_input(mut model_: TypingModel, input: char) -> Model {
    log(&format!("\n--- key_input: '{}' ---", input));
    log(&format!("  [State Before] line: {}, seg: {}, char: {}, unconfirmed: {:?}",
        model_.status.line, model_.status.segment, model_.status.char_, model_.status.unconfirmed));

    let current_time = now();
    let current_line = model_.status.line;
    
    if model_.content.lines.len() <= current_line as usize {
        log("  [Result] Typing already finished. No action.");
        return Model::Typing(model_);
    }

    // (セッション開始ロジックは変更なし)
    let should_start_new_session = if model_.user_input.is_empty() { true } else {
        model_.user_input.last().map_or(true, |s| s.inputs.last().map_or(true, |i| (current_time - i.timestamp) > 1000.0))
    };

    if should_start_new_session {
        model_.user_input.push(TypingSession { line: current_line, inputs: Vec::new() });
    }
    let current_session = model_.user_input.last_mut().unwrap();
    
    let remaining_s = match &model_.content.lines[model_.status.line as usize].segments[model_.status.segment as usize] {
        Segment::Plain { text } => text.clone(),
        Segment::Annotated { reading, .. } => reading.clone(),
    };
    log(&format!("  [Target Text] remaining: '{}'", remaining_s));
    let remaining = remaining_s.chars().collect::<Vec<char>>();

    // --- EXPECT LIST GENERATION: REVERTED TO OLD, ROBUST LOGIC ---
    let mut expect = Vec::new();
    for (key, values) in model_.layout.mapping.iter() {
        for v in values {
            let mut flag = true;
            let start_index = model_.status.char_ as usize;
            
            // Check 1: Does the Japanese key match the target text?
            for (i, c) in key.chars().enumerate() {
                if let Some(rs_char) = remaining_s.chars().nth(start_index + i) {
                    if c != rs_char {
                        flag = false;
                        break;
                    }
                } else {
                    flag = false;
                    break;
                }
            }
            if !flag {
                continue;
            }

            // Check 2: Does the romaji value match the unconfirmed input?
            for (i, unconfirmed_char) in model_.status.unconfirmed.iter().enumerate() {
                if let Some(v_char) = v.chars().nth(i) {
                    if *unconfirmed_char != v_char {
                        flag = false;
                        break;
                    }
                } else {
                    flag = false;
                    break;
                }
            }

            if flag {
                expect.push((key.clone(), v.chars().collect::<Vec<char>>()));
            }
        }
    }
    // --- END OF EXPECT LIST GENERATION ---

    log(&format!("  [Expect List] Found {} candidates:", expect.len()));
    for (key, val) in &expect {
        log(&format!("    - Key: '{}', Value: {:?}", key, val));
    }

    let mut is_correct = false;
    let mut is_finished = false;
    for (key,e) in expect {
        if e.get(model_.status.unconfirmed.len()) == Some(&input) {
            is_correct = true;
            model_.status.last_wrong_keydown = None;
            
            if e.len() == model_.status.unconfirmed.len() + 1 {
                let char_pos = model_.status.char_ as usize;
                let segment = &mut model_.typing_correctness.lines[model_.status.line as usize].segments[model_.status.segment as usize];
                let mut has_error = false;
                for i in 0..key.chars().count() {
                    if segment.chars.get(char_pos + i) == Some(&TypingCorrectnessChar::Incorrect) { has_error = true; break; }
                }
                for i in 0..key.chars().count() {
                    let correctness = if !has_error { TypingCorrectnessChar::Correct } else { TypingCorrectnessChar::Incorrect };
                    if let Some(c) = segment.chars.get_mut(char_pos + i) { *c = correctness; }
                }

                if remaining.len() == model_.status.char_ as usize + key.chars().count() {
                    if model_.content.lines[model_.status.line as usize].segments.len() == model_.status.segment as usize + 1 {
                        if model_.content.lines.len() == model_.status.line as usize + 1 {
                            model_.status.line += 1; is_finished = true;
                        } else {
                            model_.status.char_ = 0; model_.status.segment = 0; model_.status.line += 1;
                        }
                    } else {
                        model_.status.char_ = 0; model_.status.segment += 1;
                    }
                } else {
                    model_.status.char_ += key.chars().count() as i32;
                }
                model_.status.unconfirmed.clear();
            } else {
                model_.status.unconfirmed.push(input);
            }
            break;
        }
    }

    current_session.inputs.push(TypingInput { key: input, timestamp: current_time, is_correct });
    
    if !is_correct {
        model_.status.last_wrong_keydown = Some(input);
        let char_pos = model_.status.char_ as usize;
        let segment = &mut model_.typing_correctness.lines[model_.status.line as usize].segments[model_.status.segment as usize];
        if let Some(c) = segment.chars.get_mut(char_pos) { *c = TypingCorrectnessChar::Incorrect; }
    }

    log(&format!("  [Result] is_correct: {}, is_finished: {}", is_correct, is_finished));
    log(&format!("  [State After] line: {}, seg: {}, char: {}, unconfirmed: {:?}",
        model_.status.line, model_.status.segment, model_.status.char_, model_.status.unconfirmed));

    if is_finished {
        Model::Result(ResultModel { typing_model: model_ })
    } else {
        Model::Typing(model_)
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