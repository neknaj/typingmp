---
id: ISS-20260505T000000Z-TP-ARCH-003-PUBLIC-PRIVATE-INTERFACE
title: "public fieldがprivate型を露出しAPI境界が崩れている"
area: architecture
status: open
resolved: false
priority: P2
type: architecture
created: 2026-05-05
updated: 2026-05-05
target: "src/app.rs"
legacy_id: TP-ARCH-003
source: "doc/fullreview20260505/architecture/modularization.md"
---
# TP-ARCH-003: public fieldがprivate型を露出しAPI境界が崩れている

## 概要

`App::scroll_cache` など public field が crate-private / private type を含み、compiler warning が出ている。

## 影響

外部から触れるべきでない cache や cursor state が API surface に漏れている。backend が internal invariant に依存すると、refactor 時の破壊範囲が広がる。

## 修正方針

- internal field を private 化する。
- backend が必要な情報は query method または immutable snapshot で返す。
- cache type は app module 内に閉じる。

## 検証

- `cargo check --no-default-features`
- private interface warning が消えること。
