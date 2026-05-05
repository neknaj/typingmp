---
id: ISS-20260505T000000Z-TP-DOC-002-GENERATED-ARTIFACTS
title: "生成物とsource artifactの管理境界が曖昧"
area: docs
status: open
resolved: false
priority: P2
type: docs
created: 2026-05-05
updated: 2026-05-05
target: "rust_multibackend_app.efi, pkg/, .playwright-mcp, .gitignore"
legacy_id: TP-DOC-002
source: "doc/fullreview20260505/quality/docs-assets.md"
---
# TP-DOC-002: 生成物とsource artifactの管理境界が曖昧

## 概要

repository root に `rust_multibackend_app.efi` があり、`pkg/` や `.playwright-mcp` も生成物として見える。source と generated artifact の境界が曖昧である。

## 影響

review 対象が増え、生成物 drift を見落としやすい。release artifact と source artifact の責務が混ざる。

## 修正方針

- 不要生成物を `.gitignore` に追加する。
- 配布物は release artifact として管理する。
- `pkg/` を保持するなら理由と更新手順を README に書く。

## 検証

- clean checkout 後の build
- generated artifact drift check
