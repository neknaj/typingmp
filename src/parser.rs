// src/parser.rs

#[cfg(feature = "uefi")]
extern crate alloc;

#[cfg(feature = "uefi")]
use alloc::{string::{String, ToString}, vec::Vec};
#[cfg(not(feature = "uefi"))]
use std::string::{String, ToString};
#[cfg(not(feature = "uefi"))]
use std::vec::Vec;

use crate::model::{Content, Line, Segment, Word};

// --- New Parser Implementation ---

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Segment(Segment),
    Hyphen,
    Separator, // '/'
    Space,
}

// Stage 1: Tokenize the input line
fn tokenize_line(line: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let mut pos = 0;
    let mut plain_text = String::new();

    let flush_plain = |plain: &mut String, tokens: &mut Vec<Token>| {
        if !plain.is_empty() {
            tokens.push(Token::Segment(Segment::Plain { text: std::mem::take(plain) }));
        }
    };

    while pos < chars.len() {
        match chars[pos] {
            '\\' => { // Escape character
                pos += 1;
                if pos < chars.len() {
                    plain_text.push(chars[pos]);
                }
            }
            '(' => { // Start of an annotated segment
                flush_plain(&mut plain_text, &mut tokens);
                let (segment, new_pos) = parse_annotated(&chars, pos);
                tokens.push(Token::Segment(segment));
                pos = new_pos;
                continue; // parse_annotated updates pos, so skip increment at the end
            }
            '-' => { // Hyphen (potential connector)
                flush_plain(&mut plain_text, &mut tokens);
                tokens.push(Token::Hyphen);
            }
            '/' => { // Word separator
                flush_plain(&mut plain_text, &mut tokens);
                tokens.push(Token::Separator);
            }
            ' ' => { // Space (also a word separator)
                flush_plain(&mut plain_text, &mut tokens);
                tokens.push(Token::Space);
            }
            _ => { // Plain text character
                plain_text.push(chars[pos]);
            }
        }
        pos += 1;
    }
    flush_plain(&mut plain_text, &mut tokens);

    tokens
}

fn parse_annotated(chars: &Vec<char>, start: usize) -> (Segment, usize) {
    let mut pos = start + 1; // Skip '('
    let mut base = String::new();
    while pos < chars.len() {
        if chars[pos] == '\\' {
            pos += 1;
            if pos < chars.len() {
                base.push(chars[pos]);
            }
        } else if chars[pos] == '/' || chars[pos] == ')' {
            break;
        } else {
            base.push(chars[pos]);
        }
        pos += 1;
    }
    if pos < chars.len() && chars[pos] == '/' {
        pos += 1;
    }
    let mut reading = String::new();
    while pos < chars.len() {
        if chars[pos] == '\\' {
            pos += 1;
            if pos < chars.len() {
                reading.push(chars[pos]);
            }
        } else if chars[pos] == ')' {
            break;
        } else {
            reading.push(chars[pos]);
        }
        pos += 1;
    }
    if pos < chars.len() && chars[pos] == ')' {
        pos += 1;
    }
    (Segment::Annotated { base, reading }, pos)
}


// Stage 2: Group tokens into a vector of Words
fn group_tokens_into_words(tokens: Vec<Token>) -> Vec<Word> {
    let mut words = Vec::new();
    let mut current_segments = Vec::new();

    let finalize_current_word = |segments: &mut Vec<Segment>, words: &mut Vec<Word>| {
        if !segments.is_empty() {
            words.push(Word { segments: std::mem::take(segments) });
        }
    };

    let token_iter = tokens.iter().peekable();
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
                    current_segments.push(Segment::Plain { text: "-".to_string() }); // ハイフン自体をセグメントに
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
                words.push(Word { segments: vec![Segment::Plain { text: " ".to_string() }] });
                last_token_was_connector = false;
            }
        }
    }
    // ループ終了後、残っているセグメントがあれば最後の単語として確定
    finalize_current_word(&mut current_segments, &mut words);

    words
}


// Main parser function called by the application
pub fn parse_problem(input: &str) -> Content {
    let mut lines_iter = input.lines();
    
    // Parse the title line
    let title_line_str = lines_iter.next().unwrap_or("");
    let title = if title_line_str.starts_with("#title") {
        let content = title_line_str.trim_start_matches("#title").trim();
        // For the title, we treat the entire line as a single word.
        let tokens = tokenize_line(content);
        let mut segments = Vec::new();
        for token in tokens {
             match token {
                 Token::Segment(s) => segments.push(s),
                 // Treat connectors and separators as plain text in the title.
                 Token::Hyphen => segments.push(Segment::Plain { text: "-".to_string() }),
                 Token::Separator => segments.push(Segment::Plain { text: "/".to_string() }),
                 Token::Space => segments.push(Segment::Plain { text: " ".to_string() }),
             }
        }
        Line { words: vec![Word { segments }] }
    } else {
        Line { words: Vec::new() }
    };

    // Parse the remaining content lines
    let mut lines = Vec::new();
    for line_str in lines_iter {
        if line_str.trim().is_empty() {
            continue;
        }
        let tokens = tokenize_line(line_str);
        let words = group_tokens_into_words(tokens);
        lines.push(Line { words });
    }
    
    Content { title, lines }
}


#[cfg(test)]
mod tests {
    use super::*; // 親モジュールの要素（パーサー関数など）をインポート
    use crate::model::{Segment, Word}; // テストで使うモデルをインポート

