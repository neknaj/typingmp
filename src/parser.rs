// src/parser.rs

extern crate alloc;

use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use core::{fmt, mem};

use crate::model::{Content, Line, Segment, Word};
use crate::pinyin;

// --- パーサー実装 ---

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Segment(Segment),
    Hyphen,
    Separator, // '/'
    Space,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseDiagnostics {
    errors: Vec<ParseDiagnostic>,
}

impl ParseDiagnostics {
    fn new() -> Self {
        Self { errors: Vec::new() }
    }

    fn push(&mut self, line: usize, column: usize, kind: ParseDiagnosticKind) {
        self.errors.push(ParseDiagnostic { line, column, kind });
    }

    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn len(&self) -> usize {
        self.errors.len()
    }

    pub fn first(&self) -> Option<&ParseDiagnostic> {
        self.errors.first()
    }

    pub fn errors(&self) -> &[ParseDiagnostic] {
        &self.errors
    }
}

impl fmt::Display for ParseDiagnostics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Some(first) = self.first() else {
            return write!(f, "no parser diagnostics");
        };

        write!(f, "{first}")?;
        if self.len() > 1 {
            write!(f, " (+{} more)", self.len() - 1)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseDiagnostic {
    pub line: usize,
    pub column: usize,
    pub kind: ParseDiagnosticKind,
}

impl fmt::Display for ParseDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "line {}, column {}: {}",
            self.line, self.column, self.kind
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseDiagnosticKind {
    EmptyRubyBase,
    EmptyRubyReading,
    MissingRubySeparator,
    UnclosedRuby,
    EmptyAnnotationInner,
    EmptyAnnotation,
    MissingAnnotationSeparator,
    UnclosedAnnotation,
    NestedSyntax,
    UnexpectedClosingBracket,
    ChineseRubyMustUseNumberedPinyin,
}

impl fmt::Display for ParseDiagnosticKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyRubyBase => write!(f, "ruby base is empty"),
            Self::EmptyRubyReading => write!(f, "ruby reading is empty"),
            Self::MissingRubySeparator => write!(f, "ruby segment is missing '/' separator"),
            Self::UnclosedRuby => write!(f, "ruby segment is missing closing ']'"),
            Self::EmptyAnnotationInner => write!(f, "annotation inner text is empty"),
            Self::EmptyAnnotation => write!(f, "annotation text is empty"),
            Self::MissingAnnotationSeparator => {
                write!(f, "annotation segment is missing '/' separator")
            }
            Self::UnclosedAnnotation => write!(f, "annotation segment is missing closing '}}'"),
            Self::NestedSyntax => write!(f, "nested syntax is not supported"),
            Self::UnexpectedClosingBracket => write!(f, "unexpected closing bracket"),
            Self::ChineseRubyMustUseNumberedPinyin => {
                write!(f, "Chinese pinyin ruby must use numbered tones")
            }
        }
    }
}

// Stage 1: 入力行をトークン列に変換する
fn tokenize_line(line: &str, line_number: usize, diagnostics: &mut ParseDiagnostics) -> Vec<Token> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let mut pos = 0;
    let mut plain_text = String::new();

    let flush_plain = |plain: &mut String, tokens: &mut Vec<Token>| {
        if !plain.is_empty() {
            tokens.push(Token::Segment(Segment::Plain {
                text: mem::take(plain),
            }));
        }
    };

    while pos < chars.len() {
        match chars[pos] {
            '\\' => {
                // エスケープ文字
                pos += 1;
                if pos < chars.len() {
                    plain_text.push(chars[pos]);
                }
            }
            '[' => {
                // ruby記法の開始 [base/reading]
                flush_plain(&mut plain_text, &mut tokens);
                let (segment, new_pos) = parse_ruby(&chars, pos, line_number, diagnostics);
                tokens.push(Token::Segment(segment));
                pos = new_pos;
                continue;
            }
            '{' => {
                // anno記法の開始 {inner/annotation}
                flush_plain(&mut plain_text, &mut tokens);
                let (segment, new_pos) = parse_anno(&chars, pos, line_number, diagnostics);
                tokens.push(Token::Segment(segment));
                pos = new_pos;
                continue;
            }
            ']' | '}' => {
                diagnostics.push(
                    line_number,
                    pos + 1,
                    ParseDiagnosticKind::UnexpectedClosingBracket,
                );
                plain_text.push(chars[pos]);
            }
            '-' => {
                // ハイフン（連結子の候補）
                flush_plain(&mut plain_text, &mut tokens);
                tokens.push(Token::Hyphen);
            }
            '/' => {
                // 単語区切り
                flush_plain(&mut plain_text, &mut tokens);
                tokens.push(Token::Separator);
            }
            ' ' => {
                // スペース（単語区切り）
                flush_plain(&mut plain_text, &mut tokens);
                tokens.push(Token::Space);
            }
            _ => {
                // プレーンテキスト文字
                plain_text.push(chars[pos]);
            }
        }
        pos += 1;
    }
    flush_plain(&mut plain_text, &mut tokens);

    tokens
}

