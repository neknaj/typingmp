---
id: ISS-20260505T000000Z-TP-ERR-001-BACKEND-UNWRAPS
title: "platform backendに環境依存unwrapが多い"
area: error-handling
status: open
resolved: false
priority: P1
type: bug
created: 2026-05-05
updated: 2026-05-06
target: "src/wasm.rs, src/mobile.rs, src/uefi.rs, src/tui.rs"
legacy_id: TP-ERR-001
source: "doc/fullreview20260505/platforms/wasm-web.md"
---
# TP-ERR-001: platform backendに環境依存unwrapが多い

## 概要

WASM、mobile、UEFI、TUI で host API や internal state を `unwrap()` する箇所が多い。platform 依存の失敗が起きた時に user-visible error ではなく crash になる。

## 影響

browser DOM、mobile surface、firmware protocol、terminal state は環境差が大きい。multi-platform project では、compile が通るだけでは十分でない。

## 修正方針

- platform init は `Result` を返す。
- render / storage / asset failure は visible error state に変換する。
- TUI と UEFI は panic しても復旧できる guard / fallback を持つ。

## 検証

- backend init error path test
- WASM missing DOM smoke
- TUI guard cleanup smoke

## 進捗: T12

- `src/backend.rs` を追加し、backend init / asset / render / storage / DOM / state failure を `BackendErrorKind` と `BackendError` で分類する契約を導入した。
- GUI / TUI / mobile の font parse failure は typed error に変換し、runtime の font load / import failure は `App::report_visible_error()` で status 表示へ流すようにした。
- TUI の render path に残っていた `typing_model().unwrap()` は missing state 時に frame を skip する形へ変更した。
- WASM の `window` / `document` / `body` / canvas context / `ImageData` / RAF は `Result` または console-visible error に変換した。
- mobile の `Arc<Mutex<App>>` lock は poison 時に panic せず callback を抜ける形へ変えた。

残作業:

- TUI terminal state の復旧 guard は T13 で実装する。
- WASM の user-visible DOM / storage diagnostic と clipboard side effect は T14 で実装する。
- mobile state ownership の設計見直しは T15 で実装する。
- UEFI firmware API と timer / blit の `unwrap()` は T16 で実装する。

検証:

- `cargo fmt --check`: pass
- `cargo test --no-default-features`: pass
- `cargo clippy --no-default-features --all-targets -- -W clippy::all`: pass
- `cargo check --no-default-features`: pass
- `cargo check --no-default-features --features wasm --target wasm32-unknown-unknown`: pass
