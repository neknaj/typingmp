// ./src/typing.rs

#[cfg(feature = "uefi")]
extern crate alloc;

#[cfg(feature = "uefi")]
use alloc::{
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

/// Backspaceキーが押されたときの処理 (全プラットフォーム共通)
pub fn process_backspace(mut model: TypingModel, timestamp: f64) -> TypingModel {
    if !model.status.current_word_input.is_empty() {
        model.status.current_word_input.pop();
        model.status.backspace_count += 1;
        log_physical_input(&mut model, '\u{08}', timestamp); // '\u{08}' は Backspace
    }
    update_progress(&mut model);
    model
}

/// 1文字入力の処理 (GUI/TUI/UEFI用)
pub fn process_char_input(mut model: TypingModel, c: char, timestamp: f64) -> Model {
    model.status.current_word_input.push(c);
    log_physical_input(&mut model, c, timestamp);
    
    // ライブ判定と進捗更新
    let word_completed = update_progress(&mut model);
    
    if word_completed {
        finalize_word(&mut model);
        if is_typing_finished(&model) {
            let backspace_count = model.status.backspace_count;
            return Model::Result(ResultModel { 
                typing_model: model, 
                total_backspaces: backspace_count
            });
        }
    }
    Model::Typing(model)
}

/// 文字列入力の処理 (WASM用)
pub fn process_string_input(mut model: TypingModel, new_input: String, timestamp: f64) -> Model {
    // 差分を計算して物理入力をログに記録
    let old_input = &model.status.current_word_input;
    if new_input.len() > old_input.len() && new_input.starts_with(old_input) {
        let added = &new_input[old_input.len()..];
        for c in added.chars() {
            log_physical_input(&mut model, c, timestamp);
        }
    } else if new_input.len() < old_input.len() && old_input.starts_with(&new_input) {
         let removed_count = old_input.len() - new_input.len();
         for _ in 0..removed_count {
             log_physical_input(&mut model, '\u{08}', timestamp);
         }
         model.status.backspace_count += removed_count as u32;
    } else {
        // 複雑なIME操作（文節の削除など）の場合、差分追跡を諦めてリセットに近い形でログを取る
        for _ in 0..old_input.len() {
            log_physical_input(&mut model, '\u{08}', timestamp);
        }
        for c in new_input.chars() {
            log_physical_input(&mut model, c, timestamp);
        }
    }

    model.status.current_word_input = new_input;
    
    // ライブ判定と進捗更新
    let word_completed = update_progress(&mut model);
    
    if word_completed {
        finalize_word(&mut model);
        if is_typing_finished(&model) {
            let backspace_count = model.status.backspace_count;
            return Model::Result(ResultModel { 
                typing_model: model, 
                total_backspaces: backspace_count 
            });
        }
    }
    
    Model::Typing(model)
}

/// 物理的なキー入力をセッションログに記録する
fn log_physical_input(model: &mut TypingModel, key: char, timestamp: f64) {
    if model.user_input_sessions.is_empty() || model.user_input_sessions.last().unwrap().line != model.status.line {
        model.user_input_sessions.push(TypingSession {
            line: model.status.line,
            inputs: Vec::new(),
        });
    }
    model.user_input_sessions.last_mut().unwrap().inputs.push(TypingInput {
        key,
        timestamp,
    });
}

/// 現在の単語の目標となる「読み」文字列を取得する
fn get_current_word_reading(model: &TypingModel) -> String {
    if let Some(line) = model.content.lines.get(model.status.line as usize) {
        if let Some(word) = line.words.get(model.status.word as usize) {
            return word.segments.iter().map(|seg| match seg {
                Segment::Plain { text } => text.clone(),
                Segment::Annotated { reading, .. } => reading.clone(),
            }).collect();
        }
    }
    String::new()
}

/// 入力文字列から、ターゲットの読みに最も長く一致する部分文字列を見つける
/// 戻り値: (マッチした読み, マッチしなかった入力の余り, マッチした入力部分)
fn find_best_romaji_match<'a>(user_input: &'a str, target_reading: &str, layout: &crate::model::Layout) -> (String, &'a str, &'a str) {
    if user_input.is_empty() {
        return (String::new(), "", "");
    }

    let mut matched_reading = String::new();
    let mut last_successful_input_pos = 0;
    let mut current_input_pos = 0;
    
    while current_input_pos < user_input.len() {
        let remaining_input = &user_input[current_input_pos..];
        let mut found_romaji_match = false;

        // 最長のローマ字表記を探す (例: "kya" を "k" "y" "a" より優先する)
        for len in (1..=remaining_input.len()).rev() {
            let romaji_candidate = &remaining_input[..len];
            if let Some(kana) = layout.mapping.iter().find_map(|(k, vs)| {
                if vs.iter().any(|v| v == romaji_candidate) { Some(k.clone()) } else { None }
            }) {
                let next_reading = matched_reading.clone() + &kana;
                if target_reading.starts_with(&next_reading) {
                    matched_reading = next_reading;
                    current_input_pos += len;
                    last_successful_input_pos = current_input_pos;
                    found_romaji_match = true;
                    break;
                }
            }
        }
        
        if !found_romaji_match {
            // これ以上マッチするローマ字がない
            break;
        }
    }
    
    let matched_input = &user_input[..last_successful_input_pos];
    let remaining_input = &user_input[last_successful_input_pos..];
    
    (matched_reading, remaining_input, matched_input)
}

