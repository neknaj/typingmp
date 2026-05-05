---
id: ISS-20260505T000000Z-TP-WASM-001-DOM-STORAGE-ERRORS
title: "WASM/webがDOMとstorage失敗をunwrapまたはsilent fallbackにしている"
area: wasm
status: open
resolved: false
priority: P1
type: bug
created: 2026-05-05
updated: 2026-05-05
target: "src/wasm.rs, index.html"
legacy_id: TP-WASM-001
source: "doc/fullreview20260505/platforms/wasm-web.md"
---
# TP-WASM-001: WASM/webがDOMとstorage失敗をunwrapまたはsilent fallbackにしている

## 概要

WASM/web backend は `window`、`document`、canvas、2D context、RAF、ImageData、localStorage などの失敗を `unwrap()` または silent fallback で扱う箇所がある。

## 影響

host page の DOM が変わるだけで crash する。custom problem や settings の読み込み失敗を利用者が判断できない。

## 修正方針

- init を `Result<(), JsValue>` に分ける。
- missing DOM は visible error にする。
- storage parse / schema / size error を diagnostic として表示する。

## 検証

- missing canvas / missing input の browser smoke
- corrupt localStorage fixture
