---
id: ISS-20260505T000000Z-TP-WEB-001-CLIPBOARD-SIDE-EFFECT
title: "web版が初回pointerdownでclipboardを書き換える"
area: web
status: verified
resolved: true
priority: P1
type: ux
created: 2026-05-05
updated: 2026-05-06
target: "index.html"
legacy_id: TP-WEB-001
source: "doc/fullreview20260505/security/web-behavior.md"
---
# TP-WEB-001: web版が初回pointerdownでclipboardを書き換える

## 概要

`index.html` は初回 pointerdown で `navigator.clipboard.writeText('Neknaj Typing MP wasm edition')` を実行する。typing app の通常操作として期待される挙動ではない。

## 影響

利用者の clipboard を予期せず変更する。browser permission と user gesture の条件を満たしていても、明示的な copy 操作ではない。

## 修正方針

- 自動 clipboard 書き込みを削除する。
- copy 機能が必要なら明示 button と user action に限定する。

## 検証

- 初回 pointerdown で clipboard API が呼ばれないこと。

## 対応: 2026-05-06

- `index.html` の初回 `pointerdown` listener と `navigator.clipboard.writeText(...)` 呼び出しを削除した。
- web 版の通常 pointer / keyboard 操作は clipboard API に触れない。

## 検証: 2026-05-06

- `Select-String -Path index.html -Pattern 'clipboard'` が該当なし。
- `cargo check --no-default-features --features wasm --target wasm32-unknown-unknown`
