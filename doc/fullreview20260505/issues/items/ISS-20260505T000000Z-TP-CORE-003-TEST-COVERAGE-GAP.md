---
id: ISS-20260505T000000Z-TP-CORE-003-TEST-COVERAGE-GAP
title: "parser以外のcore regression testが不足している"
area: quality
status: verified
resolved: true
priority: P1
type: test
created: 2026-05-05
updated: 2026-05-06
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

## 対応: 2026-05-06

- `src/typing.rs` に direct kana / romaji prefix / miss の regression test を追加し、typing cursor と correctness 更新を固定した。
- `src/layout_data.rs` に `し` の複数ローマ字、促音 `っか` の collision、ASCII direct mapping の test を追加した。
- `src/app.rs` に main menu -> problem selection、builtin problem start、不正 problem index の scene transition test を追加した。

## 検証: 2026-05-06

- `cargo fmt --check`
- `cargo test --no-default-features`
- `cargo clippy --no-default-features --all-targets -- -W clippy::all`
- `cargo check --no-default-features --features wasm --target wasm32-unknown-unknown`
