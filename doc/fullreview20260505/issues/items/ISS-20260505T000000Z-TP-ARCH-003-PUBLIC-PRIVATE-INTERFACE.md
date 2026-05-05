---
id: ISS-20260505T000000Z-TP-ARCH-003-PUBLIC-PRIVATE-INTERFACE
title: "public fieldがprivate型を露出しAPI境界が崩れている"
area: architecture
status: verified
resolved: true
priority: P2
type: architecture
created: 2026-05-05
updated: 2026-05-06
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

## 対応: 2026-05-06

- `App::scroll_cache` を private field に戻し、描画側は `App::scroll_cache()` の immutable query を使うようにした。
- `ScrollCacheState::cursor_state` と unused `ScrollCache::Empty` を削除し、private type が public field に漏れる原因を除去した。
- `App` の mutable fields を `pub(crate)` に落とし、外部 API は `AppSnapshot` / query method / drain method に寄せた。

## 検証: 2026-05-06

- `cargo check --no-default-features`
- `cargo clippy --no-default-features --all-targets -- -W clippy::all`
- private interface warning が出ないことを確認した。
