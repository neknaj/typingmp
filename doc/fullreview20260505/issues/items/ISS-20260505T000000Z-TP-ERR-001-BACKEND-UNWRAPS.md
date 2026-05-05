---
id: ISS-20260505T000000Z-TP-ERR-001-BACKEND-UNWRAPS
title: "platform backendに環境依存unwrapが多い"
area: error-handling
status: open
resolved: false
priority: P1
type: bug
created: 2026-05-05
updated: 2026-05-05
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