// ruby記法 [base/reading] をパースする。pos は '[' の位置を指す
fn parse_ruby(
    chars: &[char],
    start: usize,
    line_number: usize,
    diagnostics: &mut ParseDiagnostics,
) -> (Segment, usize) {
    let mut pos = start + 1; // '[' をスキップ
    let mut base = String::new();
    let mut has_separator = false;
    while pos < chars.len() {
        if chars[pos] == '\\' {
            pos += 1;
            if pos < chars.len() {
                base.push(chars[pos]);
            }
        } else if chars[pos] == '/' {
            has_separator = true;
            break;
        } else if chars[pos] == ']' {
            break;
        } else if matches!(chars[pos], '[' | '{') {
            diagnostics.push(line_number, pos + 1, ParseDiagnosticKind::NestedSyntax);
            base.push(chars[pos]);
        } else {
            base.push(chars[pos]);
        }
        pos += 1;
    }

    if base.is_empty() {
        diagnostics.push(line_number, start + 1, ParseDiagnosticKind::EmptyRubyBase);
    }

    if pos < chars.len() && chars[pos] == '/' {
        pos += 1;
    } else {
        diagnostics.push(
            line_number,
            pos.saturating_add(1),
            ParseDiagnosticKind::MissingRubySeparator,
        );
    }

    let mut reading = String::new();
    if has_separator {
        while pos < chars.len() {
            if chars[pos] == '\\' {
                pos += 1;
                if pos < chars.len() {
                    reading.push(chars[pos]);
                }
            } else if chars[pos] == ']' {
                break;
            } else if matches!(chars[pos], '[' | '{') {
                diagnostics.push(line_number, pos + 1, ParseDiagnosticKind::NestedSyntax);
                reading.push(chars[pos]);
            } else {
                reading.push(chars[pos]);
            }
            pos += 1;
        }
    }

    if reading.is_empty() {
        diagnostics.push(
            line_number,
            pos.saturating_add(1),
            ParseDiagnosticKind::EmptyRubyReading,
        );
    } else if should_validate_numbered_pinyin(&base, &reading)
        && !pinyin::is_valid_numbered_pinyin(&reading)
    {
        diagnostics.push(
            line_number,
            start + base.chars().count() + 2,
            ParseDiagnosticKind::ChineseRubyMustUseNumberedPinyin,
        );
    }

    if pos < chars.len() && chars[pos] == ']' {
        pos += 1;
    } else {
        diagnostics.push(line_number, start + 1, ParseDiagnosticKind::UnclosedRuby);
    }
    (Segment::Annotated { base, reading }, pos)
}

fn should_validate_numbered_pinyin(base: &str, reading: &str) -> bool {
    contains_cjk_ideograph(base)
        && contains_pinyin_letter(reading)
        && !contains_kana(reading)
        && !contains_bopomofo(reading)
}

fn contains_pinyin_letter(text: &str) -> bool {
    text.chars().any(|character| {
        character.is_ascii_alphabetic()
            || character == 'ü'
            || character == 'Ü'
            || matches!(character as u32, 0x00C0..=0x024F)
    })
}

fn contains_cjk_ideograph(text: &str) -> bool {
    text.chars().any(|character| {
        matches!(
            character as u32,
            0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0x20000..=0x2FA1F
        )
    })
}

