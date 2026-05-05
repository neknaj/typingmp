---
id: ISS-20260505T000000Z-TP-STATIC-001-ENUM-MATCH-STATE
title: "状態とcommandがraw number/raw stringに依存しenumとmatchの網羅性検査が効いていない"
area: static-safety
status: verified
resolved: true
priority: P1
type: type-safety
created: 2026-05-05
updated: 2026-05-06
target: "src/app.rs, src/model.rs, src/typing.rs, src/mobile.rs, src/wasm.rs"
legacy_id: TP-STATIC-001
source: "doc/fullreview20260505/quality/static-safety.md"
---
# TP-STATIC-001: 状態とcommandがraw number/raw stringに依存しenumとmatchの網羅性検査が効いていない

## 概要

typing cursor、menu selection、problem source badge、UI action bridge などに raw number / raw string state が残っている。開発方針では、有限状態は enum で表し、`match` の網羅性検査を効かせる必要がある。

## 影響

新しい scene、command、source kind、cursor state を追加した時に、修正漏れが compile error として検出されない。runtime convention に依存すると multi-platform adapter 間で挙動がずれやすい。

## 修正方針

- `LineIndex`、`WordIndex`、`SegmentIndex`、`CharIndex` などの newtype を導入する。
- menu item / settings item / problem source kind / UI command を enum にする。
- string boundary は adapter の入口で enum に parse し、core には raw string command を渡さない。
- enum branch は `_` で握りつぶさず、`match` で variant を明示する。

## 検証

- `cargo clippy` で unchecked cast と magic string 分岐を減らす。
- enum variant 追加時に compile error が出る構造になっていること。

## 修正結果

typing cursor は `LineIndex` / `WordIndex` / `SegmentIndex` / `CharIndex` newtype に移行した。main menu と settings selection は enum にし、adapter bridge 由来の string command は `UiCommand` で parse してから `AppEvent` に変換するようにした。problem source は T05 で typed `ProblemId` / `ProblemSourceKind` を導入済み。

検証:

- `cargo fmt --check`: pass
- `cargo test --no-default-features`: pass
- `cargo clippy --no-default-features --all-targets -- -W clippy::all`: pass
- `cargo check --no-default-features --features gui`: pass
- `cargo check --no-default-features --features tui`: pass
- `cargo check --no-default-features --features mobile`: pass
- `cargo check --no-default-features --features wasm --target wasm32-unknown-unknown`: pass
- `cargo check --no-default-features --features uefi --target x86_64-unknown-uefi`: pass
