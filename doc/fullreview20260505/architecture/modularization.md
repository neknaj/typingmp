# アーキテクチャレビュー: モジュール化

対象 commit: `501bb73fe53a32bdbc100303498a9dc438ff85aa`

## 結論

現在の中心的な設計負債は、`src/app.rs` と `src/ui.rs` が状態管理、入力処理、問題管理、scroll、metrics、render data 構築をまとめて抱えている点である。feature ごとの build は概ね通るが、修正時に platform へ影響する境界が不明瞭になっている。

## `App` の責務過多

`src/app.rs` の `App` は public field を多く持ち、`impl App` は scene transition、problem loading、font loading、typing update、scroll cache、render input の生成まで扱う。

問題:

- state invariant を型で守れず、backend から field を直接変更できる。
- typing state と view state が同じ object にあるため pure test を作りにくい。
- `ScrollCache` などの private / pub(crate) type が public field に露出して warning になる。
- custom problem、example problem、runtime state が同じ file に混在している。

修正方針:

- `app/state.rs`: scene と high-level state transition
- `app/problems.rs`: builtin / custom problem repository
- `app/scroll.rs`: scroll cache、cursor visibility、layout-independent calculation
- `app/input.rs`: key input と command dispatch
- `app/view_model.rs`: UI / renderer へ渡す immutable snapshot

初期段階では file 分割だけを目的にせず、backend が触ってよい API を狭めることを優先する。

## `ui.rs` の責務過多

`src/ui.rs` は rendering のための data tree 構築、status panel、typing text、progress、help content、scroll segment を同時に扱う。`build_typing_ui` は長く、layout 計算と correctness 表示が同じ関数にある。

問題:

- correctness segment の仕様変更が layout 計算へ波及する。
- status 行や score 表示の変更でも typing text 周辺に触る必要がある。
- unused variable / useless vec の warning が出ており、細部の追跡が難しい。

修正方針:

- `ui/typing.rs`: typing text view model
- `ui/status.rs`: metrics / progress / mode indicator
- `ui/help.rs`: help content
- `ui/layout.rs`: viewport と spacing calculation
- view model は `App` の mutable state を直接読まず、snapshot を受け取る。

## public field と private type

`App::scroll_cache` は public field だが、`ScrollCache` は `pub(crate)` である。`ScrollCacheState::cursor_state` も private type を含む。この warning は単なる lint ではなく、API 境界が曖昧なことを示す。

修正方針:

- field を private 化する。
- backend が必要な情報は query method または view snapshot で返す。
- internal cache は `App` 外から見えないようにする。

## 関連 issue

- `TP-ARCH-001`
- `TP-ARCH-003`
- `TP-CORE-002`

## 実装進捗: T11

`src/app/view.rs` に `AppSnapshot` を追加し、menu / settings / problem source / status / fps などの描画入力を immutable snapshot として読める境界にした。GUI / TUI / UEFI / WASM の一部状態参照は snapshot / query method / drain method 経由へ寄せた。

`src/app/problems.rs` に problem repository 操作を移し、builtin / custom / open-file の選択と表示 label 管理を `app.rs` 本体から分離した。`src/app/scroll.rs` には scroll cache、line origin、cursor position 計算を移し、render cache の型は app module 内部に閉じた。

`App` の mutable field は `pub(crate)` に落とし、外部 API は `AppSnapshot`、`typing_model()`、`result_model()`、`take_file_open_request()` などの method へ整理した。これにより private-interface warning は消え、以降の backend error contract / renderer surface 分離で `App` internals に触る範囲を狭められる。
