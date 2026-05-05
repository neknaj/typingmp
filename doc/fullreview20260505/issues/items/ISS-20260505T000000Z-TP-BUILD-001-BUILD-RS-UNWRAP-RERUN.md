---
id: ISS-20260505T000000Z-TP-BUILD-001-BUILD-RS-UNWRAP-RERUN
title: "build.rsがunwrapとrerun-if-changed不足に依存している"
area: build
status: verified
resolved: true
priority: P2
type: build
created: 2026-05-05
updated: 2026-05-06
target: "build.rs, examples/"
legacy_id: TP-BUILD-001
source: "doc/fullreview20260505/quality/static-validation.md"
---
# TP-BUILD-001: build.rsがunwrapとrerun-if-changed不足に依存している

## 概要

`build.rs` は generated problem entries を作るが、`unwrap()` が多く、examples 変更に対する `cargo:rerun-if-changed` の扱いも明確でない。

## 影響

壊れた example や非 UTF-8 path で build が不親切に失敗する。問題文 source と generated data の drift を追いにくい。

## 修正方針

- `build.rs` の error を context 付き `Result` にする。
- `examples/` と関連 docs に `cargo:rerun-if-changed` を出す。
- generated source の escaping と UTF-8 handling を test する。

## 検証

- clean build
- examples 変更後の rebuild
