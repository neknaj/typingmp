---
id: ISS-20260505T000000Z-TP-WEB-001-CLIPBOARD-SIDE-EFFECT
title: "web版が初回pointerdownでclipboardを書き換える"
area: web
status: open
resolved: false
priority: P1
type: ux
created: 2026-05-05
updated: 2026-05-05
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
