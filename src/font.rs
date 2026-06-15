extern crate alloc;

use crate::model::{Line, Segment};
use ab_glyph::FontVec;
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

pub const DEFAULT_JAPANESE_FONT_NAME: &str = "YujiSyuku";
pub const DEFAULT_CHINESE_SIMPLIFIED_FONT_NAME: &str = "MaShanZheng";
pub const DEFAULT_TRADITIONAL_CHINESE_FONT_NAME: &str = "MaShanZheng";
pub const DEFAULT_ENGLISH_FONT_NAME: &str = "Kalam";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontScript {
    Japanese,
    ChineseSimplified,
    TraditionalChinese,
    English,
}

impl FontScript {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Japanese => "Japanese",
            Self::ChineseSimplified => "Chinese Simplified",
            Self::TraditionalChinese => "Traditional Chinese",
            Self::English => "English",
        }
    }

    pub const fn settings_label(self) -> &'static str {
        match self {
            Self::Japanese => "Japanese Font",
            Self::ChineseSimplified => "Chinese Simplified Font",
            Self::TraditionalChinese => "Traditional Chinese Font",
            Self::English => "English Font",
        }
    }
}

pub struct Fonts {
    japanese: FontSlot,
    chinese_simplified: FontSlot,
    traditional_chinese: FontSlot,
    english: FontSlot,
    generation: u64,
}

struct FontSlot {
    font: FontVec,
    name: String,
}

impl FontSlot {
    fn new(font: FontVec, name: &str) -> Self {
        Self {
            font,
            name: name.to_string(),
        }
    }
}

impl Fonts {
    pub fn new(
        japanese: FontVec,
        chinese_simplified: FontVec,
        traditional_chinese: FontVec,
        english: FontVec,
    ) -> Self {
        Self {
            japanese: FontSlot::new(japanese, DEFAULT_JAPANESE_FONT_NAME),
            chinese_simplified: FontSlot::new(
                chinese_simplified,
                DEFAULT_CHINESE_SIMPLIFIED_FONT_NAME,
            ),
            traditional_chinese: FontSlot::new(
                traditional_chinese,
                DEFAULT_TRADITIONAL_CHINESE_FONT_NAME,
            ),
            english: FontSlot::new(english, DEFAULT_ENGLISH_FONT_NAME),
            generation: 0,
        }
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }

    pub fn get_for_script(&self, script: FontScript) -> &FontVec {
        &self.slot_for_script(script).font
    }

    pub fn name_for_script(&self, script: FontScript) -> &str {
        &self.slot_for_script(script).name
    }

    pub fn set_for_script(&mut self, script: FontScript, name: String, font: FontVec) {
        let slot = self.slot_for_script_mut(script);
        slot.font = font;
        slot.name = name;
        self.generation = self.generation.wrapping_add(1);
    }

    pub fn primary(&self) -> &FontVec {
        &self.japanese.font
    }

    fn slot_for_script(&self, script: FontScript) -> &FontSlot {
        match script {
            FontScript::Japanese => &self.japanese,
            FontScript::ChineseSimplified => &self.chinese_simplified,
            FontScript::TraditionalChinese => &self.traditional_chinese,
            FontScript::English => &self.english,
        }
    }