fn contains_kana(text: &str) -> bool {
    text.chars().any(|character| {
        matches!(
            character as u32,
            0x3040..=0x309F | 0x30A0..=0x30FF | 0x31F0..=0x31FF | 0xFF66..=0xFF9D
        )
    })
}

fn contains_bopomofo(text: &str) -> bool {
    text.chars()
        .any(|character| matches!(character as u32, 0x3100..=0x312F | 0x31A0..=0x31BF))
}

// anno記法 {inner/annotation} をパースする。pos は '{' の位置を指す
// inner は ruby ([base/reading]) とプレーンテキストの混合を許容する
// '[...]' の内部の '/' は anno の区切りとして扱わない
fn parse_anno(
    chars: &[char],
    start: usize,
    line_number: usize,
    diagnostics: &mut ParseDiagnostics,
) -> (Segment, usize) {
    let mut pos = start + 1; // '{' をスキップ
    let mut inner: Vec<Segment> = Vec::new();
    let mut plain_buf = String::new();
    let mut has_separator = false;

    let flush_plain_buf = |buf: &mut String, inner: &mut Vec<Segment>| {
        if !buf.is_empty() {
            inner.push(Segment::Plain {
                text: mem::take(buf),
            });
        }
    };

    // inner 部分（'/' または '}' まで）を解析
    while pos < chars.len() {
        match chars[pos] {
            '\\' => {
                pos += 1;
                if pos < chars.len() {
                    plain_buf.push(chars[pos]);
                }
            }
            '[' => {
                // ruby セグメントとしてパース
                flush_plain_buf(&mut plain_buf, &mut inner);
                let (seg, new_pos) = parse_ruby(chars, pos, line_number, diagnostics);
                inner.push(seg);
                pos = new_pos;
                continue;
            }
            '/' => {
                has_separator = true;
                break;
            }
            '}' => {
                // inner の終端
                break;
            }
            '{' => {
                diagnostics.push(line_number, pos + 1, ParseDiagnosticKind::NestedSyntax);
                plain_buf.push(chars[pos]);
            }
            c => {
                plain_buf.push(c);
            }
        }
        pos += 1;
    }
    flush_plain_buf(&mut plain_buf, &mut inner);

    if inner.is_empty() {
        diagnostics.push(
            line_number,
            start + 1,
            ParseDiagnosticKind::EmptyAnnotationInner,
        );
    }

    // '/' をスキップして annotation 部分へ
    if pos < chars.len() && chars[pos] == '/' {
        pos += 1;
    } else {
        diagnostics.push(
            line_number,
            pos.saturating_add(1),
            ParseDiagnosticKind::MissingAnnotationSeparator,
        );
    }

    let mut annotation = String::new();
    if has_separator {
        while pos < chars.len() {
            if chars[pos] == '\\' {
                pos += 1;
                if pos < chars.len() {
                    annotation.push(chars[pos]);
                }
            } else if chars[pos] == '}' {
                break;
            } else if matches!(chars[pos], '[' | '{') {
                diagnostics.push(line_number, pos + 1, ParseDiagnosticKind::NestedSyntax);
                annotation.push(chars[pos]);
            } else {
                annotation.push(chars[pos]);
            }
            pos += 1;
        }
    }

    if annotation.is_empty() {
        diagnostics.push(
            line_number,
            pos.saturating_add(1),
            ParseDiagnosticKind::EmptyAnnotation,
        );
    }

    if pos < chars.len() && chars[pos] == '}' {
        pos += 1;
    } else {
        diagnostics.push(
            line_number,
            start + 1,
            ParseDiagnosticKind::UnclosedAnnotation,
        );
    }

    (Segment::Anno { inner, annotation }, pos)
}

