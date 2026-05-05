---
id: ISS-20260505T000000Z-TP-PERF-001-LAYOUT-REBUILD
title: "Layout mappingがsessionごとに再構築される"
area: performance
status: open
resolved: false
priority: P2
type: performance
created: 2026-05-05
updated: 2026-05-05
target: "src/model.rs, src/layout_data.rs"
legacy_id: TP-PERF-001
source: "doc/fullreview20260505/core/model-layout.md"
---
# TP-PERF-001: Layout mappingがsessionごとに再構築される

## 概要

`Layout::default()` は key mapping を runtime に構築する。default mapping はほぼ static data なので、毎 session 作り直す必要は薄い。

## 影響

低性能 target や no_std 環境で不要な allocation が増える。lookup 構造の責務も曖昧になる。

## 修正方針

- default mapping を lazy static または generated static table にする。
- user configurable mapping と built-in mapping を分ける。
- lookup は trie / explicit map を検討する。

## 検証

- layout mapping test
- allocation 回数の比較
