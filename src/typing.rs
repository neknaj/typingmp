// ./src/typing.rs

#[cfg(feature = "uefi")]
extern crate alloc;

#[cfg(feature = "uefi")]
use alloc::{
    collections::BTreeMap, // Use BTreeMap for no_std compatibility
    format,
    string::String,
    vec::Vec,
};
#[cfg(not(feature = "uefi"))]
use std::{
    collections::HashMap, // Use HashMap for std environment
    string::String,
    vec::Vec,
};


use crate::model::{
    Content, Model, ResultModel, Segment, TypingCorrectnessChar, TypingCorrectnessContent,
    TypingCorrectnessLine, TypingCorrectnessSegment, TypingCorrectnessWord, TypingInput, TypingMetrics, TypingModel,
    TypingSession,
};

// Helper function for logging to handle both native and wasm targets.
fn log(message: &str) {
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
        {
            #[cfg(debug_assertions)]
            crate::wasm_debug_logger::log(message);
            #[cfg(not(debug_assertions))]
            web_sys::console::log_1(&message.into());
        }
    }
}

/// Backspaceキーが押されたときの処理 (全プラットフォーム共通)
pub fn process_backspace(mut model: TypingModel, timestamp: f64) -> TypingModel {
    log(&format!("[BACKSPACE] current_input: '{}'", model.status.current_word_input));
    if !model.status.current_word_input.is_empty() {
        model.status.current_word_input.pop();
        model.status.backspace_count += 1;
        log_physical_input(&mut model, '\u{08}', timestamp); // '\u{08}' は Backspace
    }
    update_progress(&mut model);
    log(&format!("[BACKSPACE] new_input: '{}'", model.status.current_word_input));
    model
}

