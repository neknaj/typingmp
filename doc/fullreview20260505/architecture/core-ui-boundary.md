# アーキテクチャレビュー: no_std core と UI adapter 境界

対象 commit: `501bb73fe53a32bdbc100303498a9dc438ff85aa`

## 結論

`typingmp` の目標は、どこでも動く typing core と、幅広く使える UI を target ごとに段階的に具体化する構造である。現状は `uefi` feature の時だけ crate 全体を `no_std` にする形で、typing core が常に `no_std + alloc` で成立することを独立して検証できない。

これはプロジェクトの方向性とずれている。core と UI adapter の境界を crate / module / feature に明示し、platform I/O を core から分離する必要がある。

## 現状

`src/lib.rs` は `#![cfg_attr(feature = "uefi", no_std)]` であり、UEFI build だけが no_std mode になる。`parser.rs`、`typing.rs`、`model.rs`、`layout_data.rs` は `feature = "uefi"` の時に `alloc` を使い、それ以外では `std` を使う。

この構造では、desktop build 中に core が誤って `std` へ依存しても検出しにくい。`src/app.rs` には font discovery、filesystem、custom problem path など platform I/O が入り、typing core と app shell の境界も曖昧である。

## 目標境界

typing core:

- `core` と `alloc` のみを使う。
- parser、content model、typing state、romaji mapping、metrics、pure command transition を持つ。
- filesystem、clock、font、window、terminal、DOM、clipboard、firmware API を知らない。
- 入力は target-independent command として受け取り、出力は view-independent state / event として返す。

UI adapter:

- GUI、TUI、WASM/web、mobile、UEFI に分かれる。
- target 固有の input event を core command に変換する。
- core state を view model / render tree に変換する。
- storage、file picker、clipboard、font discovery、time source、logging を持つ。

shared UI layer:

- target-independent view model を定義する。
- small screen、keyboard-only、touch、firmware framebuffer の制約を表現する。
- target ごとの concrete rendering へ落とす前の共通 layout policy を持つ。

## crate / feature の候補

NEPLg2 は `nepl-core` を `#![no_std]` crate にし、`nepl-cli` と `nepl-web` が `nepl-core` に依存する構成を取っている。この依存方向は typingmp でも参考になる。typingmp では compiler core の代わりに typing core を置き、CLI / web の代わりに GUI / TUI / WASM / mobile / UEFI backend を置く。

短期:

- 現 crate 内で `core_engine` module を作り、`std` import を禁止する。
- `cargo check --no-default-features --features core-only` のような gate を作る。
- `core_engine` 配下で `alloc` を標準にし、`std` alias をやめる。

中期:

- `typingmp-core`: `#![no_std]` + `extern crate alloc`
- `typingmp-ui`: shared view model / layout policy
- `typingmp-backend-gui`
- `typingmp-backend-web`
- `typingmp-backend-tui`
- `typingmp-backend-mobile`
- `typingmp-backend-uefi`

## 修正方針

1. `parser`、`typing`、`model`、`layout_data` から `std` import をなくし、`alloc` / `core` に統一する。
2. `App` から filesystem、font discovery、custom problem path を adapter service として切り出す。
3. platform input を `CoreCommand` に変換し、typing core は command だけを見る。
4. `TypingSnapshot` / `UiSnapshot` を作り、backend は core mutable state を直接触らない。
5. CI に no_std core check を追加する。
6. problem source / asset / storage / logging は provider trait にし、core は host I/O を知らないようにする。

## 関連 issue

- `TP-ARCH-004`
- `TP-ARCH-005`
- `TP-ARCH-001`
- `TP-TYPE-001`
