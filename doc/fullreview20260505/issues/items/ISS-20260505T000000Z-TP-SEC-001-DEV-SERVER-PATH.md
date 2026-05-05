---
id: ISS-20260505T000000Z-TP-SEC-001-DEV-SERVER-PATH
title: "serve.jsのpath traversal guardがprefix checkに依存している"
area: security
status: verified
resolved: true
priority: P1
type: security
created: 2026-05-05
updated: 2026-05-06
target: "serve.js"
legacy_id: TP-SEC-001
source: "doc/fullreview20260505/security/dev-tools.md"
---
# TP-SEC-001: serve.jsのpath traversal guardがprefix checkに依存している

## 概要

`serve.js` は `path.join(ROOT, urlPath)` の結果に対して `startsWith(ROOT)` で traversal を防ごうとしている。prefix check は path normalization と sibling directory に弱い。

## 影響

dev server であっても、workspace 外の file を誤って配信する可能性がある。

## 修正方針

- `path.resolve(ROOT, '.' + urlPath)` を使う。
- resolved path が `ROOT` または `ROOT + path.sep` 配下であることを確認する。
- traversal test を追加する。

## 検証

- `..`、encoded path、sibling prefix path の request test
