---
id: ISS-20260505T000000Z-TP-CI-003-WORKFLOW-MATRIX-ACTIONS
title: "CI workflowがmulti-target feature matrixを十分に固定していない"
area: ci
status: verified
resolved: true
priority: P2
type: quality
created: 2026-05-05
updated: 2026-05-06
target: ".github/workflows/release.yml"
legacy_id: TP-CI-003
source: "doc/fullreview20260505/project/verification-status.md"
---
# TP-CI-003: CI workflowがmulti-target feature matrixを十分に固定していない

## 概要

project は GUI / TUI / WASM / mobile / UEFI を持つが、CI の target / feature / profile の組み合わせが設計意図として明文化されていない。古い `actions-rs` action も残る。

## 根拠

local check では target ごとに異なる warning と failure が出た。WASM debug build は env 依存で失敗し、UEFI は compile は通るが runtime failure を検出できない。

## 影響

特定 platform の regression が main に入りやすい。multi-platform typing app としての信頼性を CI が保証できない。

## 修正方針

- workflow を maintained action に更新する。
- feature matrix を明示する。
- WASM debug / release の両方を check する。
- no_std core check を追加する。

## 検証

- GitHub Actions の matrix job が全 target で通ること。
- local でも同等の check script を実行できること。
