---
id: ISS-20260505T000000Z-TP-TUI-001-TERMINAL-GUARD
title: "TUI raw modeとalternate screenの復旧がRAII化されていない"
area: tui
status: open
resolved: false
priority: P1
type: bug
created: 2026-05-05
updated: 2026-05-05
target: "src/tui.rs"
legacy_id: TP-TUI-001
source: "doc/fullreview20260505/platforms/tui.md"
---
# TP-TUI-001: TUI raw modeとalternate screenの復旧がRAII化されていない

## 概要

TUI は raw mode と alternate screen を有効化し、通常終了時に戻す。途中 error や panic で cleanup に到達しない場合、端末が壊れた状態で残る可能性がある。

## 影響

typing game の TUI は key input を多用するため、raw mode 復旧失敗は利用者体験として重大である。

## 修正方針

- `TerminalGuard` を導入し、`Drop` で状態を戻す。
- setup phase で部分的に有効化された state も復旧する。
- panic hook で最低限の cleanup を試みる。

## 検証

- setup failure simulation
- panic path smoke
- normal exit で alternate screen / raw mode が戻ること。
