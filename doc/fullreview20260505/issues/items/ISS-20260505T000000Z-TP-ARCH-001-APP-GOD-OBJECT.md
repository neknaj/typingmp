---
id: ISS-20260505T000000Z-TP-ARCH-001-APP-GOD-OBJECT
title: "Appがscene/input/problem/scroll/fontを抱えすぎている"
area: architecture
status: open
resolved: false
priority: P1
type: architecture
created: 2026-05-05
updated: 2026-05-05
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
