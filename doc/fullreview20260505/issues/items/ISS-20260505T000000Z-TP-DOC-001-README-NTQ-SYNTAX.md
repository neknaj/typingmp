---
id: ISS-20260505T000000Z-TP-DOC-001-README-NTQ-SYNTAX
title: "READMEの問題記法が実装とずれている"
area: docs
status: verified
resolved: true
priority: P2
type: docs
created: 2026-05-05
updated: 2026-05-06
target: "readme.md, doc/ntq-format.md, src/parser.rs"
legacy_id: TP-DOC-001
source: "doc/fullreview20260505/core/parser.md"
---
# TP-DOC-001: READMEの問題記法が実装とずれている

## 概要

README は ruby annotation を `(base/reading)` と説明している箇所があるが、実装と `doc/ntq-format.md` は `[base/reading]` を使う。

## 影響

custom problem を書く利用者が誤った syntax を使う。parser の仕様と public docs が一致しない。

## 修正方針

- README を `[base/reading]` に統一する。
- README sample を parser test に追加する。
- `doc/ntq-format.md` を正規仕様として参照する。

## 検証

- README sample parse test

## 対応: 2026-05-06

- `readme.md` の ruby annotation 記法を `[base/reading]` に統一した。
- README の sample を parser test に追加し、`doc/ntq-format.md` と同じ bracket syntax で parse できることを固定した。

## 検証: 2026-05-06

- `cargo test --no-default-features`
