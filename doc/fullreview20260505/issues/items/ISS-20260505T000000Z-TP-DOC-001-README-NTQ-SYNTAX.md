---
id: ISS-20260505T000000Z-TP-DOC-001-README-NTQ-SYNTAX
title: "READMEの問題記法が実装とずれている"
area: docs
status: open
resolved: false
priority: P2
type: docs
created: 2026-05-05
updated: 2026-05-05
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
