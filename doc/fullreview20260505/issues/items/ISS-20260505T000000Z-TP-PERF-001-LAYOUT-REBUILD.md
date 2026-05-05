---
id: ISS-20260505T000000Z-TP-PERF-001-LAYOUT-REBUILD
title: "Layout mappingがsessionごとに再構築される"
area: performance
status: verified
resolved: true
priority: P2
type: performance
created: 2026-05-05
updated: 2026-05-06
target: "src/model.rs, src/layout_data.rs"
legacy_id: TP-PERF-001
source: "doc/fullreview20260505/core/model-layout.md"
---
# TP-PERF-001: Layout mappingがsessionごとに再構築される

## 概要

`Layout::default()` は key mapping を runtime に構築する。default mapping はほぼ static data なので、毎 session 作り直す必要は薄い。

## 影響

低性能 target や no_std 環境で不要な allocation が増える。lookup 構造の責務も曖昧になる。

## 修正方針

- default mapping を lazy static または generated static table にする。
- user configurable mapping と built-in mapping を分ける。
- lookup は trie / explicit map を検討する。

## 検証

- layout mapping test
- allocation 回数の比較

## 対応: 2026-05-06

- `LayoutData` を導入し、default layout mapping / normalized lookup / first-byte index を `spin::Once` で lazy 初期化するようにした。
- `Layout` は `&'static LayoutData` を持つ軽量な `Copy` 型に変更し、typing session ごとの mapping 再構築を避けるようにした。
- lookup は `normalized_mapping_by_first_byte` / `normalized_mapping_at` method 経由に閉じた。

## 検証: 2026-05-06

- `cargo fmt --check`
- `cargo test --no-default-features`
- `cargo clippy --no-default-features --all-targets -- -W clippy::all`
- `cargo check --no-default-features --features gui`
- `cargo check --no-default-features --features tui`
- `cargo check --no-default-features --features wasm --target wasm32-unknown-unknown`
- `cargo check --no-default-features --features mobile`
- `cargo check --no-default-features --features uefi --target x86_64-unknown-uefi`
