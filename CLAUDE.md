# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Neknaj Typing MP** is a Japanese Romaji typing practice application written in Rust that runs on five platforms from a single shared codebase: desktop GUI, terminal (TUI), WebAssembly (browser), Android (Slint), and bare-metal UEFI.

## Build & Run Commands

```bash
# Desktop GUI
cargo run --release --features gui

# Terminal (TUI) — press Tab to cycle rendering modes
cargo run --release --features tui

# WebAssembly（ビルド後は必ずローカルサーバー経由でテスト）
wasm-pack build --target web --release -- --features wasm
node serve.js          # http://localhost:8080 をブラウザで開く
# ※ file:// では WASM・FileReader・localStorage が正常に動作しないため直接開かないこと

# WASM with debug logging
WEBSOCKET_ADDRESS="ws://localhost:8081" wasm-pack build --target web --dev -- --features wasm
node logger_server.js  # WebSocket debug receiver
node serve.js          # 別ターミナルで起動

# Mobile (Slint) — デスクトップ上でモバイルUIをテスト
cargo run --release --features mobile

# Android (Slint + cargo-apk)
# 事前準備: cargo install cargo-apk、Android NDK セットアップ
# Cargo.toml の slint 依存に features = ["backend-android-activity"] を追加してから:
# cargo apk build --features mobile
# cargo apk run --features mobile

# UEFI (bare-metal via QEMU, run from PowerShell)
./run_uefi.ps1
```

## Architecture

The codebase uses Rust feature flags (`gui`, `tui`, `wasm`, `uefi`) to compile different backends from shared core logic:

```
Backends (gui.rs / tui.rs / wasm.rs / mobile.rs / uefi.rs)
    └── UI Layer (ui.rs) — builds platform-independent Renderable items
        └── Renderers (renderer.rs) — gui_renderer (pixels) / tui_renderer (text)
            └── Core Logic — app.rs, typing.rs, parser.rs, model.rs, layout_data.rs
Mobile Slint UI definition: ui/mobile.slint (compiled by slint-build in build.rs)
```

**Core modules** (platform-agnostic):
- `app.rs` — `AppState` enum and `App` struct, scene transitions, event dispatch
- `typing.rs` — keystroke validation, Romaji input logic, metrics
- `parser.rs` — parses `.ntq` problem files into `Content` model
- `model.rs` — data structures: `Content`, `Word`, `Segment`, `TypingModel`, `ResultModel`
- `layout_data.rs` — PHF-based Romaji→Kana lookup table
- `ui.rs` — builds `Renderable` items from app state; all layout logic lives here

**Platform backends** (feature-gated):
- `gui.rs` — minifb window, pixel buffer, font rendering via ab_glyph; mouse click/scroll wheel support
- `tui.rs` — crossterm terminal with three modes: SimpleText, AsciiArt, Braille
- `wasm.rs` — HTML canvas rendering, IME via hidden `<input>`, web-sys bindings; touch/swipe/flick keyboard
- `mobile.rs` — Slint window, pixel-buffer-to-Image bridge, flick keyboard via `ui/mobile.slint`; Android entry via `android_main`
- `uefi.rs` — bare-metal UEFI graphics and keyboard, no std allocator

**Build infrastructure:**
- `build.rs` — auto-discovers `*.ntq` files in `examples/` at compile time; generates `problem_files.rs` with a static file list and `get_problem_content()` lookup

## Problem File Format (.ntq)

Located in `examples/`. Full spec in `doc/ntq-format.md`.

- First line: `#title <title>`
- **Ruby**: `[base/reading]` — user types `reading` (e.g. `[秋/あき]`)
- **Anno**: `{inner/annotation}` — user types inner content, annotation shown as hint below
- Plain hiragana/katakana text inline
- Word boundaries: space or `/`
- Compound words joined by `-` (e.g. `[悲/かな]-しき`)
- Backslash escaping for special characters (`\[`, `\/`, `\-`, etc.)

## Code Style Rules (from src/LLM.md)

- **No change-marker comments** — Never add comments like `// ここから変更` or `// changed`
- **Comments describe what/why**, not where a change was made — explain purpose and flow abstractly
- **No unnecessary changes** — don't touch formatting, style, or code outside the required fix
- **Don't silently remove existing features**
- When providing modified source, supply the **complete file** per changed file in a code block
