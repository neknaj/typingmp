# WASI/WASIX TUI 調査と設計 2026-05-07

## 調査結果

- Rust toolchain には `wasm32-wasip1` と `wasm32-wasip2` が存在する。ローカル環境にも `rustup target add wasm32-wasip1 wasm32-wasip2` で追加済み。
- `wasm32-wasip2` は WASI Preview 2 と Component Model を使う target であり、runtime 側も Preview 2 component を実行できる必要がある。Rust 公式は cfg として `all(target_os = "wasi", target_env = "p2")` を推奨している。
- Wasmtime 44.0.1、Wasmer 7.1.0、cargo-wasix 0.1.26 を Windows 環境へ導入済み。現在の shell では PATH に `C:\Program Files\Wasmtime\bin` と `C:\Program Files (x86)\Wasmer\bin` を追加すると `wasmtime --version` / `wasmer --version` が通る。
- 環境構築は `scripts/setup_wasm_wasi_env.ps1` で再実行できる。Rust target、`wasm-pack`、`cargo-wasix`、Wasmtime、Wasmer を確認し、既知の runtime install path を現在 shell の PATH に追加する。
- 既存 `tui` feature は `crossterm` に依存している。`cargo check --no-default-features --features tui --target wasm32-wasip1/wasip2` は `crossterm` 側で `raw_mode`、terminal size、event source が `unix/windows` cfg に隠れて失敗する。
- 既存 `tui.rs` は terminal raw mode / alternate screen / key event / terminal size / color flush と、`ui::build_ui` から cell buffer を作る処理が同じ module に混在している。
- `target_arch = "wasm32"` を browser wasm とみなす cfg が複数ある。WASI/WASIX も `wasm32` なので、browser 固有の IME/localStorage/open-file 判定は `feature = "wasm"` に寄せる必要がある。
- `timestamp::now` は `target_arch = "wasm32"` で常に `js_sys::Date` を使うため、WASI/WASIX では不適切である。WASI は `std::time` を使う。
- smoke 実行では Wasmtime 44.0.1 が `wasm32-wasip1` と `wasm32-wasip2` の両方を実行できた。Wasmer 7.1.0 は `wasm32-wasip1` を実行できたが、`wasm32-wasip2` は Component Model validation が無効で実行できなかった。
- `cargo wasix check --no-default-features --features wasi-tui --bin rust_multibackend_app` は通る。直接の `cargo wasix build` は現行の単一 crate が browser wasm 用 `cdylib` crate-type を持つため、WASIX toolchain 側で cdylib link を試みて `scrt1.o` 不足で失敗する。現段階の実行 artifact は標準 `wasm32-wasip1` を Wasmer/WASIX runtime で動かす。

参照:

- [Rust `wasm32-wasip2` target](https://doc.rust-lang.org/stable/rustc/platform-support/wasm32-wasip2.html)
- [Wasmtime CLI install](https://docs.wasmtime.dev/cli-install.html)
- [Wasmtime CLI run](https://docs.wasmtime.dev/cli-options.html)
- [Wasmer run CLI](https://docs.wasmer.io/runtime/cli)
- [Wasmer WASIX runner](https://docs.wasmer.io/runtime/runners/wasix)
- [WASIX Rust installation](https://wasix.org/docs/language-guide/rust/installation)
- [WASIX Rust usage](https://wasix.org/docs/language-guide/rust/usage)

## 適切な挙動

- native TUI は従来どおり `crossterm` で raw key input と差分描画を行う。
- WASI/WASIX TUI は `crossterm` を使わず、ANSI escape sequence と標準入出力で動く portable backend とする。
- WASI portable baseline は raw keyboard event を前提にしない。runtime ごとの TTY 拡張差を避けるため、初期実装は line-buffered command input を使う。
- WASIX は Wasmer の拡張 ABI だが、Rust stable の標準 target ではない。まず `wasm32-wasip1` artifact を Wasmer で動かす経路を WASIX runtime 互換として扱い、WASIX 固有 raw TTY は adapter 差し替えで追加できる設計にする。
- browser wasm (`feature = "wasm"`) と WASI/WASIX (`feature = "wasi-tui"`) は同じ `wasm32` でも明確に別 backend として扱う。

## 設計

### Feature と target

- `wasm`: browser canvas/DOM backend。target は `wasm32-unknown-unknown`。
- `tui`: native terminal backend。`crossterm` を使う。target は native OS のみ。
- `wasi-tui`: WASI/WASIX compatible ANSI TUI backend。`crossterm` には依存しない。target は `wasm32-wasip1` と `wasm32-wasip2`。
- `cargo-wasix`: WASIX toolchain の静的検査経路。現行 release artifact は `wasm32-wasip1` を Wasmer で実行する。

### Backend 分離

WASI TUI は次の責務を分ける。

- `wasi_tui` adapter: stdin/stdout、ANSI clear/draw、line command parse、固定または環境変数由来の terminal size。
- shared app/ui/model: 既存の `App`、`ui::build_ui`、font script routing を再利用する。
- font asset: browser wasm ではなく CLI 実行なので、まずは `include_bytes!("../fonts/...")` による bundled font provider を使う。WASI preopen directory に依存しないため、Wasmtime/Wasmer で同じ wasm が動く。WASI/WASIX では native desktop の system font picker は出さない。

### 入力仕様

portable baseline の line commands:

- empty line: `Enter`
- `/q`: quit
- `/esc`: escape
- `/tab`: TUI mode cycle
- `/up`, `/down`: menu/source/settings navigation
- `/bs`: backspace
- その他の文字列: 文字単位で `AppEvent::Char` として投入する

raw key input が必要になった場合は、`wasi_tui` の input 部分だけを WASIX TTY adapter に差し替える。app/ui/rendering には影響させない。

### 表示仕様

- ANSI backend は各 frame を full redraw する。WASI の portable stdout では差分描画よりも backend 互換性を優先する。
- terminal size は `COLUMNS` / `LINES` env を優先し、未指定なら `100x30` とする。
- portable WASI backend は互換性を優先し、SimpleText 相当の cell output を描画する。`/tab` は app state との互換のため受け付けるが、native TUI の Braille / ASCII art 描画は `crossterm` backend 側の機能として残す。

### CI と成果物

- feature matrix に `wasi-tui-wasip1-check` と `wasi-tui-wasip2-check` を追加する。
- push build では `wasm32-wasip1` と `wasm32-wasip2` の release artifact を追加する。Wasmer/WASIX 実行互換の実体としては `wasip1` module を使う。
- `wasmtime` による smoke test は非対話 command を stdin へ渡す形にする。

## 実施順

1. 既存の `target_arch = "wasm32"` browser 固有 cfg を `feature = "wasm"` に修正する。
2. `wasi-tui` feature と `src/wasi_tui.rs` を追加する。
3. `timestamp::now` を browser wasm / WASI / native / UEFI で分岐する。
4. `cargo check` を native, browser wasm, `wasm32-wasip1`, `wasm32-wasip2` で通す。
5. Wasmtime/Wasmer で smoke run する。
6. CI matrix と release artifact を更新する。
