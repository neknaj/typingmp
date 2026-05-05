# platform review: WASM / web

対象 commit: `501bb73fe53a32bdbc100303498a9dc438ff85aa`

## 結論

WASM/web は機能量が多い一方で、build-time env、DOM unwrap、storage failure、JS keyboard loop、clipboard 書き込みが混在している。platform として最も利用者に近いため、失敗表示と opt-in 境界を明確にする必要がある。

## build-time env

`src/wasm_debug_logger.rs` は debug build で `WEBSOCKET_ADDRESS` を要求する。CI / release build と local debug の境界が曖昧であり、未設定なら compile 自体が失敗する。

修正方針:

- `option_env!` を使い、未設定時は logger disabled にする。
- debug websocket logger は feature flag に切り出す。
- workflow に明示的な WASM profile check を追加する。

## DOM / canvas unwrap

`src/wasm.rs` は `window`、`document`、canvas、2D context、`ImageData`、RAF などを `unwrap()` する箇所が多い。WASM は host page の構造に依存するため、missing DOM が crash ではなく error message になるべきである。

修正方針:

- init を `Result<(), JsValue>` に分ける。
- canvas / input / keyboard element が見つからない場合は visible error を出す。
- localStorage parse failure は silent fallback ではなく user-facing diagnostic にする。

## clipboard

`index.html` は初回 pointerdown で `navigator.clipboard.writeText('Neknaj Typing MP wasm edition')` を実行する。ユーザー操作を契機にしていても、typing app が clipboard を勝手に変更するのは不適切である。

修正方針:

- 自動 clipboard 書き込みを削除する。
- copy 機能が必要なら明示的な button と説明を持たせる。

## mobile keyboard JS

`index.html` は mobile keyboard 表示や flick input を JS 側で管理する。Rust 側の input handling と JS 側の virtual keyboard の責務が分散しているため、入力 regression test を作りにくい。

修正方針:

- JS は raw input event を Rust に渡すだけに寄せる。
- keyboard layout data は Rust / generated data / JS のどれを正とするか決める。

## 関連 issue

- `TP-CI-001`
- `TP-WASM-001`
- `TP-WEB-001`
- `TP-SEC-002`

## 実装進捗: T12

`src/backend.rs` に backend 共通の `BackendError` / `BackendErrorKind` を追加した。WASM startup は `window`、`document`、`body`、canvas 2D context、初回 resize callback、`ImageData`、RAF の失敗を `JsValue` / console-visible error に変換し、host page 欠落時に `unwrap()` で落ちない形へ寄せた。

localStorage の user-facing diagnostic と clipboard side effect は T14 の対象として残す。
