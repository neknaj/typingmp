---
id: ISS-20260505T000000Z-TP-ARCH-005-IO-PROVIDER-VFS
title: "problem/font/storage/loggerのI/Oがprovider境界で仮想化されていない"
area: architecture
status: open
resolved: false
priority: P1
type: architecture
created: 2026-05-05
updated: 2026-05-06
target: "src/app.rs, src/gui.rs, src/tui.rs, src/wasm.rs, src/mobile.rs"
legacy_id: TP-ARCH-005
source: "doc/fullreview20260505/architecture/io-virtualization.md"
---
# TP-ARCH-005: problem/font/storage/loggerのI/Oがprovider境界で仮想化されていない

## 概要

typingmp は no_std core と target-specific UI を組み合わせる方針だが、現状は `App` と backend が filesystem、font path、localStorage、DOM、logger を直接扱う。NEPLg2 の `SourceMap` / provider VFS / preopen root のような I/O 境界が typingmp 側にはまだない。

## 影響

desktop の都合が core に入り込みやすい。UEFI や web など filesystem を持たない target で、同じ typing core を使い回す設計が難しくなる。

## 修正方針

- `ProblemSourceProvider`、`AssetProvider`、`PersistentStore`、`Clock`、`Logger` を adapter service として導入する。
- core は raw path ではなく `ProblemId` / source label を扱う。
- parser diagnostic は source label と span を持つ。
- default logger は no-op にし、debug websocket は opt-in にする。

## 検証

- no_std core check
- desktop / web / UEFI で provider 差し替え compile
- custom problem import failure の diagnostic test

## 進捗: T05

`src/io.rs` を追加し、`ProblemSourceProvider`、`AssetProvider`、`PersistentStore`、`Clock`、`Logger`、typed `ProblemId` / `FontAssetId`、`ProblemRepository` を導入した。`App` は custom problem repository と font catalog / request を扱い、desktop filesystem path、font discovery、font bytes loading、web localStorage の詳細を直接持たない形にした。

backend 側の接続:

- GUI / TUI / mobile は `DesktopAssetProvider` 経由で bundled font と font catalog を取得する。
- GUI file import は `DesktopProblemSourceProvider` 経由で `.ntq` を読み込む。
- WASM custom problem storage は `PersistentStore` 実装に閉じた。
- default logger provider として `NoopLogger` を定義した。

検証:

- `cargo fmt --check`: pass
- `cargo test --no-default-features`: pass
- `cargo clippy --no-default-features --all-targets -- -W clippy::all`: pass
- `cargo check --no-default-features --features tui`: pass
- `cargo check --no-default-features --features gui`: pass
- `cargo check --no-default-features --features gui-file`: pass
- `cargo check --no-default-features --features mobile`: pass
- `cargo check --no-default-features --features wasm --target wasm32-unknown-unknown`: pass
- `cargo check --no-default-features --features uefi --target x86_64-unknown-uefi`: pass

残作業:

- parser diagnostic の source label / span は T08 で扱う。
- backend error contract と visible error 化は T12 / T14 で扱う。
