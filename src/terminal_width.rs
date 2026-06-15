pub(crate) fn text_width(text: &str) -> usize {
    text.chars().map(char_width).sum()
}

pub(crate) fn char_width(character: char) -> usize {
    let code = character as u32;
    if is_zero_width(code) {
        0
    } else if is_wide(code) {
        2
    } else {
        1
    }
}

fn is_zero_width(code: u32) -> bool {
    matches!(
        code,
        0x0000
            | 0x0300..=0x036F
            | 0x0483..=0x0489
            | 0x0591..=0x05BD
            | 0x05BF
            | 0x05C1..=0x05C2
            | 0x05C4..=0x05C5
            | 0x05C7
            | 0x0610..=0x061A
            | 0x064B..=0x065F
            | 0x0670
            | 0x06D6..=0x06DC
            | 0x06DF..=0x06E4
            | 0x06E7..=0x06E8
            | 0x06EA..=0x06ED
            | 0xFE00..=0xFE0F
            | 0xE0100..=0xE01EF
    )
}

fn is_wide(code: u32) -> bool {
    matches!(
        code,
        0x1100..=0x115F
            | 0x231A..=0x231B
            | 0x2329..=0x232A
            | 0x23E9..=0x23EC
            | 0x23F0
            | 0x23F3
            | 0x25FD..=0x25FE
            | 0x2614..=0x2615
            | 0x2648..=0x2653
            | 0x267F
            | 0x2693
            | 0x26A1
            | 0x26AA..=0x26AB
            | 0x26BD..=0x26BE
            | 0x26C4..=0x26C5
            | 0x26CE
            | 0x26D4
            | 0x26EA
            | 0x26F2..=0x26F3
            | 0x26F5
            | 0x26FA
            | 0x26FD
            | 0x2705
            | 0x270A..=0x270B
            | 0x2728
            | 0x274C
            | 0x274E
            | 0x2753..=0x2755
            | 0x2757
            | 0x2795..=0x2797
            | 0x27B0
            | 0x27BF
            | 0x2B1B..=0x2B1C
            | 0x2B50
            | 0x2B55
            | 0x2E80..=0xA4CF
            | 0xAC00..=0xD7A3
            | 0xF900..=0xFAFF
            | 0xFE10..=0xFE19
            | 0xFE30..=0xFE6F
            | 0xFF00..=0xFF60
            | 0xFFE0..=0xFFE6
            | 0x1F300..=0x1FAFF
            | 0x20000..=0x3FFFD
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_uses_single_cells() {
        assert_eq!(text_width("ABC!"), 4);
    }

    #[test]
    fn japanese_and_chinese_use_double_cells() {
        assert_eq!(text_width("日本"), 4);
        assert_eq!(text_width("春晓"), 4);
        assert_eq!(text_width("。"), 2);
    }

    #[test]
    fn combining_marks_do_not_advance() {
        assert_eq!(text_width("e\u{0301}"), 1);
    }
}