/// 現在の入力状況に応じて、モデルの進捗（セグメント、文字）と正誤を更新する
/// 戻り値: 単語が完了したか否か
fn update_progress(model: &mut TypingModel) -> bool {
    let target_reading = get_current_word_reading(model);
    if target_reading.is_empty() {
        model.status.current_word_correctness.clear();
        return true; // 空の単語は即完了
    }
    
    let (matched_reading, remaining_input, matched_input) = find_best_romaji_match(&model.status.current_word_input, &target_reading, &model.layout);
    
    // 正誤配列を更新
    model.status.current_word_correctness.clear();
    for _ in 0..matched_input.len() {
        model.status.current_word_correctness.push(TypingCorrectnessChar::Correct);
    }
    for _ in 0..remaining_input.len() {
        model.status.current_word_correctness.push(TypingCorrectnessChar::Incorrect);
    }

    // UI上のカーソル位置を進める
    let mut chars_to_advance = matched_reading.chars().count();
    let mut seg_idx = 0;
    let mut char_idx = 0;
    if let Some(line_content) = model.content.lines.get(model.status.line as usize) {
        if let Some(word_content) = line_content.words.get(model.status.word as usize) {
            'outer: for (i, seg) in word_content.segments.iter().enumerate() {
                let seg_reading = match seg {
                    Segment::Plain { text } => text,
                    Segment::Annotated { reading, .. } => reading,
                };
                for (j, _) in seg_reading.chars().enumerate() {
                    if chars_to_advance == 0 {
                        seg_idx = i;
                        char_idx = j;
                        break 'outer;
                    }
                    chars_to_advance -= 1;
                }
                if chars_to_advance == 0 {
                    seg_idx = i + 1;
                    char_idx = 0;
                    break;
                }
            }
            if chars_to_advance > 0 { // 単語の最後まで到達
                 seg_idx = word_content.segments.len();
                 char_idx = 0;
            }
        }
    }
    model.status.segment = seg_idx as i32;
    model.status.char_ = char_idx as i32;
    
    // 単語完了チェック
    matched_reading == target_reading && remaining_input.is_empty()
}

/// 単語の入力を完了させ、状態を更新する
fn finalize_word(model: &mut TypingModel) {
    let is_correct = model.status.current_word_correctness.iter().all(|c| *c == TypingCorrectnessChar::Correct);

    // 1. 正誤記録を確定
    if let Some(line) = model.typing_correctness.lines.get_mut(model.status.line as usize) {
        if let Some(word) = line.words.get_mut(model.status.word as usize) {
            for seg in word.segments.iter_mut() {
                for c in seg.chars.iter_mut() {
                    *c = if is_correct { TypingCorrectnessChar::Correct } else { TypingCorrectnessChar::Incorrect };
                }
            }
        }
    }
    
    // 2. 次の単語に進む
    model.status.word += 1;
    model.status.segment = 0;
    model.status.char_ = 0;
    model.status.current_word_input.clear();
    model.status.current_word_correctness.clear();
    
    // 3. 行完了判定
    if let Some(line_content) = model.content.lines.get(model.status.line as usize) {
        if model.status.word as usize >= line_content.words.len() {
            model.status.word = 0;
            model.status.line += 1;
        }
    }
}

/// タイピングが全て完了したかチェック
fn is_typing_finished(model: &TypingModel) -> bool {
    model.status.line as usize >= model.content.lines.len()
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
            backspace_count: 0,
        }
    }

    fn calculate(&mut self) {
        let correctly_typed = self.type_count - self.miss_count;
        if self.type_count > 0 {
            self.accuracy = correctly_typed as f64 / self.type_count as f64;
            self.accuracy = self.accuracy.max(0.0);
        }
        if self.total_time > 0.0 {
            self.speed = (correctly_typed as f64) / (self.total_time / 1000.0);
        }
    }
}

pub fn calculate_total_metrics(model: &TypingModel) -> TypingMetrics {
    let mut metrics = TypingMetrics::new();
    metrics.backspace_count = model.status.backspace_count;
    
    if model.user_input_sessions.is_empty() {
        return metrics;
    }

    let first_input_time = model.user_input_sessions.iter().flat_map(|s| s.inputs.first()).map(|i| i.timestamp).fold(f64::MAX, f64::min);
    let last_input_time = model.user_input_sessions.iter().flat_map(|s| s.inputs.last()).map(|i| i.timestamp).fold(f64::MIN, f64::max);

    if last_input_time > first_input_time {
        metrics.total_time = last_input_time - first_input_time;
    }
    
    // 総物理タイプ数 (Backspaceを除く)
    metrics.type_count = model.user_input_sessions.iter()
        .flat_map(|s| s.inputs.iter())
        .filter(|i| i.key != '\u{08}')
        .count() as i32;

    // 入力履歴をシミュレートして最終的な入力文字列を構築
    let mut effective_inputs: Vec<char> = Vec::new();
    for session in &model.user_input_sessions {
        for input in &session.inputs {
            if input.key == '\u{08}' {
                effective_inputs.pop();
            } else {
                effective_inputs.push(input.key);
            }
        }
    }
    let final_input_str: String = effective_inputs.iter().collect();

    // 全ての目標テキスト（読み）を連結
    let total_target_reading: String = model.content.lines.iter()
        .flat_map(|line| line.words.iter())
        .flat_map(|word| word.segments.iter())
        .map(|seg| match seg {
            Segment::Plain { text } => text.clone(),
            Segment::Annotated { reading, .. } => reading.clone(),
        }).collect();

    // 最終的な入力から、どれだけ正しく読みに変換できたかを計算
    let (correctly_converted_reading, _, _) = find_best_romaji_match(&final_input_str, &total_target_reading, &model.layout);
    let correctly_typed_chars_count = correctly_converted_reading.chars().count() as i32;
    
    metrics.miss_count = metrics.type_count - correctly_typed_chars_count;

    metrics.calculate();
    metrics
}