// Stage 2: トークン列を単語のベクタにグループ化する
fn group_tokens_into_words(tokens: Vec<Token>) -> Vec<Word> {
    let mut words = Vec::new();
    let mut current_segments = Vec::new();

    let finalize_current_word = |segments: &mut Vec<Segment>, words: &mut Vec<Word>| {
        if !segments.is_empty() {
            words.push(Word {
                segments: mem::take(segments),
            });
        }
    };

    let mut last_token_was_connector = false;

    for (i, token) in tokens.iter().enumerate() {
        match token {
            Token::Segment(segment) => {
                // 直前が接続子でなく、かつ現在の単語が既に何かを含んでいれば、新しい単語を開始する
                if !last_token_was_connector && !current_segments.is_empty() {
                    finalize_current_word(&mut current_segments, &mut words);
                }
                current_segments.push(segment.clone());
                last_token_was_connector = false;
            }
            Token::Hyphen => {
                // ハイフンが接続子として機能するかを判定
                let prev_is_segment = !current_segments.is_empty();
                let next_is_segment = if i + 1 < tokens.len() {
                    matches!(&tokens[i + 1], Token::Segment(_))
                } else {
                    false
                };

                if prev_is_segment && next_is_segment {
                    // 接続子であるため、フラグを立てて次のセグメントを待つ
                    last_token_was_connector = true;
                } else {
                    // 接続子ではない（ただの文字）
                    finalize_current_word(&mut current_segments, &mut words); // 直前の単語を確定
                    current_segments.push(Segment::Plain {
                        text: "-".to_string(),
                    }); // ハイフン自体をセグメントに
                    finalize_current_word(&mut current_segments, &mut words); // ハイフンを独立した単語として確定
                    last_token_was_connector = false;
                }
            }
            Token::Separator => {
                finalize_current_word(&mut current_segments, &mut words);
                last_token_was_connector = false;
            }
            Token::Space => {
                finalize_current_word(&mut current_segments, &mut words);
                words.push(Word {
                    segments: vec![Segment::Plain {
                        text: " ".to_string(),
                    }],
                });
                last_token_was_connector = false;
            }
        }
    }
    // ループ終了後、残っているセグメントがあれば最後の単語として確定
    finalize_current_word(&mut current_segments, &mut words);

    words
}

