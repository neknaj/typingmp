---
id: ISS-20260505T000000Z-TP-ARCH-001-APP-GOD-OBJECT
title: "Appがscene/input/problem/scroll/fontを抱えすぎている"
area: architecture
status: verified
resolved: true
priority: P1
type: architecture
created: 2026-05-05
updated: 2026-05-06
target: "src/app.rs, src/ui.rs"
legacy_id: TP-ARCH-001
source: "doc/fullreview20260505/architecture/modularization.md"
---
# TP-ARCH-001: Appがscene/input/problem/scroll/fontを抱えすぎている

## 概要

`src/app.rs` は 1000 行を超え、`App` が state transition、typing update、problem management、font loading、scroll cache、render input をまとめて持っている。

## 影響

backend ごとの修正が core state に波及しやすく、typing core の pure test が書きにくい。public mutable field が多いため、state invariant を型で守りにくい。

## 修正方針

- `app/state.rs`、`app/problems.rs`、`app/scroll.rs`、`app/input.rs`、`app/view_model.rs` に責務を分ける。
- backend は `App` internals ではなく view snapshot を読む。
- platform I/O と font discovery は adapter service に寄せる。

## 検証

- `cargo test --no-default-features`
- `cargo check --no-default-features --features gui`
- scene transition の unit test

## 対応: 2026-05-06

- `src/app/view.rs` を追加し、backend / UI が表示状態を読むための immutable `AppSnapshot` を導入した。
- `src/app/problems.rs` に problem repository 操作を分離し、builtin / custom / open-file の管理責務を `app.rs` 本体から外した。
- `src/app/scroll.rs` に scroll cache と cursor position 計算を分離し、毎フレーム描画用 cache を `App` 内部の query method 経由に閉じた。
- `App` の mutable field は crate 内部可視に絞り、外部 API は method / snapshot 経由にした。

## 検証: 2026-05-06

- `cargo fmt --check`
- `cargo test --no-default-features`
- `cargo clippy --no-default-features --all-targets -- -W clippy::all`
- `cargo check --no-default-features --features gui`
- `cargo check --no-default-features --features tui`
- `cargo check --no-default-features --features wasm --target wasm32-unknown-unknown`
- `cargo check --no-default-features --features mobile`
- `cargo check --no-default-features --features uefi --target x86_64-unknown-uefi`
