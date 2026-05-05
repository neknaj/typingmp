---
id: ISS-20260505T000000Z-TP-ARCH-002-RENDERER-DUPLICATION
title: "backendごとにrendering pipelineが重複している"
area: architecture
status: verified
resolved: true
priority: P1
type: architecture
created: 2026-05-05
updated: 2026-05-06
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

## 修正結果

`src/renderer.rs` に `ArgbSurface` と `RenderCache` を導入し、GUI / WASM / mobile / UEFI の render tree 描画を同じ ARGB renderer に集約した。各 backend は `ui::build_ui` の結果を shared renderer に渡し、target 固有の仕事を buffer 転送と input adapter に限定する。

## 検証

- GUI / WASM / mobile / UEFI の compile check
- representative scene の pixel smoke test
- `cargo test --no-default-features`

## 追加観測: 2026-05-06

play 画面で、上段の typing target 表示と下段の入力表示における同じ文字範囲の色状態が一致していない。これは view / renderer が同じ progress state を同じ規則で描画できていない問題として扱い、T17 で上下表示の色を揃える。

T17 では plain text の active upper segment を文字単位の `UpperSegmentState` に展開し、下段の typed character と同じ correctness state で描画するようにした。
