---
id: ISS-20260505T000000Z-TP-CI-001-WASM-DEBUG-ENV
title: "WASM debug buildがWEBSOCKET_ADDRESS未設定でcompile不能になる"
area: ci
status: verified
resolved: true
priority: P0
type: bug
created: 2026-05-05
updated: 2026-05-06
target: "src/wasm_debug_logger.rs, .github/workflows/release.yml"
legacy_id: TP-CI-001
source: "doc/fullreview20260505/project/verification-status.md"
---
# TP-CI-001: WASM debug buildがWEBSOCKET_ADDRESS未設定でcompile不能になる

## 概要

`src/wasm_debug_logger.rs` は debug build で `env!("WEBSOCKET_ADDRESS", ...)` を要求する。環境変数が未設定だと、WASM target の compile が失敗する。

## 根拠

`cargo check --no-default-features --features wasm --target wasm32-unknown-unknown` は `WEBSOCKET_ADDRESS` 未設定で失敗した。`WEBSOCKET_ADDRESS=ws://localhost:8081` を設定すると同じ check は通る。

## 影響

release workflow や local debug build が、debug logger の設定漏れだけで compile 不能になる。typing core や web UI の変更と無関係に WASM build が赤くなるため、CI の信頼性を落とす。

## 修正方針

- `env!` ではなく `option_env!` を使い、未設定時は logger を disabled にする。
- debug websocket logger を明示 feature に切り出す。
- release workflow で build profile と env を明示する。

## 検証

- `cargo check --no-default-features --features wasm --target wasm32-unknown-unknown`
- logger feature 有効時の local websocket 接続 smoke

## 修正結果

`env!` を `option_env!` に置き換え、`WEBSOCKET_ADDRESS` 未設定時は WebSocket logging を無効化して compile を継続するようにした。random ID 生成失敗時も panic せず、WebSocket logging を無効化する。

検証:

- `cargo fmt --check`: pass
- `cargo check --no-default-features --features wasm --target wasm32-unknown-unknown`: pass, `WEBSOCKET_ADDRESS` 未設定
- `$env:WEBSOCKET_ADDRESS='ws://localhost:8081'; cargo check --no-default-features --features wasm --target wasm32-unknown-unknown`: pass
- `cargo test --no-default-features`: pass
