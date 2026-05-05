---
id: ISS-20260505T000000Z-TP-PERF-002-TEXT-MEASURE-CACHE
title: "text measurementとrasterizationが再計算されやすい"
area: performance
status: open
resolved: false
priority: P2
type: performance
created: 2026-05-05
updated: 2026-05-05
target: "src/renderer.rs, src/ui.rs, src/app.rs"
legacy_id: TP-PERF-002
source: "doc/fullreview20260505/architecture/rendering.md"
---
# TP-PERF-002: text measurementとrasterizationが再計算されやすい

## 概要

typing text、status、scroll calculation で text measurement と rasterization が backend ごとに再計算されやすい。fractional width を早く `u32` に落とす箇所もある。

## 影響

scroll 位置が platform ごとにずれる可能性がある。低性能 target では frame time が悪化する。

## 修正方針

- font / size / string / style key の measurement cache を導入する。
- 内部 layout は logical pixel / float で保持し、backend transfer の最後で丸める。
- render tree と measurement cache の責務を分ける。

## 検証

- scroll visibility test
- representative text の measurement snapshot
