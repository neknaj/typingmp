// src/model.rs

extern crate alloc;

use alloc::{string::String, vec, vec::Vec};
use core::fmt;

use crate::layout_data;

#[derive(Debug, Clone, PartialEq)]
pub struct Content {
    pub title: Line,
    pub lines: Vec<Line>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub words: Vec<Word>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Word {
    pub segments: Vec<Segment>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Segment {
    /// プレーンテキスト（読みなし）
    Plain { text: String },
    /// ルビ付きテキスト（[base/reading] 記法）
    Annotated { base: String, reading: String },
    /// アノテーション付きテキスト（{inner/annotation} 記法）
    /// inner が入力対象、annotation は表示専用
    Anno {
        inner: Vec<Segment>,
        annotation: String,
    },
}

impl fmt::Display for Line {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for word in &self.words {
            for segment in &word.segments {
                write!(f, "{}", segment)?;
            }
        }
        Ok(())
    }
}

impl fmt::Display for Segment {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Segment::Plain { text } => write!(f, "{}", text),
            Segment::Annotated { base, .. } => write!(f, "{}", base),
            Segment::Anno { inner, .. } => {
                for seg in inner {
                    write!(f, "{}", seg)?;
                }
                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum Model {
    Typing(TypingModel),
    Result(ResultModel),
}

#[derive(Debug, Clone)]
pub struct TypingModel {
    pub content: Content,
    pub status: TypingStatus,
    pub user_input: Vec<TypingSession>,
    pub total_type_count: i32,
    pub total_miss_count: i32,
    pub first_input_time: Option<f64>,
    pub last_input_time: Option<f64>,
    pub typing_correctness: TypingCorrectnessContent,
    pub layout: Layout,
    pub scroll: Scroll,
}

#[derive(Debug, Clone)]
pub struct ResultModel {
    pub typing_model: TypingModel,
}

#[derive(Debug, Clone)]
pub struct TypingStatus {
    pub line: i32,
    pub word: i32,
    pub segment: i32,
    pub char_: i32,
    pub unconfirmed: Vec<char>,
    pub last_wrong_keydown: Option<char>,
}

#[derive(Debug, Clone)]
pub struct TypingSession {
    pub line: i32,
    pub inputs: Vec<TypingInput>,
}

#[derive(Debug, Clone)]
pub struct TypingInput {
    pub key: char,
    pub timestamp: f64,
    pub is_correct: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypingCorrectnessChar {
    Pending,
    Correct,
    Incorrect,
}

#[derive(Debug, Clone)]
pub struct TypingCorrectnessContent {
    pub lines: Vec<TypingCorrectnessLine>,
}

#[derive(Debug, Clone)]
pub struct TypingCorrectnessLine {
    pub words: Vec<TypingCorrectnessWord>,
}

#[derive(Debug, Clone)]
pub struct TypingCorrectnessWord {
    pub segments: Vec<TypingCorrectnessSegment>,
}

#[derive(Debug, Clone)]
pub struct TypingCorrectnessSegment {
    pub chars: Vec<TypingCorrectnessChar>,
}

#[derive(Debug, Clone)]
pub struct TypingMetrics {
    pub miss_count: i32,
    pub type_count: i32,
    pub total_time: f64,
    pub accuracy: f64,
    pub speed: f64, // Chars per second
}

#[derive(Debug, Clone)]
pub struct Layout {
    pub mapping: Vec<(String, Vec<String>)>,
    pub normalized_mapping: Vec<(String, Vec<String>)>,
    pub normalized_mapping_max_key_len: usize,
    pub normalized_mapping_by_first_char: Vec<Vec<usize>>,
}

#[derive(Debug, Clone)]
pub struct Scroll {
    pub scroll: f64,
    pub max: f64,
}

impl Default for Layout {
    fn default() -> Self {
        let mapping: Vec<(String, Vec<String>)> = layout_data::get_layout();
        let normalized_mapping: Vec<(String, Vec<String>)> = mapping
            .iter()
            .map(|(key, values)| {
                (
                    key.to_ascii_lowercase(),
                    values
                        .iter()
                        .map(|value| value.to_ascii_lowercase())
                        .collect(),
                )
            })
            .collect();
        let normalized_mapping_max_key_len = normalized_mapping
            .iter()
            .map(|(key, _): &(String, Vec<String>)| key.chars().count())
            .max()
            .unwrap_or(1);
        let mut normalized_mapping_by_first_char = vec![Vec::new(); 256];
        for (index, (key, _)) in normalized_mapping.iter().enumerate() {
            match key.as_bytes().first().copied() {
                Some(first_byte) => {
                    normalized_mapping_by_first_char[first_byte as usize].push(index);
                }
                None => normalized_mapping_by_first_char[0].push(index),
            }
        }

        Layout {
            mapping,
            normalized_mapping,
            normalized_mapping_max_key_len,
            normalized_mapping_by_first_char,
        }
    }
}
