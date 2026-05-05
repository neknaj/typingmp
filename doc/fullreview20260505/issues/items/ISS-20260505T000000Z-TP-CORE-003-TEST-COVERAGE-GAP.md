---
id: ISS-20260505T000000Z-TP-CORE-003-TEST-COVERAGE-GAP
title: "parser以外のcore regression testが不足している"
area: quality
status: open
resolved: false
priority: P1
type: test
created: 2026-05-05
updated: 2026-05-05
target: "src/parser.rs, src/typing.rs, src/model.rs, src/app.rs"
legacy_id: TP-CORE-003
source: "doc/fullreview20260505/quality/tests.md"
---
# TP-CORE-003: parser以外のcore regression testが不足している

## 概要

`cargo test --no-default-features` は parser test 22 件が中心で、typing input、model invariant、layout lookup、app scene transition の test が不足している。

## 影響

refactor 時に typing correctness の regression を検出できない。multi-platform adapter を増やしても core の正しさを確認できない。

## 修正方針

- `typing::key_input` の unit test を追加する。
- layout mapping collision / prefix handling test を追加する。
- app scene transition と custom problem failure test を追加する。
- README sample を parser fixture にする。

## 検証

- `cargo test --no-default-features`
- core-only test suite
