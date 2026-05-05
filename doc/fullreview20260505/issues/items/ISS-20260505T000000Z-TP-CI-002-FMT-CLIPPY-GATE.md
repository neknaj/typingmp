---
id: ISS-20260505T000000Z-TP-CI-002-FMT-CLIPPY-GATE
title: "cargo fmtとclippy warningが品質gateになっていない"
area: ci
status: open
resolved: false
priority: P1
type: quality
created: 2026-05-05
updated: 2026-05-05
target: "Cargo.toml, src/**/*.rs, .github/workflows/*.yml"
legacy_id: TP-CI-002
source: "doc/fullreview20260505/quality/static-validation.md"
---
# TP-CI-002: cargo fmtとclippy warningが品質gateになっていない

## 概要

`cargo fmt --check` が失敗し、`clippy` warning も多数残っている。現状では format / lint が refactor の防壁として機能していない。

## 根拠

`cargo fmt --check` は `build.rs`、`src/app.rs`、`src/wasm.rs` などに大きな差分を出した。`clippy` では `if_same_then_else`、`ptr_arg`、`useless_vec`、redundant closure、unreachable expression などが出ている。

## 影響

整形差分と意味のある修正が混ざり、review の粒度が粗くなる。warning が常態化すると、platform 固有の本物の regression を見落としやすい。

## 修正方針

- format-only commit を先に作る。
- `no-default-features` の clippy warning を 0 にする。
- feature ごとに `gui`、`tui`、`wasm`、`mobile`、`uefi` の warning を潰す。
- 最終的に CI で `-D warnings` を段階導入する。

## 検証

- `cargo fmt --check`
- `cargo clippy --no-default-features --all-targets -- -D warnings`