    fn slot_for_script_mut(&mut self, script: FontScript) -> &mut FontSlot {
        match script {
            FontScript::Japanese => &mut self.japanese,
            FontScript::ChineseSimplified => &mut self.chinese_simplified,
            FontScript::TraditionalChinese => &mut self.traditional_chinese,
            FontScript::English => &mut self.english,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextScriptRun {
    pub text: String,
    pub script: FontScript,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentScriptRun {
    pub base_text: String,
    pub reading_text: String,
    pub ruby_text: Option<String>,
    pub script: FontScript,
}

pub fn script_for_segment(segment: &Segment) -> FontScript {
    match segment {
        Segment::Plain { text } => classify_plain_text(text, None),
        Segment::Annotated { reading, .. } => classify_annotated_reading(reading),
        Segment::Anno { inner, .. } => scripts_for_segments(inner)
            .into_iter()
            .next()
            .unwrap_or(FontScript::Japanese),
    }
}

pub fn scripts_for_line(line: &Line) -> Vec<FontScript> {
    let segments = line
        .words
        .iter()
        .flat_map(|word| word.segments.iter())
        .collect::<Vec<_>>();
    scripts_for_segment_refs(&segments)
}

pub fn scripts_for_segments(segments: &[Segment]) -> Vec<FontScript> {
    let refs = segments.iter().collect::<Vec<_>>();
    scripts_for_segment_refs(&refs)
}

pub fn segment_script_runs(segment: &Segment, context: FontScript) -> Vec<SegmentScriptRun> {
    match segment {
        Segment::Plain { text } => plain_text_script_runs(text, Some(context))
            .into_iter()
            .map(|run| SegmentScriptRun {
                reading_text: run.text.clone(),
                base_text: run.text,
                ruby_text: None,
                script: run.script,
            })
            .collect(),
        Segment::Annotated { base, reading } => {
            let script = classify_annotated_reading(reading);
            alloc::vec![SegmentScriptRun {
                base_text: base.clone(),
                reading_text: reading.clone(),
                ruby_text: Some(reading.clone()),
                script,
            }]
        }
        Segment::Anno { inner, .. } => segment_runs_for_segments(inner),
    }
}

pub fn segment_runs_for_segments(segments: &[Segment]) -> Vec<SegmentScriptRun> {
    let scripts = scripts_for_segments(segments);
    let mut runs = Vec::new();

    for (index, segment) in segments.iter().enumerate() {
        let script = scripts
            .get(index)
            .copied()
            .unwrap_or_else(|| script_for_segment(segment));
        runs.extend(segment_script_runs(segment, script));
    }

    runs
}

fn scripts_for_segment_refs(segments: &[&Segment]) -> Vec<FontScript> {
    let mut scripts = segments
        .iter()
        .map(|segment| classify_segment_without_context(segment))
        .collect::<Vec<_>>();

    for index in 0..scripts.len() {
        if scripts[index].is_some() {
            continue;
        }
        scripts[index] = nearest_context_script(&scripts, index).or(Some(FontScript::Japanese));
    }

    scripts
        .into_iter()
        .map(|script| script.unwrap_or(FontScript::Japanese))
        .collect()
}

fn classify_segment_without_context(segment: &Segment) -> Option<FontScript> {
    match segment {
        Segment::Plain { text } => classify_plain_text_without_context(text),
        Segment::Annotated { reading, .. } => Some(classify_annotated_reading(reading)),
        Segment::Anno { inner, .. } => scripts_for_segments(inner).into_iter().next(),
    }
}

fn nearest_context_script(scripts: &[Option<FontScript>], index: usize) -> Option<FontScript> {
    if let Some(script) = scripts[..index].iter().rev().find_map(|script| *script) {
        return Some(script);
    }
    scripts[index + 1..].iter().find_map(|script| *script)
}

pub fn classify_annotated_reading(reading: &str) -> FontScript {
    let mut has_kana = false;
    let mut has_pinyin = false;

    for character in reading.chars() {
        if is_bopomofo(character) {
            return FontScript::TraditionalChinese;
        }
        if is_kana(character) {
            has_kana = true;
        }
        if is_pinyin_latin(character) {
            has_pinyin = true;
        }
    }

    if has_kana {
        FontScript::Japanese
    } else if has_pinyin {
        FontScript::ChineseSimplified
    } else {
        FontScript::Japanese
    }
}

pub fn classify_plain_text(text: &str, context: Option<FontScript>) -> FontScript {
    classify_plain_text_without_context(text)
        .or(context)
        .unwrap_or(FontScript::Japanese)
}

pub fn plain_text_script_runs(text: &str, context: Option<FontScript>) -> Vec<TextScriptRun> {
    let characters = text.chars().collect::<Vec<_>>();
    let explicit_scripts = characters
        .iter()
        .copied()
        .map(classify_plain_character_without_context)
        .collect::<Vec<_>>();
    let mut runs = Vec::new();
    let mut current_text = String::new();
    let mut current_script = None;
    let mut previous_script = None;

    for (index, character) in characters.iter().copied().enumerate() {
        let script = if character.is_whitespace() {
            previous_script
                .or(context)
                .or_else(|| {
                    explicit_scripts[index + 1..]
                        .iter()
                        .find_map(|script| *script)
                })
                .unwrap_or(FontScript::Japanese)
        } else if is_shared_cjk_punctuation(character) {
            inherited_cjk_punctuation_script(previous_script)
                .or_else(|| inherited_cjk_punctuation_script(context))
                .or_else(|| {
                    explicit_scripts[index + 1..]
                        .iter()
                        .find_map(|script| inherited_cjk_punctuation_script(*script))
                })
                .unwrap_or(FontScript::Japanese)
        } else {
            explicit_scripts[index]
                .or(previous_script)
                .or(context)
                .or_else(|| {
                    explicit_scripts[index + 1..]
                        .iter()
                        .find_map(|script| *script)
                })
                .unwrap_or(FontScript::Japanese)
        };

        if current_script.is_some_and(|current| current != script) {
            runs.push(TextScriptRun {
                text: current_text,
                script: current_script.expect("script should exist when run text exists"),
            });
            current_text = String::new();
        }

        current_text.push(character);
        current_script = Some(script);
        previous_script = Some(script);
    }

    if let Some(script) = current_script {
        runs.push(TextScriptRun {
            text: current_text,
            script,
        });
    }

    runs
}

fn inherited_cjk_punctuation_script(script: Option<FontScript>) -> Option<FontScript> {
    match script {
        Some(FontScript::Japanese) => Some(FontScript::Japanese),
        Some(FontScript::ChineseSimplified) => Some(FontScript::ChineseSimplified),
        Some(FontScript::TraditionalChinese) => Some(FontScript::TraditionalChinese),
        Some(FontScript::English) | None => None,
    }
}

fn classify_plain_text_without_context(text: &str) -> Option<FontScript> {
    let mut has_cjk_ideograph = false;

    for character in text.chars() {
        if character.is_whitespace() || is_shared_cjk_punctuation(character) {
            continue;
        }
        if let Some(script) = classify_plain_character_without_context(character) {
            return Some(script);
        }
        if is_cjk_ideograph(character) {
            has_cjk_ideograph = true;
        }
    }

    has_cjk_ideograph.then_some(FontScript::Japanese)
}

fn classify_plain_character_without_context(character: char) -> Option<FontScript> {
    if character.is_whitespace() || is_shared_cjk_punctuation(character) {
        return None;
    }
    if is_bopomofo(character) {
        return Some(FontScript::TraditionalChinese);
    }
    if is_kana(character) || is_japanese_specific_mark(character) {
        return Some(FontScript::Japanese);
    }
    if character.is_ascii_alphanumeric()
        || character.is_ascii_punctuation()
        || is_latin_letter(character)
    {
        return Some(FontScript::English);
    }
    if is_cjk_ideograph(character) {
        return Some(FontScript::Japanese);
    }
    Some(FontScript::Japanese)
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

fn is_pinyin_latin(character: char) -> bool {
    character.is_ascii_alphabetic() || matches!(character as u32, 0x00C0..=0x024F | 0x0300..=0x036F)
}

fn is_latin_letter(character: char) -> bool {
    character.is_ascii_alphabetic() || matches!(character as u32, 0x00C0..=0x024F)
}

fn is_cjk_ideograph(character: char) -> bool {
    matches!(
        character as u32,
        0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0x20000..=0x2FA1F
    )
}

fn is_shared_cjk_punctuation(character: char) -> bool {
    matches!(
        character,
        '。' | '、'
            | '，'
            | '．'
            | '！'
            | '？'
            | '：'
            | '；'
            | '「'
            | '」'
            | '『'
            | '』'
            | '（'
            | '）'
            | '［'
            | '］'
            | '｛'
            | '｝'
            | '《'
            | '》'
            | '〈'
            | '〉'
            | '【'
            | '】'
            | '〔'
            | '〕'
            | '“'
            | '”'
            | '‘'
            | '’'
            | '…'
            | '—'
            | '～'
            | '・'
            | '·'
    )
}

fn is_japanese_specific_mark(character: char) -> bool {
    matches!(character, '々' | '〆' | '〻' | 'ゝ' | 'ゞ' | 'ヽ' | 'ヾ')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Line, Segment, Word};

    #[test]
    fn annotated_reading_detects_kana_as_japanese() {
        assert_eq!(classify_annotated_reading("かな"), FontScript::Japanese);
        assert_eq!(classify_annotated_reading("カナ"), FontScript::Japanese);
    }

    #[test]
    fn annotated_reading_detects_pinyin_as_simplified_chinese() {
        assert_eq!(
            classify_annotated_reading("hanzi"),
            FontScript::ChineseSimplified
        );
        assert_eq!(
            classify_annotated_reading("hànzì"),
            FontScript::ChineseSimplified
        );
    }

    #[test]
    fn annotated_reading_detects_bopomofo_as_traditional_chinese() {
        assert_eq!(
            classify_annotated_reading("ㄏㄢˋㄗˋ"),
            FontScript::TraditionalChinese
        );
    }

    #[test]
    fn plain_text_detects_english_text_as_english() {
        assert_eq!(
            classify_plain_text("MIT License", None),
            FontScript::English
        );
        assert_eq!(classify_plain_text("Hello!", None), FontScript::English);
    }

    #[test]
    fn plain_text_keeps_ambiguous_cjk_as_japanese() {
        assert_eq!(classify_plain_text("漢字", None), FontScript::Japanese);
    }

    #[test]
    fn shared_punctuation_inherits_line_context() {
        let line = Line {
            words: vec![Word {
                segments: vec![
                    Segment::Annotated {
                        base: "汉".into(),
                        reading: "han".into(),
                    },
                    Segment::Plain { text: "。".into() },
                    Segment::Annotated {
                        base: "字".into(),
                        reading: "ㄗˋ".into(),
                    },
                    Segment::Plain { text: "？".into() },
                ],
            }],
        };

        assert_eq!(
            scripts_for_line(&line),
            vec![
                FontScript::ChineseSimplified,
                FontScript::ChineseSimplified,
                FontScript::TraditionalChinese,
                FontScript::TraditionalChinese,
            ]
        );
    }

    #[test]
    fn plain_runs_split_mixed_japanese_and_english() {
        assert_eq!(
            plain_text_script_runs("日本ABC!", None),
            vec![
                TextScriptRun {
                    text: "日本".into(),
                    script: FontScript::Japanese,
                },
                TextScriptRun {
                    text: "ABC!".into(),
                    script: FontScript::English,
                },
            ]
        );
    }

    #[test]
    fn punctuation_only_plain_run_uses_context() {
        assert_eq!(
            plain_text_script_runs("。", Some(FontScript::ChineseSimplified)),
            vec![TextScriptRun {
                text: "。".into(),
                script: FontScript::ChineseSimplified,
            }]
        );
    }

    #[test]
    fn shared_cjk_punctuation_does_not_inherit_english() {
        assert_eq!(
            plain_text_script_runs("ABC。", None),
            vec![
                TextScriptRun {
                    text: "ABC".into(),
                    script: FontScript::English,
                },
                TextScriptRun {
                    text: "。".into(),
                    script: FontScript::Japanese,
                },
            ]
        );
    }

    #[test]
    fn whitespace_inherits_english_context() {
        assert_eq!(
            plain_text_script_runs("ABC DEF", None),
            vec![TextScriptRun {
                text: "ABC DEF".into(),
                script: FontScript::English,
            }]
        );
    }

    #[test]
    fn anno_inner_keeps_mixed_script_runs() {
        let segment = Segment::Anno {
            inner: alloc::vec![
                Segment::Annotated {
                    base: "秋".into(),
                    reading: "あき".into(),
                },
                Segment::Annotated {
                    base: "汉".into(),
                    reading: "han".into(),
                },
                Segment::Annotated {
                    base: "字".into(),
                    reading: "ㄗˋ".into(),
                },
            ],
            annotation: "mixed".into(),
        };
        let scripts = segment_script_runs(&segment, FontScript::Japanese)
            .into_iter()
            .map(|run| run.script)
            .collect::<Vec<_>>();

        assert_eq!(
            scripts,
            vec![
                FontScript::Japanese,
                FontScript::ChineseSimplified,
                FontScript::TraditionalChinese,
            ]
        );
    }

    #[test]
    fn segment_script_uses_inner_reading_for_annotation_segments() {
        let segment = Segment::Anno {
            inner: alloc::vec![Segment::Annotated {
                base: "汉字".into(),
                reading: "hanzi".into(),
            }],
            annotation: "Chinese".into(),
        };

        assert_eq!(script_for_segment(&segment), FontScript::ChineseSimplified);
    }
}
