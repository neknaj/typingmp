---
id: ISS-20260505T000000Z-TP-SEC-002-LOGGER-SERVER-DEV-ONLY
title: "debug logger serverにdev-only境界と上限がない"
area: security
status: open
resolved: false
priority: P2
type: security
created: 2026-05-05
updated: 2026-05-06
target: "logger_server.js, src/wasm_debug_logger.rs"
legacy_id: TP-SEC-002
source: "doc/fullreview20260505/security/dev-tools.md"
---
# TP-SEC-002: debug logger serverにdev-only境界と上限がない

## 概要

debug WebSocket logger は認証、message size limit、rate limit、log retention の境界がない。local dev 用なら許容できるが、配布や network 公開とは分けるべきである。

## 影響

不用意に公開すると log spam や resource exhaustion の入口になる。入力 log と組み合わさると privacy 上も不適切である。

## 修正方針

- bind address を `127.0.0.1` に固定する。
- message size と log size の上限を設ける。
- README に dev-only と明記する。
- WASM debug logger は opt-in feature にする。

## 検証

- local bind 確認
- oversized message の拒否 test

## 進捗: T05

I/O provider 境界の一部として `Logger` trait と `NoopLogger` を追加し、core / `App` 側の default logger は no-op に寄せた。debug WebSocket logger server 自体の bind / size / rate limit は T18 で扱う。