    // テスト用のヘルパー関数
    // 行文字列を受け取り、解析されたWordのベクタを返す
    fn parse_line_to_words(line: &str) -> Vec<Word> {
        let tokens = tokenize_line(line);
        // デバッグ用にトークン列を出力
        println!("Testing line: '{}'", line);
        println!("Tokens: {:?}", tokens);
        let words = group_tokens_into_words(tokens);
        println!("Resulting words: {:?}\n", words);
        words
    }

    #[test]
    fn test_simple_separation() {
        let line = "(私/わたし)が(彼/かれ)を";
        let expected = vec![
            Word { segments: vec![Segment::Annotated { base: "私".to_string(), reading: "わたし".to_string() }] },
            Word { segments: vec![Segment::Plain { text: "が".to_string() }] },
            Word { segments: vec![Segment::Annotated { base: "彼".to_string(), reading: "かれ".to_string() }] },
            Word { segments: vec![Segment::Plain { text: "を".to_string() }] },
        ];
        assert_eq!(parse_line_to_words(line), expected);
    }

    #[test]
    fn test_okurigana_connection() {
        let line = "(走/はし)-る";
        let expected = vec![
            Word { segments: vec![
                Segment::Annotated { base: "走".to_string(), reading: "はし".to_string() },
                Segment::Plain { text: "る".to_string() },
            ] },
        ];
        assert_eq!(parse_line_to_words(line), expected);
    }

    #[test]
    fn test_multiple_connections() {
        let line = "plain-(annotated/a)-plain";
        let expected = vec![
            Word { segments: vec![
                Segment::Plain { text: "plain".to_string() },
                Segment::Annotated { base: "annotated".to_string(), reading: "a".to_string() },
                Segment::Plain { text: "plain".to_string() },
            ] },
        ];
        assert_eq!(parse_line_to_words(line), expected);
    }
    
    #[test]
    fn test_space_as_word() {
        let line = "plain1 plain2";
        let expected = vec![
            Word { segments: vec![Segment::Plain { text: "plain1".to_string() }] },
            Word { segments: vec![Segment::Plain { text: " ".to_string() }] },
            Word { segments: vec![Segment::Plain { text: "plain2".to_string() }] },
        ];
        assert_eq!(parse_line_to_words(line), expected);
    }

    #[test]
    fn test_separator_as_delimiter() {
        let line = "plain1/plain2";
        let expected = vec![
            Word { segments: vec![Segment::Plain { text: "plain1".to_string() }] },
            Word { segments: vec![Segment::Plain { text: "plain2".to_string() }] },
        ];
        assert_eq!(parse_line_to_words(line), expected);
    }

    #[test]
    fn test_escape_parentheses() {
        let line = "\\(plain\\)";
        let expected = vec![
            Word { segments: vec![Segment::Plain { text: "(plain)".to_string() }] },
        ];
        assert_eq!(parse_line_to_words(line), expected);
    }

    #[test]
    fn test_escape_hyphen() {
        // エスケープされたハイフンは接続子ではなく、前のセグメントの一部になる
        // その後の `(annotated)` は接続子がないため、新しい単語になる
        let line = "plain\\-(annotated/a)";
        let expected = vec![
            Word { segments: vec![Segment::Plain { text: "plain-".to_string() }] },
            Word { segments: vec![Segment::Annotated { base: "annotated".to_string(), reading: "a".to_string() }] },
        ];
        assert_eq!(parse_line_to_words(line), expected);
    }

    #[test]
    fn test_escape_inside_annotated() {
        let line = "(base\\/part/read\\)ing)";
        let expected = vec![
            Word { segments: vec![Segment::Annotated { base: "base/part".to_string(), reading: "read)ing".to_string() }] },
        ];
        assert_eq!(parse_line_to_words(line), expected);
    }

    #[test]
    fn test_unconnected_hyphen() {
        // 前後にセグメントがないハイフンは、それ自体が単語になる
        let line = "word1/-/(word2/w2)";
        let expected = vec![
            Word { segments: vec![Segment::Plain { text: "word1".to_string() }] },
            Word { segments: vec![Segment::Plain { text: "-".to_string() }] },
            Word { segments: vec![Segment::Annotated { base: "word2".to_string(), reading: "w2".to_string() }] },
        ];
        assert_eq!(parse_line_to_words(line), expected);
    }

    #[test]
    fn test_trailing_hyphen() {
        // 末尾のハイフンは、前のセグメントに接続する相手がいないため、
        // 直前の単語に追加されるのではなく、独立した単語になる
        let line = "(word/w)-";
        let expected = vec![
             Word { segments: vec![Segment::Annotated { base: "word".to_string(), reading: "w".to_string() }] },
             Word { segments: vec![Segment::Plain { text: "-".to_string() }] },
        ];
        assert_eq!(parse_line_to_words(line), expected);
    }

    #[test]
    fn test_user_provided_complex_example() {
        let line = "(私/わたし)が(走/はし)-り-(出/だ)-す/と/、";
        let expected = vec![
            Word { segments: vec![Segment::Annotated { base: "私".to_string(), reading: "わたし".to_string() }] },
            Word { segments: vec![Segment::Plain { text: "が".to_string() }] },
            Word { segments: vec![
                Segment::Annotated { base: "走".to_string(), reading: "はし".to_string() },
                Segment::Plain { text: "り".to_string() },
                Segment::Annotated { base: "出".to_string(), reading: "だ".to_string() },
                Segment::Plain { text: "す".to_string() },
            ]},
            Word { segments: vec![Segment::Plain { text: "と".to_string() }] },
            Word { segments: vec![Segment::Plain { text: "、".to_string() }] },
        ];
        assert_eq!(parse_line_to_words(line), expected);
    }
}