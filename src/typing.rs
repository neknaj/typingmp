// src/typing.rs

use crate::model::{
    Content, Model, ResultModel, Scroll, Segment, TypingCorrectnessChar, TypingCorrectnessContent,
    TypingCorrectnessLine, TypingCorrectnessSegment, TypingInput, TypingMetrics, TypingModel,
    TypingSession,
};
use crate::timestamp::now;

pub fn key_input(mut model_: TypingModel, input: char) -> Model {
    let current_time = now();
    let current_line_num = model_.status.line;

    if model_.content.lines.len() <= current_line_num as usize {
        return Model::Typing(model_); // Already finished
    }

    // Start a new session if it's the very first input or if there's a pause
    let should_start_new_session = if model_.user_input.is_empty() {
        true
    } else {
        match model_.user_input.last() {
            Some(last_session) if last_session.line != current_line_num => true,
            Some(last_session) => {
                if let Some(last_input) = last_session.inputs.last() {
                    (current_time - last_input.timestamp) > 1000.0 // 1-second pause
                } else {
                    true
                }
            }
            None => true,
        }
    };

    if should_start_new_session {
        model_.user_input.push(TypingSession {
            line: current_line_num,
            inputs: Vec::new(),
        });
    }

    let current_session = model_.user_input.last_mut().unwrap();

    let remaining_s = match &model_.content.lines[model_.status.line as usize].segments
        [model_.status.segment as usize]
    {
        Segment::Plain { text } => text.clone(),
        Segment::Annotated { reading, .. } => reading.clone(),
    };
    let remaining: Vec<char> = remaining_s.chars().collect();

    let mut expect = Vec::new();
    for (key, values) in model_.layout.mapping.iter() {
        for v in values {
            let mut flag = true;
            let start_index = model_.status.char_ as usize;
            if start_index + key.len() > remaining.len() {
                flag = false;
            } else {
                for (i, c) in key.chars().enumerate() {
                    if c != remaining[start_index + i] {
                        flag = false;
                        break;
                    }
                }
            }
            if !flag {
                continue;
            }

            for (i, c) in v.chars().enumerate() {
                if i < model_.status.unconfirmed.len() {
                    if model_.status.unconfirmed[i] != c {
                        flag = false;
                        break;
                    }
                }
            }
            if flag {
                expect.push((key.clone(), v.chars().collect::<Vec<char>>()));
            }
        }
    }

    let mut is_correct = false;
    let mut is_finished = false;
    for (key, e) in expect {
        if e.get(model_.status.unconfirmed.len()) == Some(&input) {
            is_correct = true;
            model_.status.last_wrong_keydown = None;

            if e.len() == model_.status.unconfirmed.len() + 1 {
                let char_pos = model_.status.char_ as usize;
                let segment = &mut model_.typing_correctness.lines[model_.status.line as usize]
                    .segments[model_.status.segment as usize];

                let mut has_error = false;
                for i in 0..key.chars().count() {
                    if segment.chars.get(char_pos + i) == Some(&TypingCorrectnessChar::Incorrect) {
                        has_error = true;
                        break;
                    }
                }

                for i in 0..key.chars().count() {
                    let correctness = if !has_error {
                        TypingCorrectnessChar::Correct
                    } else {
                        TypingCorrectnessChar::Incorrect
                    };
                    if let Some(c) = segment.chars.get_mut(char_pos + i) {
                        *c = correctness;
                    }
                }

                if remaining.len() == char_pos + key.chars().count() {
                    if model_.content.lines[model_.status.line as usize]
                        .segments
                        .len()
                        == model_.status.segment as usize + 1
                    {
                        if model_.content.lines.len() == model_.status.line as usize + 1 {
                            model_.status.line += 1;
                            is_finished = true;
                        } else {
                            model_.status.char_ = 0;
                            model_.status.segment = 0;
                            model_.status.line += 1;
                            model_.status.unconfirmed.clear();
                            model_.scroll.scroll = model_.scroll.max;
                        }
                    } else {
                        model_.status.char_ = 0;
                        model_.status.segment += 1;
                        model_.status.unconfirmed.clear();
                    }
                } else {
                    model_.status.char_ += key.chars().count() as i32;
                    model_.status.unconfirmed.clear();
                }
            } else {
                model_.status.unconfirmed.push(input);
            }
            break;
        }
    }

    current_session.inputs.push(TypingInput {
        key: input,
        timestamp: current_time,
        is_correct,
    });

    if !is_correct {
        model_.status.last_wrong_keydown = Some(input);
        let char_pos = model_.status.char_ as usize;
        let segment = &mut model_.typing_correctness.lines[model_.status.line as usize].segments
            [model_.status.segment as usize];
        if let Some(c) = segment.chars.get_mut(char_pos) {
            *c = TypingCorrectnessChar::Incorrect;
        }
    }

    if is_finished {
        Model::Result(ResultModel {
            typing_model: model_,
        })
    } else {
        Model::Typing(model_)
    }
}

pub fn create_typing_correctness_model(content: &Content) -> TypingCorrectnessContent {
    let lines = content
        .lines
        .iter()
        .map(|line| {
            let segments = line
                .segments
                .iter()
                .map(|segment| {
                    let target_text = match segment {
                        Segment::Plain { text } => text,
                        Segment::Annotated { reading, .. } => reading,
                    };
                    let chars = target_text
                        .chars()
                        .map(|_| TypingCorrectnessChar::Pending)
                        .collect();
                    TypingCorrectnessSegment { chars }
                })
                .collect();
            TypingCorrectnessLine { segments }
        })
        .collect();
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
        if session.inputs.is_empty() {
            continue;
        }

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
                total_miss_count += 1;
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
