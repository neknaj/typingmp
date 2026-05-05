# コアレビュー: model と layout data

対象 commit: `501bb73fe53a32bdbc100303498a9dc438ff85aa`

## 結論

`TypingModel` と `Layout` は動作の中心だが、public mutable data と runtime 構築が多い。layout mapping はほぼ static data なので、session ごとに作り直す必要は薄い。lookup 構造を整理すると、入力処理と test が単純になる。

## public mutable model

`src/model.rs` の model は public field が多く、invariant を外部から壊せる。typing state の正しさは cursor、line、candidate、metrics の整合に依存するため、field を直接公開するほど regression が増える。

修正方針:

- state を private field に寄せる。
- update は command method 経由にする。
- UI には read-only snapshot を返す。

## layout rebuild

`Layout::default()` は mapping を毎回構築する。入力 key mapping は実行時に頻繁に変わるものではないため、`OnceLock` / `LazyLock` / generated static table の候補になる。

修正方針:

- static mapping と user configurable mapping を分ける。
- default mapping は lazy static にする。
- first char lookup は `Vec<Vec<usize>>` ではなく、key space を明示した map / trie を検討する。

## lookup 設計

`normalized_mapping_by_first_char` は normalized romaji の first byte 前提で動く。現状の data では成立していても、将来 key sequence や kana direct input を拡張すると壊れやすい。

修正方針:

- romaji sequence lookup を trie 化する。
- normalized byte と Unicode char の境界を明示する。
- test で mapping collision と prefix handling を固定する。

## 関連 issue

- `TP-TYPE-001`
- `TP-PERF-001`
