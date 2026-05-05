---
id: ISS-20260505T000000Z-TP-CORE-002-TYPING-MODEL-MOVE-LOGGING
title: "typing inputがmodel値渡しと入力logに依存している"
area: core
status: open
resolved: false
priority: P1
type: performance
created: 2026-05-05
updated: 2026-05-05
target: "src/typing.rs, src/app.rs"
legacy_id: TP-CORE-002
source: "doc/fullreview20260505/core/typing-model.md"
---
# TP-CORE-002: typing inputがmodel値渡しと入力logに依存している

## 概要

`key_input` は `TypingModel` を値で受け取り、頻繁な typing update で move / clone が発生しやすい。さらに key input の log が broad cfg で stdout / console に出る。

## 影響

低性能 target で無駄な copy が効きやすい。入力文字の log は性能と privacy の両面で default behavior に向かない。

## 修正方針

- `fn key_input(model: &mut TypingModel, ...) -> TypingResult` へ寄せる。
- debug logging は optional feature にする。
- release / default build では入力文字を出さない。

## 検証

- typing unit test
- default build で入力 log が出ないこと。
