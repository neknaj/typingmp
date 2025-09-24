// ./src/typing.rs

#[cfg(feature = "uefi")]
extern crate alloc;

#[cfg(feature = "uefi")]
use alloc::{
    format,
    string::String,
    vec::Vec,
};
#[cfg(not(feature = "uefi"))]
use std::{
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
    
    let common_prefix_len = old_input.chars()
        .zip(new_input.chars())
        .take_while(|(a, b)| a == b)
        .count();

    let removed_char_count = old_input.chars().count() - common_prefix_len;
    if removed_char_count > 0 {
        model.status.backspace_count += removed_char_count as u32;
        for _ in 0..removed_char_count {
            log_physical_input(&mut model, '\u{08}', timestamp);
        }
    }

    let added_part_start_index = new_input.char_indices().nth(common_prefix_len).map_or(new_input.len(), |(i, _)| i);
    let added_part = &new_input[added_part_start_index..];
    for c in added_part.chars() {
        log_physical_input(&mut model, c, timestamp);
    }

    model.status.current_word_input = new_input;
    
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

/// ユーザーのローマ字入力を「かな」に変換し、どの入力範囲がどの「かな」に対応したかの情報も返す
fn convert_input_to_kana_info(user_input: &str, layout: &crate::model::Layout) -> (String, Vec<(usize, usize)>) {
    let mut converted_kana = String::new();
    let mut input_ranges = Vec::new();
    
    let mut current_pos = 0;
    while current_pos < user_input.len() {
        let remaining_input = &user_input[current_pos..];
        let mut best_match: (&str, &str) = ("", ""); // (romaji, kana)

        // 最長一致するローマ字を探す
        for (kana, romaji_vec) in &layout.mapping {
            for romaji in romaji_vec {
                if !romaji.is_empty() && remaining_input.starts_with(romaji) {
                    if romaji.len() > best_match.0.len() {
                        best_match = (romaji, kana);
                    }
                }
            }
        }
        
        if !best_match.0.is_empty() {
            converted_kana.push_str(best_match.1);
            let start = current_pos;
            current_pos += best_match.0.len();
            input_ranges.push((start, current_pos));
        } else {
            // マッチしなかった場合（入力途中など）は、残りをまとめて一つの範囲とする
            input_ranges.push((current_pos, user_input.len()));
            break;
        }
    }
    
    (converted_kana, input_ranges)
}


/// 現在の単語の目標となる正規化された「かな」文字列を取得する
fn get_current_word_target_kana(model: &TypingModel) -> String {
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


/// 現在の入力状況に応じて、モデルの進捗（セグメント、文字）と正誤を更新する
fn update_progress(model: &mut TypingModel) -> bool {
    let target_kana = get_current_word_target_kana(model);
    if target_kana.is_empty() {
        model.status.current_word_correctness.clear();
        return model.status.current_word_input.is_empty();
    }
    
    let (converted_kana, input_ranges) = convert_input_to_kana_info(&model.status.current_word_input, &model.layout);
    
    let is_correct_so_far = target_kana.starts_with(&converted_kana);
    let is_complete = is_correct_so_far && converted_kana.len() == target_kana.len() && input_ranges.last().map_or(true, |&(_, end)| end == model.status.current_word_input.len());

    // 正誤配列を更新
    model.status.current_word_correctness.clear();
    model.status.current_word_correctness.resize(model.status.current_word_input.len(), TypingCorrectnessChar::Incorrect);
    if is_correct_so_far {
        for (start, end) in input_ranges {
            for i in start..end {
                if i < model.status.current_word_correctness.len() {
                    model.status.current_word_correctness[i] = TypingCorrectnessChar::Correct;
                }
            }
        }
    }

    // UIカーソル位置を進める
    let mut chars_to_advance = converted_kana.chars().count();
     if !is_correct_so_far {
        // 間違っている場合、最後に正しかった位置までカーソルを戻す
        let common_prefix_len = target_kana.chars().zip(converted_kana.chars()).take_while(|(a, b)| a == b).count();
        chars_to_advance = common_prefix_len;
    }
    
    let mut seg_idx = 0;
    let mut char_idx = 0;
    if let Some(line_content) = model.content.lines.get(model.status.line as usize) {
        if let Some(word_content) = line_content.words.get(model.status.word as usize) {
            'outer: for (i, seg) in word_content.segments.iter().enumerate() {
                let seg_reading_kana = match seg {
                    Segment::Plain { text } => text,
                    Segment::Annotated { reading, .. } => reading,
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
        .map(|seg| match seg {
            Segment::Plain { text } => text.clone(),
            Segment::Annotated { reading, .. } => reading.clone(),
        }).collect();
    log(&format!("[METRICS] Total target kana: '{}'", total_target_kana));

    let (converted_kana, _) = convert_input_to_kana_info(&final_input_str, &model.layout);
    let correct_kana_count = total_target_kana.chars().zip(converted_kana.chars()).take_while(|(a, b)| a == b).count();

    // 正しくタイプされたローマ字数を概算
    let (_, correct_input_ranges) = convert_input_to_kana_info(&final_input_str, &model.layout);
    let correct_type_count = if correct_kana_count > 0 && correct_kana_count <= correct_input_ranges.len() {
        correct_input_ranges[correct_kana_count - 1].1 as i32
    } else {
        0
    };
    
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

    #[test]
    fn test_single_char_input_correct() {
        let model = setup_model("#title test\n(か/か)");
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
        let model = setup_model("#title test\n(か/か)");
        let model = match process_char_input(model, 't', 0.0) {
            Model::Typing(m) => m,
            _ => panic!("Should be Typing model"),
        };
        assert_eq!(model.status.current_word_input, "t");
        assert_eq!(model.status.current_word_correctness, vec![TypingCorrectnessChar::Incorrect]);
    }
    
    #[test]
    fn test_backspace_correction() {
        let mut model = setup_model("#title test\n(か/か)");
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
        let mut model = setup_model("#title test\n(か/か) (き/き)");
        model = match process_char_input(model, 'k', 0.0) { Model::Typing(m) => m, _ => panic!() };
        model = match process_char_input(model, 'a', 1.0) { Model::Typing(m) => m, _ => panic!() };
        assert_eq!(model.status.word, 1);

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
        let model = setup_model("#title test\n(あ/あ)");
        let final_model = process_char_input(model, 'a', 0.0);
        assert!(matches!(final_model, Model::Result(_)));
    }

    #[test]
    fn test_metrics_calculation_with_corrections() {
        let mut model = setup_model("#title test\n(かき/かき)");
        
        model = match process_char_input(model, 'k', 100.0) { Model::Typing(m) => m, _ => panic!() };
        model = match process_char_input(model, 'o', 200.0) { Model::Typing(m) => m, _ => panic!() }; // ミス
        model = process_backspace(model, 300.0); // 修正
        model = match process_char_input(model, 'a', 400.0) { Model::Typing(m) => m, _ => panic!() };
        model = match process_char_input(model, 'k', 500.0) { Model::Typing(m) => m, _ => panic!() };
        let final_model = process_char_input(model, 'i', 600.0);

        if let Model::Result(result_model) = final_model {
            let metrics = calculate_total_metrics(&result_model.typing_model);

            assert_eq!(metrics.type_count, 5);
            // 最終入力 "kaki" は "かき" にマッチ。入力文字数4。よってミスは 5 - 4 = 1
            assert_eq!(metrics.miss_count, 1); 
            assert_eq!(metrics.backspace_count, 1);
            assert_eq!(metrics.total_time, 500.0);
            assert!((metrics.accuracy - 0.8).abs() < f64::EPSILON); // (5-1)/5 = 0.8
            assert!((metrics.speed - 8.0).abs() < f64::EPSILON); // 4 correct / 0.5s = 8
        } else {
            panic!("Model should have transitioned to Result");
        }
    }

    #[test]
    fn test_alternative_romaji_notations() {
        let mut model_shi = setup_model("#title test\n(し/し)");
        model_shi = match process_char_input(model_shi, 's', 0.0) { Model::Typing(m) => m, _ => panic!() };
        model_shi = match process_char_input(model_shi, 'h', 1.0) { Model::Typing(m) => m, _ => panic!() };
        let final_model_shi = process_char_input(model_shi, 'i', 2.0);
        assert!(matches!(final_model_shi, Model::Result(_)), "Failed with 'shi'");

        let mut model_si = setup_model("#title test\n(し/し)");
        model_si = match process_char_input(model_si, 's', 0.0) { Model::Typing(m) => m, _ => panic!() };
        let final_model_si = process_char_input(model_si, 'i', 1.0);
        assert!(matches!(final_model_si, Model::Result(_)), "Failed with 'si'");
    }

    #[test]
    fn test_sokuon_typing() {
        let mut model = setup_model("#title test\n(かっぱ/かっぱ)");
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
        let mut model_nn = setup_model("#title test\n(かん/かん)");
        model_nn = match process_char_input(model_nn, 'k', 0.0) { Model::Typing(m) => m, _ => panic!() };
        model_nn = match process_char_input(model_nn, 'a', 1.0) { Model::Typing(m) => m, _ => panic!() };
        model_nn = match process_char_input(model_nn, 'n', 2.0) { Model::Typing(m) => m, _ => panic!() };
        let final_model_nn = process_char_input(model_nn, 'n', 3.0);
        assert!(matches!(final_model_nn, Model::Result(_)), "Hatsuon 'kann' failed");

        let mut model_n_prime = setup_model("#title test\n(かんい/かんい)");
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
        let mut model = setup_model("#title test\n(こんにちは/こんにちは)");

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
        let mut model = setup_model("#title test\n(さかな/さかな)");

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