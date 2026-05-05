---
id: ISS-20260505T000000Z-TP-DOC-003-CARGO-COMMENT-MOJIBAKE
title: "Cargo.tomlのコメントがmojibakeしている"
area: docs
status: open
resolved: false
priority: P3
type: docs
created: 2026-05-05
updated: 2026-05-05
target: "Cargo.toml"
legacy_id: TP-DOC-003
source: "doc/fullreview20260505/quality/docs-assets.md"
---
# TP-DOC-003: Cargo.tomlのコメントがmojibakeしている

## 概要

`Cargo.toml` の日本語コメントが mojibake している。source file は UTF-8 に統一する方針と合っていない。

## 影響

feature や dependency の意図が読みにくく、保守者が設定を誤解しやすい。

## 修正方針

- コメントを UTF-8 の日本語または英語に直す。
- 不要なコメントは削除する。
- text file encoding を UTF-8 に統一する。

## 検証

- UTF-8 として読み直してコメントが正しく表示されること。
