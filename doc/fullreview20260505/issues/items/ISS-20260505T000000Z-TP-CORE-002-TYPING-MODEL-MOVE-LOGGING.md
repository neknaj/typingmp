---
id: ISS-20260505T000000Z-TP-CORE-002-TYPING-MODEL-MOVE-LOGGING
title: "typing inputがmodel値渡しと入力logに依存している"
area: core
status: verified
resolved: true
priority: P1
type: performance
created: 2026-05-05
updated: 2026-05-06
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

## 進捗: T04

no_std core boundary の先行修正として、`typing.rs` から stdout / UEFI console / web console / WASM debug logger への直接出力を外した。`key_input` の値渡し改善と optional logging provider 化は T09 で続けて扱う。

## 対応: 2026-05-06

- `key_input` を `fn key_input(model: &mut TypingModel, ...) -> TypingTransition` に変更し、頻繁な入力処理で `TypingModel` を move しない形にした。
- `Model` return enum を削除し、typing state transition は `TypingTransition` だけを返すようにした。
- `typing.rs` の no-op logging と `format!` allocation を削除し、default build で入力文字列を生成しないようにした。
- `ん` 自動確定は内部合成 key を user input log / type count に記録しないように修正した。

## 検証: 2026-05-06

- `cargo fmt --check`
- `cargo test --no-default-features`
- `cargo clippy --no-default-features --all-targets -- -W clippy::all`
- `cargo check --no-default-features --features gui`
- `cargo check --no-default-features --features tui`
- `cargo check --no-default-features --features wasm --target wasm32-unknown-unknown`
- `cargo check --no-default-features --features mobile`
- `cargo check --no-default-features --features uefi --target x86_64-unknown-uefi`
