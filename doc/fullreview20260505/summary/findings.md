# 総レビュー findings

対象 commit: `501bb73fe53a32bdbc100303498a9dc438ff85aa`

## 結論

`typingmp` は multi-backend の typing app として広い target を持っており、主要 feature の compile check は多くが通る。一方で、WASM debug build blocker、format failure、platform error handling、巨大 `App` / `ui.rs`、typing core の test gap が残っている。

project の目的は、no_std レベルでも成立する typing core と、target に合わせて段階的に具体化する UI を組み合わせることにある。現段階の修正は、大規模 rewrite ではなく、この二層境界を crate / feature / adapter として明示しながら、静的検証を clean にする順序がよい。

開発方針として後方互換は不要であり、不適切な既存設計は温存しない。特に状態や command は enum / newtype / `match` に寄せ、数値や文字列による convention を残さない。

## 重要 findings

### 1. WASM debug logger が build blocker になる

`WEBSOCKET_ADDRESS` 未設定で WASM check が失敗する。release workflow と build profile の前提がずれているため、最初に直すべきである。

### 2. `cargo fmt --check` が失敗する

format が揃っていない状態で refactor を始めると、意味のある差分と整形差分が混ざる。format-only commit を先に作るべきである。

### 3. no_std core と UI adapter の境界が弱い

現状は `uefi` feature 時に crate 全体を `no_std` 化しているが、typing core が常に `core + alloc` だけで成立することを検証する構造ではない。parser / typing / model / layout を core 側に寄せ、font discovery、file I/O、DOM、terminal、firmware を adapter 側に分ける必要がある。

### 4. I/O 仮想化が不足している

NEPLg2 の `SourceMap` / provider VFS / preopen の設計は、typingmp では problem source、font asset、storage、logger の provider 化として使える。現状は `App` と backend が `std::fs`、DOM、localStorage、font path を直接扱っており、no_std core 目標と相性が悪い。

### 5. raw number / raw string state が静的検査を弱めている

typing cursor、menu selection、UI action bridge、problem source badge に raw number / raw string が残る。enum と `match` の網羅性検査が効くように置き換えるべきである。

### 6. `App` / `ui.rs` が大きすぎる

state、scene、problem、scroll、layout、render view が密結合している。backend から触れる mutable surface を減らし、view snapshot を導入するべきである。

### 7. parser / typing core の diagnostic と test が不足している

parser は malformed input を structured diagnostic として返せない。typing input は high-risk だが regression test が不足している。

### 8. platform backend の `unwrap()` が多い

WASM、mobile、UEFI、TUI では環境依存の失敗が起きやすい。init / render / input の error path を `Result` と visible error に変える必要がある。

### 9. renderer と frame buffer 処理が重複している

backend ごとの render code が増え、性能改善の反映漏れが起きやすい。surface abstraction と shared renderer cache を導入するべきである。

### 10. docs / artifacts が実装とずれている

README の syntax、Cargo.toml コメント、生成物の repository 管理は、修正者と利用者の判断を誤らせる。

## issue 対応

今回の findings は `issues/index.md` と `issues/index.json` で管理する。最初の修正 wave は `P0` と `P1` を対象にする。

## 修正優先順位

1. `TP-CI-001`: WASM debug logger の env 依存をなくす。
2. `TP-ARCH-004`: no_std typing core と UI adapter の境界を固定する。
3. `TP-ARCH-005`: problem / asset / storage / logger の I/O provider 境界を作る。
4. `TP-STATIC-001`: raw number / raw string state を enum / newtype / typed command に置き換える。
5. `TP-CI-002`: format / clippy warning を段階的に clean にする。
6. `TP-CORE-003`: typing core と parser の regression test を追加する。
7. `TP-ARCH-001`: `App` の scene / problem / scroll / input 分離を始める。
8. `TP-ERR-001`: platform init と render error の `unwrap()` を減らす。
9. `TP-ARCH-002`: renderer surface を共通化する。
10. `TP-DOC-001` / `TP-DOC-002`: docs と artifacts を整理する。