/// 1文字入力の処理 (GUI/TUI/UEFI用)
pub fn process_char_input(mut model: TypingModel, c: char, timestamp: f64) -> Model {
    log(&format!("[CHAR_INPUT] char: '{}', current_input: '{}'", c, model.status.current_word_input));
    model.status.current_word_input.push(c);
    log_physical_input(&mut model, c, timestamp);
    
    // ライブ判定と進捗更新
    let word_completed = update_progress(&mut model);
    
    if word_completed {
        finalize_word(&mut model);
        if is_typing_finished(&model) {
            log("[FINISH] Typing complete. Transitioning to Result model.");
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
    log(&format!("[STRING_INPUT] string: '{}', current_input: '{}'", new_input, model.status.current_word_input));
    
    let old_input = &model.status.current_word_input;
    
    // 1. 最長共通接頭辞の長さを計算
    let common_prefix_len = old_input.chars()
        .zip(new_input.chars())
        .take_while(|(a, b)| a == b)
        .count();

    // 2. 削除された文字数を計算し、バックスペースとしてログに記録
    let removed_char_count = old_input.chars().count() - common_prefix_len;
    if removed_char_count > 0 {
        model.status.backspace_count += removed_char_count as u32;
        for _ in 0..removed_char_count {
            log_physical_input(&mut model, '\u{08}', timestamp);
        }
    }

    // 3. 追加された部分を新しい入力としてログに記録
    let added_part_start_index = new_input.char_indices().nth(common_prefix_len).map_or(new_input.len(), |(i, _)| i);
    let added_part = &new_input[added_part_start_index..];
    for c in added_part.chars() {
        log_physical_input(&mut model, c, timestamp);
    }

    model.status.current_word_input = new_input;
    
    // ライブ判定と進捗更新
    let word_completed = update_progress(&mut model);
    
    if word_completed {
        finalize_word(&mut model);
        if is_typing_finished(&model) {
            log("[FINISH] Typing complete. Transitioning to Result model.");
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

/// ローマ字から「かな」への逆引きマップを生成する
#[cfg(not(feature = "uefi"))]
fn get_romaji_to_kana_map(layout: &crate::model::Layout) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for (kana, romaji_vec) in &layout.mapping {
        for romaji in romaji_vec {
            if !romaji.is_empty() {
                map.insert(romaji.clone(), kana.clone());
            }
        }
    }
    map
}

#[cfg(feature = "uefi")]
fn get_romaji_to_kana_map(layout: &crate::model::Layout) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for (kana, romaji_vec) in &layout.mapping {
        for romaji in romaji_vec {
            if !romaji.is_empty() {
                map.insert(romaji.clone(), kana.clone());
            }
        }
    }
    map
}


/// ローマ字文字列を「かな」文字列に変換する
fn convert_romaji_to_kana(romaji_str: &str, layout: &crate::model::Layout) -> String {
    let romaji_map = get_romaji_to_kana_map(layout);
    let mut result = String::new();
    let mut current_pos = 0;
    while current_pos < romaji_str.len() {
        // 4文字（最大ローマ字長を想定）から1文字まで、最長一致を探す
        let remaining = &romaji_str[current_pos..];
        let mut found_match = false;
        for len in (1..=4).rev() {
            if remaining.len() >= len {
                let sub = &remaining[..len];
                if let Some(kana) = romaji_map.get(sub) {
                    result.push_str(kana);
                    current_pos += len;
                    found_match = true;
                    break;
                }
            }
        }
        // マッチしなかった場合は、1文字進めてスキップ（英字入力など）
        if !found_match {
            result.push(remaining.chars().next().unwrap());
            current_pos += 1;
        }
    }
    result
}

/// readingがローマ字か（ASCII文字のみか）を判定する
fn is_romaji(s: &str) -> bool {
    s.chars().all(|c| c.is_ascii_alphabetic() || c == '\'')
}

/// 現在の単語の目標となる正規化された「かな」文字列を取得する
fn get_current_word_target_kana(model: &TypingModel) -> String {
    if let Some(line) = model.content.lines.get(model.status.line as usize) {
        if let Some(word) = line.words.get(model.status.word as usize) {
            let reading_str: String = word.segments.iter().map(|seg| match seg {
                Segment::Plain { text } => text.clone(),
                Segment::Annotated { reading, .. } => reading.clone(),
            }).collect();
            
            // readingがローマ字表記なら「かな」に変換する
            if is_romaji(&reading_str) {
                return convert_romaji_to_kana(&reading_str, &model.layout);
            } else {
                return reading_str;
            }
        }
    }
    String::new()
}


/// 入力文字列から、ターゲットの読みに最も長く一致する部分文字列を見つける
fn find_best_romaji_match<'a>(user_input: &'a str, target_kana: &str, layout: &crate::model::Layout) -> (String, &'a str, &'a str) {
    if user_input.is_empty() || target_kana.is_empty() {
        return (String::new(), user_input, "");
    }

    let mut matched_reading = String::new();
    let mut last_successful_input_pos = 0;
    
    let target_chars: Vec<char> = target_kana.chars().collect();
    let mut target_idx = 0;
    
    let mut current_input_pos = 0;
    while current_input_pos < user_input.len() {
        if target_idx >= target_chars.len() {
            break; 
        }
        
        let current_target_char = target_chars[target_idx];
        let remaining_input = &user_input[current_input_pos..];

        let romaji_options = layout.mapping.iter()
            .find(|(key, _)| key.chars().next() == Some(current_target_char))
            .map(|(_, values)| values);
            
        if let Some(options) = romaji_options {
            let mut found_match_for_char = false;
            let mut sorted_options = options.clone();
            sorted_options.sort_by(|a, b| b.len().cmp(&a.len()));

            for option in sorted_options.iter() {
                if !option.is_empty() && remaining_input.starts_with(option.as_str()) {
                    matched_reading.push(current_target_char);
                    current_input_pos += option.len();
                    last_successful_input_pos = current_input_pos;
                    target_idx += 1;
                    found_match_for_char = true;
                    break;
                }
            }
            if !found_match_for_char {
                break;
            }
        } else {
            break;
        }
    }
    
    let matched_input = &user_input[..last_successful_input_pos];
    let remaining_input = &user_input[last_successful_input_pos..];

    log(&format!("[ROMAJI_MATCH] user_input: '{}', target: '{}' -> matched_reading: '{}', matched_input: '{}', remaining_input: '{}'", 
        user_input, target_kana, matched_reading, matched_input, remaining_input));
    
    (matched_reading, remaining_input, matched_input)
}


/// 現在の入力状況に応じて、モデルの進捗（セグメント、文字）と正誤を更新する
/// 戻り値: 単語が完了したか否か
fn update_progress(model: &mut TypingModel) -> bool {
    let target_kana = get_current_word_target_kana(model);
    if target_kana.is_empty() {
        model.status.current_word_correctness.clear();
        return model.status.current_word_input.is_empty();
    }
    
    let (matched_reading, remaining_input, matched_input) = find_best_romaji_match(&model.status.current_word_input, &target_kana, &model.layout);
    
    model.status.current_word_correctness.clear();
    for _ in 0..matched_input.len() {
        model.status.current_word_correctness.push(TypingCorrectnessChar::Correct);
    }
    for _ in 0..remaining_input.len() {
        model.status.current_word_correctness.push(TypingCorrectnessChar::Incorrect);
    }

    let mut chars_to_advance = matched_reading.chars().count();
    let mut seg_idx = 0;
    let mut char_idx = 0;
    if let Some(line_content) = model.content.lines.get(model.status.line as usize) {
        if let Some(word_content) = line_content.words.get(model.status.word as usize) {
            'outer: for (i, seg) in word_content.segments.iter().enumerate() {
                let seg_reading_raw = match seg {
                    Segment::Plain { text } => text,
                    Segment::Annotated { reading, .. } => reading,
                };
                let seg_reading_kana = if is_romaji(seg_reading_raw) {
                    convert_romaji_to_kana(seg_reading_raw, &model.layout)
                } else {
                    seg_reading_raw.clone()
                };

                for (j, _) in seg_reading_kana.chars().enumerate() {
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
            if chars_to_advance > 0 {
                 seg_idx = word_content.segments.len();
                 char_idx = 0;
            }
        }
    }
    model.status.segment = seg_idx as i32;
    model.status.char_ = char_idx as i32;
    log(&format!("[PROGRESS] advanced to seg: {}, char: {}. correctness: {:?}", seg_idx, char_idx, model.status.current_word_correctness));
    
    let is_complete = matched_reading == target_kana && remaining_input.is_empty();
    log(&format!("[PROGRESS] word_complete check: {}", is_complete));
    is_complete
}

/// 単語の入力を完了させ、状態を更新する
fn finalize_word(model: &mut TypingModel) {
    let is_correct = model.status.current_word_correctness.iter().all(|c| *c == TypingCorrectnessChar::Correct);
    log(&format!("[FINALIZE] Word finished. Correct: {}. Advancing to word {}", is_correct, model.status.word + 1));

    if let Some(line) = model.typing_correctness.lines.get_mut(model.status.line as usize) {
        if let Some(word) = line.words.get_mut(model.status.word as usize) {
            for seg in word.segments.iter_mut() {
                for c in seg.chars.iter_mut() {
                    *c = if is_correct { TypingCorrectnessChar::Correct } else { TypingCorrectnessChar::Incorrect };
                }
            }
        }
    }
    
    model.status.word += 1;
    model.status.segment = 0;
    model.status.char_ = 0;
    model.status.current_word_input.clear();
    model.status.current_word_correctness.clear();
    
    if let Some(line_content) = model.content.lines.get(model.status.line as usize) {
        if model.status.word as usize >= line_content.words.len() {
            model.status.word = 0;
            model.status.line += 1;
            log(&format!("[FINALIZE] Line finished. Advancing to line {}", model.status.line));
        }
    }
}

/// タイピングが全て完了したかチェック
fn is_typing_finished(model: &TypingModel) -> bool {
    model.status.line as usize >= model.content.lines.len()
}

pub fn create_typing_correctness_model(content: &Content) -> TypingCorrectnessContent {
    let mut lines = Vec::new();
    let layout = crate::model::Layout::default(); // レイアウトを一時的に作成
    for line in &content.lines {
        let mut words = Vec::new();
        for word in &line.words {
            let mut segments = Vec::new();
            for segment in &word.segments {
                let target_text_raw = match segment {
                    Segment::Plain { text } => text,
                    Segment::Annotated { base: _, reading } => reading,
                };
                let target_text_kana = if is_romaji(target_text_raw) {
                    convert_romaji_to_kana(target_text_raw, &layout)
                } else {
                    target_text_raw.clone()
                };
                let chars = target_text_kana.chars()
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
    log("[METRICS] Calculating total metrics...");
    let mut metrics = TypingMetrics::new();
    metrics.backspace_count = model.status.backspace_count;
    
    if model.user_input_sessions.is_empty() {
        log("[METRICS] No user input sessions found.");
        return metrics;
    }

    let first_input_time = model.user_input_sessions.iter().flat_map(|s| s.inputs.first()).map(|i| i.timestamp).fold(f64::MAX, f64::min);
    let last_input_time = model.user_input_sessions.iter().flat_map(|s| s.inputs.last()).map(|i| i.timestamp).fold(f64::MIN, f64::max);

    if last_input_time > first_input_time {
        metrics.total_time = last_input_time - first_input_time;
    }
    log(&format!("[METRICS] Total time: {} ms", metrics.total_time));
    
    metrics.type_count = model.user_input_sessions.iter()
        .flat_map(|s| s.inputs.iter())
        .filter(|i| i.key != '\u{08}')
        .count() as i32;
    log(&format!("[METRICS] Total physical types (type_count): {}", metrics.type_count));

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
    log(&format!("[METRICS] Final effective input string: '{}'", final_input_str));

    let total_target_kana: String = model.content.lines.iter()
        .flat_map(|line| line.words.iter())
        .flat_map(|word| word.segments.iter())
        .map(|seg| {
            let raw = match seg {
                Segment::Plain { text } => text.clone(),
                Segment::Annotated { reading, .. } => reading.clone(),
            };
            if is_romaji(&raw) {
                convert_romaji_to_kana(&raw, &model.layout)
            } else {
                raw
            }
        }).collect();
    log(&format!("[METRICS] Total target kana: '{}'", total_target_kana));

    let (_, _, correctly_typed_input) = find_best_romaji_match(&final_input_str, &total_target_kana, &model.layout);
    
    let correct_type_count = correctly_typed_input.chars().count() as i32;
    
    metrics.miss_count = metrics.type_count - correct_type_count;
    log(&format!("[METRICS] Correct types: {}, Miss count: {}", correct_type_count, metrics.miss_count));

    metrics.calculate();
    log(&format!("[METRICS] Final metrics: Accuracy = {:.2}%, Speed = {:.2} kps", metrics.accuracy * 100.0, metrics.speed));
    metrics
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;
    use crate::model::{Scroll, TypingStatus};

    /// テスト用のTypingModelをセットアップするヘルパー関数
    fn setup_model(problem_text: &str) -> TypingModel {
        let content = parser::parse_problem(problem_text);
        let typing_correctness = create_typing_correctness_model(&content);
        TypingModel {
            content,
            status: TypingStatus {
                line: 0,
                word: 0,
                segment: 0,
                char_: 0,
                current_word_input: String::new(),
                current_word_correctness: Vec::new(),
                backspace_count: 0,
            },
            user_input_sessions: Vec::new(),
            typing_correctness,
            layout: Default::default(),
            scroll: Scroll { scroll: 0.0, max: 0.0 },
        }
    }

    // テスト用の正誤判定ヘルパー
    fn is_word_correct(word: &TypingCorrectnessWord) -> bool {
        word.segments.iter().all(|seg| {
            !seg.chars.iter().any(|c| *c == TypingCorrectnessChar::Incorrect)
        })
    }

    #[test]
    fn test_single_char_input_correct() {
        let model = setup_model("#title test\n(か/ka)");
        let model = match process_char_input(model, 'k', 0.0) {
            Model::Typing(m) => m,
            _ => panic!("Should be Typing model"),
        };
        assert_eq!(model.status.current_word_input, "k");
        assert_eq!(model.status.current_word_correctness, vec![TypingCorrectnessChar::Correct]);
        assert_eq!(model.status.word, 0);
    }

    #[test]
    fn test_single_char_input_incorrect() {
        let model = setup_model("#title test\n(か/ka)");
        let model = match process_char_input(model, 't', 0.0) {
            Model::Typing(m) => m,
            _ => panic!("Should be Typing model"),
        };
        assert_eq!(model.status.current_word_input, "t");
        assert_eq!(model.status.current_word_correctness, vec![TypingCorrectnessChar::Incorrect]);
    }
    
    #[test]
    fn test_backspace_correction() {
        let mut model = setup_model("#title test\n(か/ka)");
        model = match process_char_input(model, 't', 0.0) { Model::Typing(m) => m, _ => panic!() };
        model = process_backspace(model, 1.0);
        model = match process_char_input(model, 'k', 2.0) { Model::Typing(m) => m, _ => panic!() };
        let final_model = process_char_input(model, 'a', 3.0);

        if let Model::Result(result_model) = final_model {
            assert_eq!(result_model.typing_model.status.backspace_count, 1);
            assert!(is_typing_finished(&result_model.typing_model));
        } else {
            panic!("Model should be Result after finishing the problem");
        }
    }

    #[test]
    fn test_word_finalization() {
        let mut model = setup_model("#title test\n(か/ka) (き/ki)");
        model = match process_char_input(model, 'k', 0.0) { Model::Typing(m) => m, _ => panic!() };
        model = match process_char_input(model, 'a', 1.0) { Model::Typing(m) => m, _ => panic!() };
        assert_eq!(model.status.word, 1);
        assert!(is_word_correct(&model.typing_correctness.lines[0].words[0]));

        model = match process_char_input(model, ' ', 2.0) { Model::Typing(m) => m, _ => panic!() };
        assert_eq!(model.status.word, 2);

        model = match process_char_input(model, 'k', 3.0) { Model::Typing(m) => m, _ => panic!() };
        let final_model = process_char_input(model, 'i', 4.0);
        
        if let Model::Result(result_model) = final_model {
            assert_eq!(result_model.typing_model.status.word, 0);
            assert_eq!(result_model.typing_model.status.line, 1);
        } else {
            panic!("Model should be Result after finishing the problem");
        }
    }
    
    #[test]
    fn test_full_completion_and_result_transition() {
        let model = setup_model("#title test\n(あ/a)");
        let final_model = process_char_input(model, 'a', 0.0);
        assert!(matches!(final_model, Model::Result(_)));
    }

    #[test]
    fn test_metrics_calculation_with_corrections() {
        let mut model = setup_model("#title test\n(かき/kaki)");
        
        model = match process_char_input(model, 'k', 100.0) { Model::Typing(m) => m, _ => panic!() };
        model = match process_char_input(model, 'o', 200.0) { Model::Typing(m) => m, _ => panic!() }; // ミス
        model = process_backspace(model, 300.0); // 修正
        model = match process_char_input(model, 'a', 400.0) { Model::Typing(m) => m, _ => panic!() };
        model = match process_char_input(model, 'k', 500.0) { Model::Typing(m) => m, _ => panic!() };
        let final_model = process_char_input(model, 'i', 600.0);

        if let Model::Result(result_model) = final_model {
            assert!(is_typing_finished(&result_model.typing_model));
            let metrics = calculate_total_metrics(&result_model.typing_model);

            assert_eq!(metrics.type_count, 5);
            assert_eq!(metrics.miss_count, 1);
            assert_eq!(metrics.backspace_count, 1);
            assert_eq!(metrics.total_time, 500.0);
            assert!((metrics.accuracy - 0.8).abs() < f64::EPSILON);
            assert!((metrics.speed - 8.0).abs() < f64::EPSILON);
        } else {
            panic!("Model should have transitioned to Result");
        }
    }

    #[test]
    fn test_alternative_romaji_notations() {
        let mut model_shi = setup_model("#title test\n(し/shi)");
        model_shi = match process_char_input(model_shi, 's', 0.0) { Model::Typing(m) => m, _ => panic!() };
        model_shi = match process_char_input(model_shi, 'h', 1.0) { Model::Typing(m) => m, _ => panic!() };
        let final_model_shi = process_char_input(model_shi, 'i', 2.0);
        assert!(matches!(final_model_shi, Model::Result(_)), "Failed with 'shi'");

        let mut model_si = setup_model("#title test\n(し/shi)");
        model_si = match process_char_input(model_si, 's', 0.0) { Model::Typing(m) => m, _ => panic!() };
        let final_model_si = process_char_input(model_si, 'i', 1.0);
        assert!(matches!(final_model_si, Model::Result(_)), "Failed with 'si'");
    }

    #[test]
    fn test_sokuon_typing() {
        let mut model = setup_model("#title test\n(かっぱ/kappa)");
        model = match process_char_input(model, 'k', 0.0) { Model::Typing(m) => m, _ => panic!() };
        model = match process_char_input(model, 'a', 1.0) { Model::Typing(m) => m, _ => panic!() };
        model = match process_char_input(model, 'p', 2.0) { Model::Typing(m) => m, _ => panic!() };
        assert_eq!(model.status.current_word_input, "kap");
        model = match process_char_input(model, 'p', 3.0) { Model::Typing(m) => m, _ => panic!() };
        assert_eq!(model.status.current_word_input, "kapp");
        let final_model = process_char_input(model, 'a', 4.0);
        assert!(matches!(final_model, Model::Result(_)), "Sokuon 'kappa' failed");
    }

    #[test]
    fn test_hatsuon_typing() {
        let mut model_nn = setup_model("#title test\n(かん/kann)");
        model_nn = match process_char_input(model_nn, 'k', 0.0) { Model::Typing(m) => m, _ => panic!() };
        model_nn = match process_char_input(model_nn, 'a', 1.0) { Model::Typing(m) => m, _ => panic!() };
        model_nn = match process_char_input(model_nn, 'n', 2.0) { Model::Typing(m) => m, _ => panic!() };
        let final_model_nn = process_char_input(model_nn, 'n', 3.0);
        assert!(matches!(final_model_nn, Model::Result(_)), "Hatsuon 'kann' failed");

        let mut model_n_prime = setup_model("#title test\n(かんい/kan'i)");
        model_n_prime = match process_char_input(model_n_prime, 'k', 0.0) { Model::Typing(m) => m, _ => panic!() };
        model_n_prime = match process_char_input(model_n_prime, 'a', 1.0) { Model::Typing(m) => m, _ => panic!() };
        model_n_prime = match process_char_input(model_n_prime, 'n', 2.0) { Model::Typing(m) => m, _ => panic!() };
        model_n_prime = match process_char_input(model_n_prime, '\'', 3.0) { Model::Typing(m) => m, _ => panic!() };
        let final_model_n_prime = process_char_input(model_n_prime, 'i', 4.0);
        assert!(matches!(final_model_n_prime, Model::Result(_)), "Hatsuon 'kan'i' failed");
    }
    
    #[test]
    fn test_combined_word_with_okurigana() {
        let model = setup_model("#title test\n(悲/かな)-しき");
        assert_eq!(get_current_word_target_kana(&model), "かなしき");

        match process_string_input(model, "kanashiki".to_string(), 0.0) {
            Model::Result(_) => (),
            _ => panic!("Combined word 'kanashiki' should complete the problem"),
        }
    }

    #[test]
    fn test_process_string_input_ime_simulation() {
        let mut model = setup_model("#title test\n(こんにちは/konnichiha)");

        model = match process_string_input(model, "k".to_string(), 0.0) { Model::Typing(m) => m, _ => panic!() };
        assert_eq!(model.status.current_word_input, "k");

        model = match process_string_input(model, "ko".to_string(), 1.0) { Model::Typing(m) => m, _ => panic!() };
        assert_eq!(model.status.current_word_input, "ko");
        
        model = match process_string_input(model, "konn".to_string(), 2.0) { Model::Typing(m) => m, _ => panic!() };
        assert_eq!(model.status.current_word_input, "konn");

        model = match process_string_input(model, "konnici".to_string(), 3.0) { Model::Typing(m) => m, _ => panic!() };
        assert_eq!(model.status.current_word_input, "konnici");
        assert!(!is_typing_finished(&model));

        let final_model = process_string_input(model, "konnichiha".to_string(), 4.0);
        assert!(matches!(final_model, Model::Result(_)));
    }

    #[test]
    fn test_string_input_backspace_simulation() {
        let mut model = setup_model("#title test\n(さかな/sakana)");

        model = match process_string_input(model, "sakaki".to_string(), 0.0) { Model::Typing(m) => m, _ => panic!() };
        assert_eq!(model.status.current_word_correctness.last(), Some(&TypingCorrectnessChar::Incorrect));
        assert_eq!(model.status.backspace_count, 0);

        model = match process_string_input(model, "sakan".to_string(), 1.0) { Model::Typing(m) => m, _ => panic!() };
        assert_eq!(model.status.backspace_count, 2);
        assert!(model.status.current_word_correctness.iter().all(|c| *c == TypingCorrectnessChar::Correct));

        let final_model = process_string_input(model, "sakana".to_string(), 2.0);
        if let Model::Result(result) = final_model {
             assert_eq!(result.typing_model.status.backspace_count, 2);
        } else {
            panic!("Model should be Result");
        }
    }
}