---
id: ISS-20260505T000000Z-TP-MOBILE-001-MUTEX-UI-STATE
title: "mobile backendがUI callback内でArc<Mutex<App>>とunwrapに依存している"
area: mobile
status: open
resolved: false
priority: P2
type: architecture
created: 2026-05-05
updated: 2026-05-05
target: "src/mobile.rs"
legacy_id: TP-MOBILE-001
source: "doc/fullreview20260505/platforms/mobile.md"
---
# TP-MOBILE-001: mobile backendがUI callback内でArc<Mutex<App>>とunwrapに依存している

## 概要

mobile backend は Slint UI callback の中で `Arc<Mutex<App>>` を使い、`.lock().unwrap()` に依存している。

## 影響

single-threaded UI callback では mutex の利点が薄く、panic poison による二次 crash のリスクが増える。state ownership の意図も読みにくい。

## 修正方針

- UI thread 専用なら `Rc<RefCell<_>>` または Slint state model に寄せる。
- multi-thread が必要なら message passing を明示する。
- view snapshot を使い、callback が `App` internals を直接触らないようにする。

## 検証

- mobile feature check
- callback error path smoke
