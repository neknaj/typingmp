extern crate alloc;

use crate::model::Segment;
use ab_glyph::FontVec;
use alloc::string::String;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontScript {
    Japanese,
    SimplifiedChinese,
    TraditionalChinese,
}

impl FontScript {
    pub fn label(self) -> &'static str {
        match self {
            Self::Japanese => "Japanese",
            Self::SimplifiedChinese => "Simplified Chinese",
            Self::TraditionalChinese => "Traditional Chinese",
        }
    }

    pub fn settings_label(self) -> &'static str {
        match self {
            Self::Japanese => "Japanese Font",
            Self::SimplifiedChinese => "Simplified Chinese Font",
            Self::TraditionalChinese => "Traditional Chinese Font",
        }
    }
}

pub struct Fonts {
    japanese: FontVec,
    simplified_chinese: FontVec,
    traditional_chinese: FontVec,
    generation: u64,
}

impl Fonts {
    pub const fn new(
        japanese: FontVec,
        simplified_chinese: FontVec,
        traditional_chinese: FontVec,
    ) -> Self {
        Self {
            japanese,
            simplified_chinese,
            traditional_chinese,
            generation: 0,
        }
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }

    pub fn get_for_script(&self, script: FontScript) -> &FontVec {
        match script {
            FontScript::Japanese => &self.japanese,
            FontScript::SimplifiedChinese => &self.simplified_chinese,
            FontScript::TraditionalChinese => &self.traditional_chinese,
        }
    }

    pub fn set_for_script(&mut self, script: FontScript, font: FontVec) {
        match script {
            FontScript::Japanese => self.japanese = font,
            FontScript::SimplifiedChinese => self.simplified_chinese = font,
            FontScript::TraditionalChinese => self.traditional_chinese = font,
        }
        self.generation = self.generation.wrapping_add(1);
    }

    pub fn primary(&self) -> &FontVec {
        &self.japanese
    }
}

pub fn script_for_segment(segment: &Segment) -> FontScript {
    let reading = segment_reading_text(segment);
    script_for_reading(&reading)
}

pub fn script_for_reading(reading: &str) -> FontScript {
    let mut has_kana = false;
    let mut has_latin = false;

    for character in reading.chars() {
        if is_bopomofo(character) {
            return FontScript::TraditionalChinese;
        }
        if is_kana(character) {
            has_kana = true;
        }
        if is_latin_pinyin(character) {
            has_latin = true;
        }
    }

    if has_kana {
        FontScript::Japanese
    } else if has_latin {
        FontScript::SimplifiedChinese
    } else {
        FontScript::Japanese
    }
}

fn segment_reading_text(segment: &Segment) -> String {
    match segment {
        Segment::Plain { text } => text.clone(),
        Segment::Annotated { reading, .. } => reading.clone(),
        Segment::Anno { inner, .. } => inner.iter().map(segment_reading_text).collect(),
    }
}

fn is_kana(character: char) -> bool {
    matches!(
        character as u32,
        0x3040..=0x309F | 0x30A0..=0x30FF | 0x31F0..=0x31FF | 0xFF66..=0xFF9D
    )
}

fn is_bopomofo(character: char) -> bool {
    matches!(character as u32, 0x3100..=0x312F | 0x31A0..=0x31BF)
}

fn is_latin_pinyin(character: char) -> bool {
    character.is_ascii_alphabetic() || matches!(character as u32, 0x00C0..=0x024F | 0x0300..=0x036F)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reading_script_detects_kana_as_japanese() {
        assert_eq!(script_for_reading("かな"), FontScript::Japanese);
        assert_eq!(script_for_reading("カナ"), FontScript::Japanese);
    }

    #[test]
    fn reading_script_detects_pinyin_as_simplified_chinese() {
        assert_eq!(script_for_reading("hanzi"), FontScript::SimplifiedChinese);
        assert_eq!(script_for_reading("hànzì"), FontScript::SimplifiedChinese);
    }

    #[test]
    fn reading_script_detects_bopomofo_as_traditional_chinese() {
        assert_eq!(
            script_for_reading("ㄏㄢˋㄗˋ"),
            FontScript::TraditionalChinese
        );
    }

    #[test]
    fn reading_script_falls_back_to_japanese_for_plain_text() {
        assert_eq!(script_for_reading("漢字"), FontScript::Japanese);
    }

    #[test]
    fn segment_script_uses_inner_reading_for_annotation_segments() {
        let segment = Segment::Anno {
            inner: alloc::vec![Segment::Annotated {
                base: "漢字".into(),
                reading: "hanzi".into(),
            }],
            annotation: "Chinese".into(),
        };

        assert_eq!(script_for_segment(&segment), FontScript::SimplifiedChinese);
    }
}