// アプリケーションから呼び出されるメインのパーサー関数
pub fn parse_problem(input: &str) -> Result<Content, ParseDiagnostics> {
    let mut diagnostics = ParseDiagnostics::new();
    let mut lines_iter = input.lines();

    // タイトル行を解析
    let title_line_str = lines_iter.next().unwrap_or("");
    let title = if title_line_str.starts_with("#title") {
        let content = title_line_str.trim_start_matches("#title").trim();
        // タイトル行も本文と同様にトークン化し、単語にグループ化する
        let tokens = tokenize_line(content, 1, &mut diagnostics);
        let words = group_tokens_into_words(tokens);
        Line { words }
    } else {
        Line { words: Vec::new() }
    };

    // 残りの本文行を解析
    let mut lines = Vec::new();
    for (line_offset, line_str) in lines_iter.enumerate() {
        if line_str.trim().is_empty() {
            continue;
        }
        let tokens = tokenize_line(line_str, line_offset + 2, &mut diagnostics);
        let words = group_tokens_into_words(tokens);
        lines.push(Line { words });
    }

    if diagnostics.is_empty() {
        Ok(Content { title, lines })
    } else {
        Err(diagnostics)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Segment, Word};

    // テスト用ヘルパー: 行文字列を受け取り、解析されたWordのベクタを返す
    fn parse_line_to_words(line: &str) -> Vec<Word> {
        let mut diagnostics = ParseDiagnostics::new();
        let tokens = tokenize_line(line, 1, &mut diagnostics);
        assert!(
            diagnostics.is_empty(),
            "valid fixture should not emit diagnostics: {diagnostics:?}"
        );
        println!("Testing line: '{}'", line);
        println!("Tokens: {:?}", tokens);
        let words = group_tokens_into_words(tokens);
        println!("Resulting words: {:?}\n", words);
        words
    }

    fn diagnostic_kinds_for_line(line: &str) -> Vec<ParseDiagnosticKind> {
        let mut diagnostics = ParseDiagnostics::new();
        let tokens = tokenize_line(line, 1, &mut diagnostics);
        let _ = group_tokens_into_words(tokens);
        diagnostics
            .errors()
            .iter()
            .map(|diagnostic| diagnostic.kind.clone())
            .collect()
    }

    #[test]
    fn test_simple_separation() {
        // annotatedとplain、annotated同士がハイフンなしで区切られる最も基本的なケース
        let line = "[秋/あき]の[田/た]の";
        let expected = vec![
            Word {
                segments: vec![Segment::Annotated {
                    base: "秋".to_string(),
                    reading: "あき".to_string(),
                }],
            },
            Word {
                segments: vec![Segment::Plain {
                    text: "の".to_string(),
                }],
            },
            Word {
                segments: vec![Segment::Annotated {
                    base: "田".to_string(),
                    reading: "た".to_string(),
                }],
            },
            Word {
                segments: vec![Segment::Plain {
                    text: "の".to_string(),
                }],
            },
        ];
        assert_eq!(parse_line_to_words(line), expected);
    }

    #[test]
    fn test_okurigana_connection() {
        // annotatedとplainがハイフンで連結され、1つの単語になるケース（送り仮名）
        let line = "[悲/かな]-しき";
        let expected = vec![Word {
            segments: vec![
                Segment::Annotated {
                    base: "悲".to_string(),
                    reading: "かな".to_string(),
                },
                Segment::Plain {
                    text: "しき".to_string(),
                },
            ],
        }];
        assert_eq!(parse_line_to_words(line), expected);
    }

    #[test]
    fn test_multiple_connections() {
        // 複数のセグメント（plain, annotated, plain）がハイフンで連結されるケース
        let line = "ふみ-[分/わ]-け";
        let expected = vec![Word {
            segments: vec![
                Segment::Plain {
                    text: "ふみ".to_string(),
                },
                Segment::Annotated {
                    base: "分".to_string(),
                    reading: "わ".to_string(),
                },
                Segment::Plain {
                    text: "け".to_string(),
                },
            ],
        }];
        assert_eq!(parse_line_to_words(line), expected);
    }

    #[test]
    fn test_space_as_word() {
        // スペースが独立した単語として扱われるケース
        let line = "[春/はる] [夏/なつ]";
        let expected = vec![
            Word {
                segments: vec![Segment::Annotated {
                    base: "春".to_string(),
                    reading: "はる".to_string(),
                }],
            },
            Word {
                segments: vec![Segment::Plain {
                    text: " ".to_string(),
                }],
            },
            Word {
                segments: vec![Segment::Annotated {
                    base: "夏".to_string(),
                    reading: "なつ".to_string(),
                }],
            },
        ];
        assert_eq!(parse_line_to_words(line), expected);
    }

    #[test]
    fn test_separator_as_delimiter() {
        // スラッシュ `/` が単語の区切り文字として機能するケース
        let line = "とま/を/あらみ";
        let expected = vec![
            Word {
                segments: vec![Segment::Plain {
                    text: "とま".to_string(),
                }],
            },
            Word {
                segments: vec![Segment::Plain {
                    text: "を".to_string(),
                }],
            },
            Word {
                segments: vec![Segment::Plain {
                    text: "あらみ".to_string(),
                }],
            },
        ];
        assert_eq!(parse_line_to_words(line), expected);
    }

    #[test]
    fn test_escape_brackets() {
        // バックスラッシュで角括弧をエスケープし、ただの文字として扱うケース
        let line = "\\[ここまで\\]";
        let expected = vec![Word {
            segments: vec![Segment::Plain {
                text: "[ここまで]".to_string(),
            }],
        }];
        assert_eq!(parse_line_to_words(line), expected);
    }

    #[test]
    fn test_escape_hyphen() {
        // バックスラッシュでハイフンをエスケープし、連結子ではなく文字として扱うケース
        let line = "コピー\\-[機/き]";
        let expected = vec![
            Word {
                segments: vec![Segment::Plain {
                    text: "コピー-".to_string(),
                }],
            },
            Word {
                segments: vec![Segment::Annotated {
                    base: "機".to_string(),
                    reading: "き".to_string(),
                }],
            },
        ];
        assert_eq!(parse_line_to_words(line), expected);
    }

    #[test]
    fn test_escape_inside_ruby() {
        // ruby内部の特殊文字（スラッシュ）をエスケープするケース
        let line = "[A\\/B/えーぶんのびー]";
        let expected = vec![Word {
            segments: vec![Segment::Annotated {
                base: "A/B".to_string(),
                reading: "えーぶんのびー".to_string(),
            }],
        }];
        assert_eq!(parse_line_to_words(line), expected);
    }

    #[test]
    fn test_unconnected_hyphen() {
        // 前後が区切り文字で、連結の対象にならないハイフンが、それ自体で単語になるケース
        let line = "[東京/とうきょう]/-/[大阪/おおさか]";
        let expected = vec![
            Word {
                segments: vec![Segment::Annotated {
                    base: "東京".to_string(),
                    reading: "とうきょう".to_string(),
                }],
            },
            Word {
                segments: vec![Segment::Plain {
                    text: "-".to_string(),
                }],
            },
            Word {
                segments: vec![Segment::Annotated {
                    base: "大阪".to_string(),
                    reading: "おおさか".to_string(),
                }],
            },
        ];
        assert_eq!(parse_line_to_words(line), expected);
    }

    #[test]
    fn test_trailing_hyphen() {
        // 行末など、後ろに連結する相手がいないハイフンが、独立した単語になるケース
        let line = "[長/なが]-";
        let expected = vec![
            Word {
                segments: vec![Segment::Annotated {
                    base: "長".to_string(),
                    reading: "なが".to_string(),
                }],
            },
            Word {
                segments: vec![Segment::Plain {
                    text: "-".to_string(),
                }],
            },
        ];
        assert_eq!(parse_line_to_words(line), expected);
    }

    #[test]
    fn test_user_provided_complex_example() {
        // 複数の連結と区切りが混在する実践的なケース
        let line = "[思/おも]-ひ-[絶/た]-え/なむ";
        let expected = vec![
            Word {
                segments: vec![
                    Segment::Annotated {
                        base: "思".to_string(),
                        reading: "おも".to_string(),
                    },
                    Segment::Plain {
                        text: "ひ".to_string(),
                    },
                    Segment::Annotated {
                        base: "絶".to_string(),
                        reading: "た".to_string(),
                    },
                    Segment::Plain {
                        text: "え".to_string(),
                    },
                ],
            },
            Word {
                segments: vec![Segment::Plain {
                    text: "なむ".to_string(),
                }],
            },
        ];
        assert_eq!(parse_line_to_words(line), expected);
    }

    #[test]
    fn test_multiple_spaces() {
        // 複数の連続した空白が、それぞれ独立した単語として扱われることを確認
        let line = "[上/うえ]  [下/した]";
        let expected = vec![
            Word {
                segments: vec![Segment::Annotated {
                    base: "上".to_string(),
                    reading: "うえ".to_string(),
                }],
            },
            Word {
                segments: vec![Segment::Plain {
                    text: " ".to_string(),
                }],
            },
            Word {
                segments: vec![Segment::Plain {
                    text: " ".to_string(),
                }],
            },
            Word {
                segments: vec![Segment::Annotated {
                    base: "下".to_string(),
                    reading: "した".to_string(),
                }],
            },
        ];
        assert_eq!(parse_line_to_words(line), expected);
    }

    #[test]
    fn test_leading_and_trailing_whitespace() {
        // 行頭と行末の空白が正しく単語として認識されることを確認
        let line = "  [開始/かいし]  ";
        let expected = vec![
            Word {
                segments: vec![Segment::Plain {
                    text: " ".to_string(),
                }],
            },
            Word {
                segments: vec![Segment::Plain {
                    text: " ".to_string(),
                }],
            },
            Word {
                segments: vec![Segment::Annotated {
                    base: "開始".to_string(),
                    reading: "かいし".to_string(),
                }],
            },
            Word {
                segments: vec![Segment::Plain {
                    text: " ".to_string(),
                }],
            },
            Word {
                segments: vec![Segment::Plain {
                    text: " ".to_string(),
                }],
            },
        ];
        assert_eq!(parse_line_to_words(line), expected);
    }

    #[test]
    fn test_consecutive_separators() {
        // 区切り文字が連続しても、間に空の単語が生成されないことを確認
        let line = "[一/いち]//[二/に]";
        let expected = vec![
            Word {
                segments: vec![Segment::Annotated {
                    base: "一".to_string(),
                    reading: "いち".to_string(),
                }],
            },
            Word {
                segments: vec![Segment::Annotated {
                    base: "二".to_string(),
                    reading: "に".to_string(),
                }],
            },
        ];
        assert_eq!(parse_line_to_words(line), expected);
    }

    #[test]
    fn test_malformed_ruby_reports_diagnostics() {
        let unclosed = diagnostic_kinds_for_line("[未完了/みかんりょう");
        assert!(unclosed.contains(&ParseDiagnosticKind::UnclosedRuby));

        let missing_separator = diagnostic_kinds_for_line("[ベースのみ]");
        assert!(missing_separator.contains(&ParseDiagnosticKind::MissingRubySeparator));

        let empty = diagnostic_kinds_for_line("[]");
        assert!(empty.contains(&ParseDiagnosticKind::EmptyRubyBase));
        assert!(empty.contains(&ParseDiagnosticKind::EmptyRubyReading));

        let empty_reading = diagnostic_kinds_for_line("[色/]");
        assert!(empty_reading.contains(&ParseDiagnosticKind::EmptyRubyReading));

        let nested = diagnostic_kinds_for_line("[色/{いろ}]");
        assert!(nested.contains(&ParseDiagnosticKind::NestedSyntax));

        let unexpected_closing = diagnostic_kinds_for_line("色]");
        assert!(unexpected_closing.contains(&ParseDiagnosticKind::UnexpectedClosingBracket));
    }

    #[test]
    fn test_chinese_pinyin_ruby_requires_numbered_tones() {
        parse_problem("#title Test\n[\u{6709}/you3]").expect("numbered pinyin should parse");
        parse_problem("#title Test\n[\u{5b57}/\u{3117}\u{02cb}]")
            .expect("bopomofo ruby should parse");

        let marked = parse_problem("#title Test\n[\u{6709}/y\u{01d2}u]")
            .expect_err("marked pinyin should be rejected");
        assert_eq!(
            marked.first().map(|diagnostic| &diagnostic.kind),
            Some(&ParseDiagnosticKind::ChineseRubyMustUseNumberedPinyin)
        );

        let missing = parse_problem("#title Test\n[\u{6709}/you]")
            .expect_err("pinyin without tone number should be rejected");
        assert_eq!(
            missing.first().map(|diagnostic| &diagnostic.kind),
            Some(&ParseDiagnosticKind::ChineseRubyMustUseNumberedPinyin)
        );

        let out_of_range = parse_problem("#title Test\n[\u{6709}/you6]")
            .expect_err("pinyin tone outside 1-5 should be rejected");
        assert_eq!(
            out_of_range.first().map(|diagnostic| &diagnostic.kind),
            Some(&ParseDiagnosticKind::ChineseRubyMustUseNumberedPinyin)
        );
    }

    #[test]
    fn test_parse_problem_returns_diagnostics_with_line_position() {
        let diagnostics = parse_problem("#title Valid\n[未完了/みかんりょう")
            .expect_err("malformed body line should fail");

        assert_eq!(diagnostics.len(), 1);
        let diagnostic = diagnostics.first().expect("one diagnostic");
        assert_eq!(diagnostic.line, 2);
        assert_eq!(diagnostic.column, 1);
        assert_eq!(diagnostic.kind, ParseDiagnosticKind::UnclosedRuby);
    }

    #[test]
    fn test_readme_sample_parses() {
        let sample = "#title [サンプル/さんぷる]問題集\n[吾輩/わがはい]は[猫/ねこ]である。/[名前/なまえ]はまだ[無/な]い。\n[走/はし]-れメロス";

        let content = parse_problem(sample).expect("README sample should parse");

        assert_eq!(content.lines.len(), 2);
        assert_eq!(content.title.words.len(), 2);
    }

    #[test]
    fn test_title_line_parsing() {
        // `#title` 行でも本文と同じルールで区切り文字や接続子が扱われることを確認
        let full_problem = "#title [Rust/ラスト]で-[書/か]-かれた/パーサー\n[本文/ほんぶん]";
        let content = parse_problem(full_problem).expect("valid problem should parse");

        let expected_title_words = vec![
            Word {
                segments: vec![Segment::Annotated {
                    base: "Rust".to_string(),
                    reading: "ラスト".to_string(),
                }],
            },
            Word {
                segments: vec![
                    Segment::Plain {
                        text: "で".to_string(),
                    },
                    Segment::Annotated {
                        base: "書".to_string(),
                        reading: "か".to_string(),
                    },
                    Segment::Plain {
                        text: "かれた".to_string(),
                    },
                ],
            },
            Word {
                segments: vec![Segment::Plain {
                    text: "パーサー".to_string(),
                }],
            },
        ];

        assert_eq!(content.title.words, expected_title_words);

        let expected_body_words = vec![Word {
            segments: vec![Segment::Annotated {
                base: "本文".to_string(),
                reading: "ほんぶん".to_string(),
            }],
        }];
        assert_eq!(content.lines[0].words, expected_body_words);
    }

    #[test]
    fn test_escape_backslash() {
        // バックスラッシュ自体をエスケープするケース
        let line = "C:\\\\Users\\\\[Taro/たろう]";
        let expected = vec![
            Word {
                segments: vec![Segment::Plain {
                    text: "C:\\Users\\".to_string(),
                }],
            },
            Word {
                segments: vec![Segment::Annotated {
                    base: "Taro".to_string(),
                    reading: "たろう".to_string(),
                }],
            },
        ];
        assert_eq!(parse_line_to_words(line), expected);
    }

    // --- anno記法のテスト ---

    #[test]
    fn test_anno_simple() {
        // プレーンテキストを anno で囲む基本ケース
        let line = "{hello/こんにちは}";
        let expected = vec![Word {
            segments: vec![Segment::Anno {
                inner: vec![Segment::Plain {
                    text: "hello".to_string(),
                }],
                annotation: "こんにちは".to_string(),
            }],
        }];
        assert_eq!(parse_line_to_words(line), expected);
    }

    #[test]
    fn test_anno_with_ruby() {
        // anno の inner に ruby を含むケース
        let line = "{[微分/びぶん][係数/けいすう]/derivative}";
        let expected = vec![Word {
            segments: vec![Segment::Anno {
                inner: vec![
                    Segment::Annotated {
                        base: "微分".to_string(),
                        reading: "びぶん".to_string(),
                    },
                    Segment::Annotated {
                        base: "係数".to_string(),
                        reading: "けいすう".to_string(),
                    },
                ],
                annotation: "derivative".to_string(),
            }],
        }];
        assert_eq!(parse_line_to_words(line), expected);
    }

    #[test]
    fn test_anno_with_hyphen_connection() {
        // anno セグメントがハイフンで他セグメントと連結されるケース
        let line = "{[悲/かな]/sad}-しき";
        let expected = vec![Word {
            segments: vec![
                Segment::Anno {
                    inner: vec![Segment::Annotated {
                        base: "悲".to_string(),
                        reading: "かな".to_string(),
                    }],
                    annotation: "sad".to_string(),
                },
                Segment::Plain {
                    text: "しき".to_string(),
                },
            ],
        }];
        assert_eq!(parse_line_to_words(line), expected);
    }

    #[test]
    fn test_anno_separator_outside() {
        // anno の外側の '/' は通常の単語区切りとして機能することを確認
        let line = "{abc/訳}/plain";
        let expected = vec![
            Word {
                segments: vec![Segment::Anno {
                    inner: vec![Segment::Plain {
                        text: "abc".to_string(),
                    }],
                    annotation: "訳".to_string(),
                }],
            },
            Word {
                segments: vec![Segment::Plain {
                    text: "plain".to_string(),
                }],
            },
        ];
        assert_eq!(parse_line_to_words(line), expected);
    }

    #[test]
    fn test_anno_mixed_inner() {
        // anno の inner に ruby とプレーンテキストが混在するケース
        let line = "{[台湾/たいわん]の[首都/しゅと]/Taiwan's capital}";
        let expected = vec![Word {
            segments: vec![Segment::Anno {
                inner: vec![
                    Segment::Annotated {
                        base: "台湾".to_string(),
                        reading: "たいわん".to_string(),
                    },
                    Segment::Plain {
                        text: "の".to_string(),
                    },
                    Segment::Annotated {
                        base: "首都".to_string(),
                        reading: "しゅと".to_string(),
                    },
                ],
                annotation: "Taiwan's capital".to_string(),
            }],
        }];
        assert_eq!(parse_line_to_words(line), expected);
    }
}
