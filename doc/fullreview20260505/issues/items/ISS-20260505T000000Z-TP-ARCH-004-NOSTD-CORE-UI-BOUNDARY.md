---
id: ISS-20260505T000000Z-TP-ARCH-004-NOSTD-CORE-UI-BOUNDARY
title: "no_std typing coreとtarget UI adapterの境界が固定されていない"
area: architecture
status: open
resolved: false
priority: P1
type: architecture
created: 2026-05-05
updated: 2026-05-05
target: "src/lib.rs, src/parser.rs, src/typing.rs, src/model.rs, src/layout_data.rs, src/app.rs"
legacy_id: TP-ARCH-004
source: "doc/fullreview20260505/architecture/core-ui-boundary.md"
---
# TP-ARCH-004: no_std typing coreとtarget UI adapterの境界が固定されていない

## 概要

project の目標は、no_std レベルで動く typing core と target ごとに具体化する UI を組み合わせることである。現状は `uefi` feature の時だけ crate 全体を `no_std` にしており、core が常に `core + alloc` で成立することを検証できない。

## 根拠

NEPLg2 は `nepl-core` を `#![no_std]` にし、`nepl-cli` と `nepl-web` が core に依存する構造を取っている。typingmp も `typingmp-core` と backend adapter の一方向依存に寄せられる。

## 影響

desktop / web の都合で core に `std` や platform I/O が入り込むと、UEFI や将来の constrained target で同じ typing game を動かす目標が崩れる。

## 修正方針

- parser / typing / model / layout を `core + alloc` 前提に統一する。
- font discovery、filesystem、DOM、terminal、firmware API を adapter 側に移す。
- no_std core check を CI に追加する。
- 中期的に `typingmp-core` crate 化を検討する。

## 検証

- no_std core-only check
- `cargo check --no-default-features --features uefi --target x86_64-unknown-uefi`
