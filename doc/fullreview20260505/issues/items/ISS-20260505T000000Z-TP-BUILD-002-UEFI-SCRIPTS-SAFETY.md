---
id: ISS-20260505T000000Z-TP-BUILD-002-UEFI-SCRIPTS-SAFETY
title: "UEFI実行scriptsのpath解決と破壊的操作が安全境界として弱い"
area: build
status: verified
resolved: true
priority: P2
type: build
created: 2026-05-05
updated: 2026-05-06
target: "run_uefi.ps1, run_uefi_hyperv.ps1"
legacy_id: TP-BUILD-002
source: "doc/fullreview20260505/security/dev-tools.md"
---
# TP-BUILD-002: UEFI実行scriptsのpath解決と破壊的操作が安全境界として弱い

## 概要

`run_uefi.ps1` は作成前の `uefi_image` に `Resolve-Path` を使う可能性がある。`run_uefi_hyperv.ps1` は VM / VHD の削除操作を含み、target path の確認が弱い。

## 影響

初回実行で失敗したり、想定外の VM / VHD を削除する危険がある。UEFI target の検証を安定して回しにくい。

## 修正方針

- directory 作成後に absolute path を resolve する。
- destructive operation は resolved path と VM 名を明示確認する。
- QEMU / OVMF path を config 化する。

## 検証

- clean workspace で script smoke
- dry-run mode
