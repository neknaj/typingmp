---
id: ISS-20260505T000000Z-TP-UEFI-001-FIRMWARE-PANIC-TIME
title: "UEFI backendがfirmware API unwrapと粗いtimestampに依存している"
area: uefi
status: open
resolved: false
priority: P1
type: bug
created: 2026-05-05
updated: 2026-05-05
target: "src/uefi.rs, src/timestamp.rs"
legacy_id: TP-UEFI-001
source: "doc/fullreview20260505/platforms/uefi.md"
---
# TP-UEFI-001: UEFI backendがfirmware API unwrapと粗いtimestampに依存している

## 概要

UEFI backend は firmware protocol、stdout、event wait、graphics output などで `unwrap()` を使う。`timestamp.rs` の UEFI timestamp も粗い月・年計算と `unwrap()` を含む。

## 影響

firmware 実装差や device failure で診断不能な panic になりやすい。ログや ordering に timestamp を使う場合、正確性と fallback が曖昧になる。

## 修正方針

- UEFI init を段階的な `Result` にする。
- screen fallback error を用意する。
- timestamp は adapter `Clock` へ移し、monotonic duration と wall clock を分ける。

## 検証

- UEFI target check
- QEMU smoke
