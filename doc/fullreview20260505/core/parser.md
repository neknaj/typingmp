# コアレビュー: parser

対象 commit: `501bb73fe53a32bdbc100303498a9dc438ff85aa`

## 結論

parser は現状の test では通っているが、異常系を diagnostic として返す設計になっていない。問題文の記法は `doc/ntq-format.md` と実装では `[...]` を使う一方、README には `(...)` の説明が残っており、利用者向け仕様がずれている。

## 実装上の問題

`parse_problem` は `Content` を返すため、malformed input を通常 content と区別しにくい。`parse_ruby(chars: &Vec<char>, ...)` は slice で足りるため、Clippy の `ptr_arg` warning が出る。これらは小さい問題に見えるが、custom problem import や web storage と組み合わさると、失敗を user に説明できない。

修正方針:

- `parse_problem` を `Result<Content, ParseDiagnostics>` にする。
- web / GUI / TUI 側で diagnostic を表示できる path を用意する。
- malformed bracket、empty reading、nested syntax、unclosed ruby を fixture 化する。
- `&Vec<T>` は `&[T]` にする。

## docs との不整合

README は問題記法を `(base/reading)` と説明している箇所があるが、実装と `doc/ntq-format.md` は `[base/reading]` である。これは custom problem を書く利用者に直接影響する。

修正方針:

- README を bracket syntax に統一する。
- examples と generated builtin problem の表記を fixture として確認する。
- parser test に README sample を入れる。

## 関連 issue

- `TP-CORE-001`
- `TP-DOC-001`

## 対応状況: 2026-05-06

- `parse_problem` は `Result<Content, ParseDiagnostics>` を返す。
- malformed bracket、empty reading、nested syntax、unclosed ruby は parser test で diagnostic を検証している。
- README の問題記法は `[base/reading]` に統一し、README sample を parser test に入れた。
