---
id: ISS-20260505T000000Z-TP-TYPE-001-I32-CURSOR-INDICES
title: "typing stateがi32 indexとunchecked castに依存している"
area: core
status: open
resolved: false
priority: P1
type: type-safety
created: 2026-05-05
updated: 2026-05-05
target: "src/model.rs, src/typing.rs, src/app.rs"
legacy_id: TP-TYPE-001
source: "doc/fullreview20260505/core/typing-model.md"
---
# TP-TYPE-001: typing stateがi32 indexとunchecked castに依存している

## 概要

typing state は line / char / candidate の index に `i32` を多用している。Rust の collection index は `usize` であり、負値や境界外を型で表現できていない。

## 影響

cursor transition の bug が compile 時に検出されにくい。platform input や custom problem の異常系と組み合わさると、panic や表示ずれにつながる。

## 修正方針

- `LineIndex`、`CharIndex`、`CandidateIndex` の newtype を導入する。
- user-visible count と internal index を分ける。
- boundary check を constructor / transition method に閉じ込める。

## 検証

- typing cursor transition test
- clippy cast warning の削減
