# 参照レビュー: NEPLg2 の core / CLI / web 分離

対象 commit: `501bb73fe53a32bdbc100303498a9dc438ff85aa`

参照元: `C:\projects\NEPLg2_2`

## 結論

NEPLg2 の分離で typingmp に最も参考になるのは、core を独立 crate にして、CLI / web が core に依存する一方向の構造である。typingmp では、これを `typingmp-core`、shared UI model、target backend へ読み替えるのがよい。

## NEPLg2 の構成

NEPLg2 の workspace は次のように分かれている。

- `nepl-core`
  - `#![no_std]`
  - `extern crate alloc`
  - compiler pipeline、diagnostic、AST、parser、typecheck、codegen の中核
  - `loader` / `module_graph` など一部は `target_os != "none"` で gate
- `nepl-cli`
  - `std` 前提
  - filesystem、stdio、argument parsing、WASI host、artifact writer
  - `nepl-core` の compile/check API を呼ぶ
- `nepl-web`
  - wasm-bindgen facade
  - browser / JS 境界、diagnostic conversion、WAT utility
  - `nepl-core` を呼ぶ
- `web/`
  - TypeScript の editor / workspace / UI layer

## typingmp への対応

| NEPLg2 | typingmp での対応 |
|---|---|
| `nepl-core` | `typingmp-core`: parser、typing state、layout mapping、metrics |
| `nepl-cli` | TUI / desktop runner / dev tool: std I/O、filesystem、terminal、file picker |
| `nepl-web` | WASM facade: browser storage、DOM、canvas、clipboard、JS event mapping |
| `web/` | web UI shell: virtual keyboard、responsive layout、browser-specific interaction |
| `nepl-language` / `nepl-lsp` | 将来の問題 editor / lint / preview tool に相当 |

## 借りるべき設計

- core crate は常に `no_std` を宣言する。
- platform I/O は core API の外側に置く。
- core は stable data structure と diagnostic を返し、adapter が表示形式へ変換する。
- CLI / web / backend は core に依存するが、core は adapter を知らない。
- browser / filesystem / terminal / firmware などの host capability は adapter 側の service として渡す。
- source / file identity は raw host path ではなく、`SourceMap` のような stable ID と label に変換してから core に渡す。
- web は VFS、CLI は rooted/preopen path resolution のように、host ごとの I/O capability を adapter 境界で表現する。

## そのまま真似しない点

NEPLg2 の `nepl-core` は compiler なので、codegen や loader の都合で巨大 file が残っている。typingmp は typing game なので、core はもっと小さく保てる。`App` のような high-level state は core に入れず、typing session と rule engine に限定するべきである。

また、NEPLg2 の `nepl-web/src/lib.rs` や `nepl-cli/src/main.rs` は大きくなっている。typingmp では最初から backend facade、input adapter、storage adapter、render surface を分け、同じ肥大化を避けるべきである。

## 推奨構造

```text
typingmp-core
  parser
  content_model
  typing_state
  layout_mapping
  metrics
  diagnostics

typingmp-ui
  ui_snapshot
  layout_policy
  render_tree
  accessibility labels

typingmp-backend-gui
typingmp-backend-tui
typingmp-backend-web
typingmp-backend-mobile
typingmp-backend-uefi
```

## 関連 issue

- `TP-ARCH-004`
- `TP-ARCH-005`
- `TP-ARCH-001`
- `TP-ARCH-002`
