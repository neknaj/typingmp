# リスクマップ

対象 commit: `501bb73fe53a32bdbc100303498a9dc438ff85aa`

## 最重要リスク

### no_std core と UI adapter の境界が crate 構造に表れていない

project の目標は、どこでも動く typing core と、target に合わせて段階的に具体化する UI を組み合わせることである。現状は `uefi` feature の時だけ crate 全体を `no_std` にする構造で、typing core が `std` 非依存であることを常時検証できていない。

判断:

- parser / typing / model / layout は `core + alloc` crate として切り出す候補である。
- `App` の font discovery、file I/O、custom problem path、renderer backend は core から外す。
- CI には `no_std` core 単体の build / test target を追加する。

### raw number / raw string state が静的検査を弱めている

typing cursor や metrics は `i32` を多用し、UI command bridge には文字列 command が残る。menu selection も index と定数に依存している。これは `enum` と `match` による網羅性検査を効かせる開発方針と合っていない。

判断:

- core state は newtype / enum で表す。
- UI command は string boundary で受けた後、すぐ enum に parse する。
- enum 追加時に compile error で修正漏れが分かる構造にする。

### WASM build と release workflow がずれている

WASM debug logger は build-time env を要求するが、release workflow 側ではその env が設定されていない。`wasm-pack build` が release 相当で動く前提に依存しているため、build mode や CI runner の変更で破綻しやすい。

判断:

- `WEBSOCKET_ADDRESS` 未設定でも compile できる必要がある。
- debug log transport は wasm runtime 設定、feature flag、または optional init に切り出す。
- workflow は target / feature / profile を明示する。

### `App` が状態、入力、scroll、scene、problem、font を抱えすぎている

`src/app.rs` は 1000 行を超え、`App` public field と巨大 `impl` が中心になっている。typing state と view state と backend interaction が同じ型に集まっているため、bug 修正時の影響範囲が読みにくい。

判断:

- まず scene state、problem repository、scroll/cache、font/layout を分ける。
- backend から見える API を narrow interface にする。
- public mutable field を減らし、state transition は method 経由にする。

### backend ごとの error handling が弱い

WASM / mobile / UEFI / TUI は、それぞれ環境依存 API を直接 `unwrap()` する箇所が多い。desktop では問題が表面化しなくても、ブラウザ、firmware、mobile runtime では crash や復旧不能状態になりやすい。

判断:

- platform init は `Result` を返し、UI に失敗を表示する。
- TUI raw mode / alternate screen は RAII guard にする。
- UEFI は firmware call failure と allocation failure を明示的に扱う。

### test coverage が parser に偏っている

`cargo test --no-default-features` では parser test が中心で、typing input、cursor transition、romaji normalization、scroll calculation、custom problem loading、backend smoke が不足している。

判断:

- parser test を維持しつつ、typing model の pure test を先に追加する。
- backend は full UI test ではなく、init / render smoke / error path を分けて作る。
- custom problem import と README examples を fixture 化する。

## 技術的負債として残してはいけないもの

- `i32` cursor と unchecked cast を状態表現の標準にすること。
- malformed problem text を diagnostic なしで通常 content として扱うこと。
- platform backend ごとに rendering pipeline を複製し続けること。
- dev server や logger を production と区別せずに配布すること。
- generated artifact を source review 対象と同じ場所で管理し続けること。

## 優先順位

1. WASM build blocker と release workflow の整合を直す。
2. no_std typing core と target UI adapter の境界を crate / feature として固定する。
3. raw number / raw string state を enum / newtype / typed command に置き換える。
4. `cargo fmt` と主要 clippy warning を解消し、静的検証を CI gate 化する。
5. parser / typing model / layout の pure core test を追加する。
6. `App` の責務を scene、problem、scroll、render input に分ける。
7. backend error handling を `Result` / guard / fallback UI に寄せる。
8. renderer 共通化と frame allocation 削減を進める。
9. docs / examples / artifacts を整理する。

## 関連 issue

- `TP-CI-001`
- `TP-ARCH-004`
- `TP-STATIC-001`
- `TP-ARCH-001`
- `TP-ERR-001`
- `TP-CORE-003`
- `TP-DOC-002`
