---
id: ISS-20260505T000000Z-TP-PERF-003-UEFI-FRAME-ALLOC
title: "UEFI backendがframeごとに大きなbufferを確保している"
area: performance
status: verified
resolved: true
priority: P1
type: performance
created: 2026-05-05
updated: 2026-05-06
target: "src/uefi.rs, src/renderer.rs"
legacy_id: TP-PERF-003
source: "doc/fullreview20260505/platforms/uefi.md"
---
# TP-PERF-003: UEFI backendがframeごとに大きなbufferを確保している

## 概要

UEFI backend は full-screen buffer と background buffer を frame ごとに確保・変換する。firmware 環境では allocation failure と frame time の両方が問題になる。

## 影響

高解像度画面やメモリ制約のある firmware で動作が不安定になる。no_std target を重視する project 目標に反する。

## 修正方針

- persistent framebuffer を再利用する。
- background gradient は size change 時だけ再計算する。
- 差分更新または必要範囲 blit を検討する。

## 修正結果

UEFI backend は loop の外で ARGB buffer と `BltPixel` buffer を確保し、frame ごとに再利用する。background gradient は shared `RenderCache` により size / color が変わるまで再計算しない。

## 検証

- UEFI target compile
- QEMU smoke
- allocation 回数の instrumentation
- `cargo check --no-default-features --features uefi --target x86_64-unknown-uefi`
