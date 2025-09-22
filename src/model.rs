// src/model.rs

use std::collections::HashMap;
use std::fmt;
use serde_json; // serde_jsonをuseする

#[derive(Debug, Clone)]
pub struct Content {
    pub title: Line,
    pub lines: Vec<Line>,
}

#[derive(Debug, Clone)]
pub struct Line {
    pub segments: Vec<Segment>,
}

#[derive(Debug, Clone)]
pub enum Segment {
    Plain { text: String },
    Annotated { base: String, reading: String },
}

impl fmt::Display for Line {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for segment in &self.segments {
            write!(f, "{}", segment)?;
        }
        Ok(())
    }
}

impl fmt::Display for Segment {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Segment::Plain { text } => write!(f, "{}", text),
            Segment::Annotated { base, .. } => write!(f, "{}", base),
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
    // 旧コードのTextConvert.mappingに合わせる
    pub mapping: Vec<(String, Vec<String>)>,
}

#[derive(Debug, Clone)]
pub struct Scroll {
    pub scroll: f64,
    pub max: f64,
}

impl Default for Layout {
    fn default() -> Self {
        let json_data = include_str!("../assets/japanese.json");
        
        // HashMapとして一度デシリアライズ
        let mapping_hash: HashMap<String, Vec<String>> =
            serde_json::from_str(json_data).expect("Failed to parse japanese.json layout file.");
        
        // HashMapをVec<(String, Vec<String>)>に変換
        let mapping_vec = mapping_hash.into_iter().collect();

        Layout { mapping: mapping_vec }
    }
}