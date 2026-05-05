---
id: ISS-20260505T000000Z-TP-ARCH-002-RENDERER-DUPLICATION
title: "backendごとにrendering pipelineが重複している"
area: architecture
status: open
resolved: false
priority: P1
type: architecture
created: 2026-05-05
updated: 2026-05-05
target: "src/renderer.rs, src/gui.rs, src/wasm.rs, src/mobile.rs, src/uefi.rs"
legacy_id: TP-ARCH-002
source: "doc/fullreview20260505/architecture/rendering.md"
---
# TP-ARCH-002: backendごとにrendering pipelineが重複している

## 概要

GUI / WASM / mobile / UEFI がそれぞれ frame buffer、canvas、font、transfer 処理を持ち、同じ UI state の描画が複数箇所に分散している。

## 影響

renderer bug fix や性能改善が backend 間で反映漏れしやすい。target 固有の転送処理と共通 UI rendering の境界が読みにくい。

## 修正方針

- `RenderSurface` または類似の surface abstraction を導入する。
- shared render tree / view snapshot を backend に渡す。
- backend は surface transfer と input adapter に集中する。

## 検証

- GUI / WASM / mobile / UEFI の compile check
- representative scene の pixel smoke test
