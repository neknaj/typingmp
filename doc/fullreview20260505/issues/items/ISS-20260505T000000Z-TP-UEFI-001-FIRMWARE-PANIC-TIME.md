---
id: ISS-20260505T000000Z-TP-UEFI-001-FIRMWARE-PANIC-TIME
title: "UEFI backendがfirmware API unwrapと粗いtimestampに依存している"
area: uefi
status: verified
resolved: true
priority: P1
type: bug
created: 2026-05-05
updated: 2026-05-06
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

## 進捗: T16

- `src/uefi.rs` の init / GOP open / timer / event wait / blit / embedded font load を `run_inner() -> Result<(), Status>` に集約し、firmware failure を panic ではなく UEFI `Status` として返すようにした。startup failure は UEFI console にも出す。
- `src/timestamp.rs` の UEFI path は `runtime::get_time()` failure を fallback にし、year/month を固定秒数で足す近似ではなく leap year と month length を含む Unix millisecond 変換にした。
- `src/typing.rs` に残っていた runtime `last_mut().unwrap()` を invariant break 時に `TypingTransition::Ignored` へ落とす形にした。

検証:

- `cargo fmt --check`: pass
- `cargo test --no-default-features`: pass
- `cargo clippy --no-default-features --all-targets -- -W clippy::all`: pass
- `cargo check --no-default-features --features uefi --target x86_64-unknown-uefi`: pass (`cdylib` unsupported warning only)
- QEMU smoke: local environment lacks `qemu-system-x86_64.exe` and `C:\qemu\OVMF.fd`; not run.
