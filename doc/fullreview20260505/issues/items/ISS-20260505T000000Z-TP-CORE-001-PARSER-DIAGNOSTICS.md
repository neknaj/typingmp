---
id: ISS-20260505T000000Z-TP-CORE-001-PARSER-DIAGNOSTICS
title: "parserがmalformed inputをdiagnosticとして返せない"
area: core
status: open
resolved: false
priority: P1
type: bug
created: 2026-05-05
updated: 2026-05-05
target: "src/parser.rs, src/model.rs, src/wasm.rs"
legacy_id: TP-CORE-001
source: "doc/fullreview20260505/core/parser.md"
---
# TP-CORE-001: parserがmalformed inputをdiagnosticとして返せない

## 概要

`parse_problem` は `Content` を返すため、括弧の閉じ忘れや不正な ruby 記法を structured diagnostic として扱えない。

## 影響

custom problem import や web storage で不正データを読み込んだ時、利用者へ具体的な失敗理由を示せない。core と UI adapter の境界でも error contract が曖昧になる。

## 修正方針

- `Result<Content, ParseDiagnostics>` を返す。
- malformed bracket、empty reading、nested syntax、unclosed ruby を test fixture 化する。
- UI 側で diagnostic を表示する path を作る。

## 検証

- parser error test
- WASM custom problem import の失敗表示 smoke
