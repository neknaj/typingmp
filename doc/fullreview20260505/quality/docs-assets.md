# 品質レビュー: docs と assets

対象 commit: `501bb73fe53a32bdbc100303498a9dc438ff85aa`

## 結論

README の problem syntax、Cargo.toml comments、生成物の管理に整理が必要である。実装 bug とは別に、利用者と修正者が誤った前提で作業するリスクがある。

## README syntax

README は ruby annotation を `(base/reading)` と説明している箇所があるが、実装と `doc/ntq-format.md` は `[base/reading]` である。

修正方針:

- README を `[base/reading]` に統一する。
- README sample を parser test に入れる。
- `doc/ntq-format.md` を正規仕様として明記する。

## mojibake / comment hygiene

`Cargo.toml` のコメントに mojibake している箇所がある。source comment は build には影響しないが、feature の意図を誤読させる。

修正方針:

- source / docs は UTF-8 に統一する。
- コメントの意味が不要なら削除し、必要なら日本語または英語で書き直す。

## generated artifact

repository root に `rust_multibackend_app.efi` があり、`pkg/` や `.playwright-mcp` も生成物として見える。配布 artifact と source artifact の境界が曖昧である。

修正方針:

- 生成物を `.gitignore` へ追加する。
- 配布に必要な生成物は release artifact として管理する。
- `pkg/` を GitHub Pages 用に保持するなら、その理由と更新手順を README に書く。

## 関連 issue

- `TP-DOC-001`
- `TP-DOC-002`
- `TP-DOC-003`
