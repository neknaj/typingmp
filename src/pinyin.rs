extern crate alloc;

use alloc::{string::String, vec::Vec};

pub fn is_pinyin_like(text: &str) -> bool {
    let mut has_pinyin_letter = false;
    for character in text.chars() {
        if is_ascii_pinyin_letter(character) || is_marked_pinyin_letter(character) {
            has_pinyin_letter = true;
            continue;
        }
        if is_pinyin_tone_digit(character)
            || character.is_whitespace()
            || matches!(character, '\'' | '-' | '’')
        {
            continue;
        }
        return false;
    }
    has_pinyin_letter
}

pub fn is_valid_numbered_pinyin(text: &str) -> bool {
    if !is_pinyin_like(text) {
        return false;
    }

    let mut syllable_has_letter = false;
    let mut syllable_has_tone = false;
    let mut saw_tone = false;

    for character in text.chars() {
        if is_ascii_pinyin_letter(character) {
            if syllable_has_tone {
                syllable_has_tone = false;
            }
            syllable_has_letter = true;
            continue;
        }
        if is_marked_pinyin_letter(character) {
            return false;
        }
        if is_pinyin_tone_digit(character) {
            if !syllable_has_letter || syllable_has_tone {
                return false;
            }
            syllable_has_tone = true;
            saw_tone = true;
            continue;
        }
        if character.is_whitespace() || matches!(character, '\'' | '-' | '’') {
            if syllable_has_letter && !syllable_has_tone {
                return false;
            }
            syllable_has_letter = false;
            syllable_has_tone = false;
            continue;
        }
        return false;
    }

    saw_tone && (!syllable_has_letter || syllable_has_tone)
}

pub fn numbered_pinyin_to_tone_marks(text: &str) -> Option<String> {
    if !is_valid_numbered_pinyin(text) {
        return None;
    }

    let mut result = String::new();
    let mut syllable = String::new();
    for character in text.chars() {
        if is_ascii_pinyin_letter(character) {
            syllable.push(character);
            continue;
        }
        if is_pinyin_tone_digit(character) {
            push_marked_syllable(&mut result, &syllable, character);
            syllable.clear();
            continue;
        }
        if !syllable.is_empty() {
            result.push_str(&syllable);
            syllable.clear();
        }
        result.push(character);
    }
    if !syllable.is_empty() {
        result.push_str(&syllable);
    }
    Some(result)
}

fn push_marked_syllable(output: &mut String, syllable: &str, tone_digit: char) {
    let tone = tone_digit.to_digit(10).unwrap_or(5) as usize;
    if tone == 5 {
        for character in syllable.chars() {
            output.push(normalize_unmarked_pinyin_letter(character));
        }
        return;
    }

    let mut chars = syllable.chars().collect::<Vec<_>>();
    let Some(index) = tone_mark_index(&chars) else {
        output.push_str(syllable);
        return;
    };

    chars[index] = tone_marked_vowel(chars[index], tone).unwrap_or(chars[index]);
    for character in chars {
        output.push(normalize_unmarked_pinyin_letter(character));
    }
}

fn tone_mark_index(chars: &[char]) -> Option<usize> {
    for target in ['a', 'A'] {
        if let Some(index) = chars.iter().position(|character| *character == target) {
            return Some(index);
        }
    }
    for target in ['e', 'E'] {
        if let Some(index) = chars.iter().position(|character| *character == target) {
            return Some(index);
        }
    }
    for pair in [['o', 'u'], ['O', 'U']] {
        if let Some(index) = chars
            .windows(2)
            .position(|window| window[0] == pair[0] && window[1] == pair[1])
        {
            return Some(index);
        }
    }

    chars
        .iter()
        .rposition(|character| is_unmarked_vowel(*character))
}

