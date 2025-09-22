// src/model.rs

// uefi featureが有効な場合、no_stdとno_mainでコンパイルする
#[cfg(feature = "uefi")]
extern crate alloc;

#[cfg(feature = "uefi")]
use alloc::{string::String, vec, vec::Vec};
#[cfg(not(feature = "uefi"))]
use std::string::String;
#[cfg(not(feature = "uefi"))]
use std::vec::Vec; // Vecはstringではなくvecクレートから

#[cfg(not(feature = "uefi"))]
use std::fmt;

#[cfg(feature = "uefi")]
use core::fmt;

use crate::layout_data;

// (Content, Line, Segment, Model, TypingModel, etc. の定義は変更なし)
// ...

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
    pub mapping: Vec<(String, Vec<String>)>,
}

#[derive(Debug, Clone)]
pub struct Scroll {
    pub scroll: f64,
    pub max: f64,
}

impl Default for Layout {
    fn default() -> Self {
        Layout {
            mapping: layout_data::get_layout(),
        }
    }
}