fn tone_marked_vowel(vowel: char, tone: usize) -> Option<char> {
    match (vowel, tone) {
        ('a', 1) => Some('ā'),
        ('a', 2) => Some('á'),
        ('a', 3) => Some('ǎ'),
        ('a', 4) => Some('à'),
        ('e', 1) => Some('ē'),
        ('e', 2) => Some('é'),
        ('e', 3) => Some('ě'),
        ('e', 4) => Some('è'),
        ('i', 1) => Some('ī'),
        ('i', 2) => Some('í'),
        ('i', 3) => Some('ǐ'),
        ('i', 4) => Some('ì'),
        ('o', 1) => Some('ō'),
        ('o', 2) => Some('ó'),
        ('o', 3) => Some('ǒ'),
        ('o', 4) => Some('ò'),
        ('u', 1) => Some('ū'),
        ('u', 2) => Some('ú'),
        ('u', 3) => Some('ǔ'),
        ('u', 4) => Some('ù'),
        ('ü' | 'v', 1) => Some('ǖ'),
        ('ü' | 'v', 2) => Some('ǘ'),
        ('ü' | 'v', 3) => Some('ǚ'),
        ('ü' | 'v', 4) => Some('ǜ'),
        ('A', 1) => Some('Ā'),
        ('A', 2) => Some('Á'),
        ('A', 3) => Some('Ǎ'),
        ('A', 4) => Some('À'),
        ('E', 1) => Some('Ē'),
        ('E', 2) => Some('É'),
        ('E', 3) => Some('Ě'),
        ('E', 4) => Some('È'),
        ('I', 1) => Some('Ī'),
        ('I', 2) => Some('Í'),
        ('I', 3) => Some('Ǐ'),
        ('I', 4) => Some('Ì'),
        ('O', 1) => Some('Ō'),
        ('O', 2) => Some('Ó'),
        ('O', 3) => Some('Ǒ'),
        ('O', 4) => Some('Ò'),
        ('U', 1) => Some('Ū'),
        ('U', 2) => Some('Ú'),
        ('U', 3) => Some('Ǔ'),
        ('U', 4) => Some('Ù'),
        ('Ü' | 'V', 1) => Some('Ǖ'),
        ('Ü' | 'V', 2) => Some('Ǘ'),
        ('Ü' | 'V', 3) => Some('Ǚ'),
        ('Ü' | 'V', 4) => Some('Ǜ'),
        _ => None,
    }
}

fn is_ascii_pinyin_letter(character: char) -> bool {
    character.is_ascii_alphabetic() || matches!(character, 'ü' | 'Ü')
}

fn is_pinyin_tone_digit(character: char) -> bool {
    matches!(character, '1'..='5')
}

fn is_unmarked_vowel(character: char) -> bool {
    matches!(
        character,
        'a' | 'e' | 'i' | 'o' | 'u' | 'ü' | 'v' | 'A' | 'E' | 'I' | 'O' | 'U' | 'Ü' | 'V'
    )
}

fn normalize_unmarked_pinyin_letter(character: char) -> char {
    match character {
        'v' => 'ü',
        'V' => 'Ü',
        _ => character,
    }
}

fn is_marked_pinyin_letter(character: char) -> bool {
    matches!(
        character,
        'ā' | 'á'
            | 'ǎ'
            | 'à'
            | 'ē'
            | 'é'
            | 'ě'
            | 'è'
            | 'ī'
            | 'í'
            | 'ǐ'
            | 'ì'
            | 'ō'
            | 'ó'
            | 'ǒ'
            | 'ò'
            | 'ū'
            | 'ú'
            | 'ǔ'
            | 'ù'
            | 'ǖ'
            | 'ǘ'
            | 'ǚ'
            | 'ǜ'
            | 'Ā'
            | 'Á'
            | 'Ǎ'
            | 'À'
            | 'Ē'
            | 'É'
            | 'Ě'
            | 'È'
            | 'Ī'
            | 'Í'
            | 'Ǐ'
            | 'Ì'
            | 'Ō'
            | 'Ó'
            | 'Ǒ'
            | 'Ò'
            | 'Ū'
            | 'Ú'
            | 'Ǔ'
            | 'Ù'
            | 'Ǖ'
            | 'Ǘ'
            | 'Ǚ'
            | 'Ǜ'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbered_pinyin_converts_tones_to_marks() {
        assert_eq!(
            numbered_pinyin_to_tone_marks("you3").as_deref(),
            Some("yǒu")
        );
        assert_eq!(
            numbered_pinyin_to_tone_marks("chun1xiao3").as_deref(),
            Some("chūnxiǎo")
        );
        assert_eq!(
            numbered_pinyin_to_tone_marks("duo1shao3").as_deref(),
            Some("duōshǎo")
        );
        assert_eq!(
            numbered_pinyin_to_tone_marks("lve4").as_deref(),
            Some("lüè")
        );
        assert_eq!(numbered_pinyin_to_tone_marks("ma5").as_deref(), Some("ma"));
        assert_eq!(
            numbered_pinyin_to_tone_marks("lv5").as_deref(),
            Some("l\u{00fc}")
        );
    }

    #[test]
    fn numbered_pinyin_requires_digit_tones() {
        assert!(is_valid_numbered_pinyin("you3"));
        assert!(is_valid_numbered_pinyin("xi1'an1"));
        assert!(!is_valid_numbered_pinyin("you"));
        assert!(!is_valid_numbered_pinyin("yǒu"));
        assert!(!is_valid_numbered_pinyin("you6"));
        assert!(!is_valid_numbered_pinyin("you33"));
    }
}
