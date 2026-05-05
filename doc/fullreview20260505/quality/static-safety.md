# 品質レビュー: 静的安全性

対象 commit: `501bb73fe53a32bdbc100303498a9dc438ff85aa`

## 結論

開発方針上、型安全とメモリ安全は必達であり、検査が効く構造で実装する必要がある。現状の Rust safe code は大部分でメモリ安全の土台を持つが、typing state、UI command、menu selection、source kind には raw number / raw string / index driven state が残っている。

これらは runtime convention ではなく、enum、newtype、`Result`、`match` の網羅性検査で表すべきである。後方互換は不要なので、既存 API や記法を保つために不適切な状態表現を残してはいけない。

## 確認した傾向

- `TypingStatus` や metrics に `i32` cursor / count が多い。
- `App` の menu selection は `usize` index と `MENU_ITEM_COUNT` に依存している。
- mobile callback は `action.as_str()` で command を分岐している。
- problem source badge は `"B"` / `"W"` / `"F"` の raw string を返す。
- parser failure は typed diagnostic ではなく通常 content に混ざりやすい。
- UEFI には `unsafe` event creation があるが、safety boundary の説明と失敗処理が薄い。

## 必要な設計

### enum / match

有限状態は enum にする。

- scene
- menu item
- settings item
- problem source kind
- UI command
- input command
- parse diagnostic code
- backend capability

enum 分岐は `match` で書き、将来 variant を追加した時に compile error で修正漏れが見えるようにする。`_` arm は、将来 variant を握りつぶす場合には使わない。

### newtype

数値 index は意味ごとに分ける。

- `LineIndex`
- `WordIndex`
- `SegmentIndex`
- `CharIndex`
- `MenuIndex`
- `ProblemId`
- `ProblemSourceId`

`usize` は collection access の直前だけで使い、状態として保持する時は newtype にする。

### Result / diagnostic

異常系は `Option` や silent fallback で潰さない。

- parser は `ParseDiagnostics` を返す。
- storage import は schema / size / parse failure を分ける。
- backend init は `Result` を返す。
- provider missing asset は typed error にする。

### unsafe boundary

`unsafe` が必要な platform API は、次を同じ場所に置く。

- なぜ `unsafe` が必要か。
- 呼び出し前提。
- failure をどう扱うか。
- safe wrapper の戻り値。

## 関連 issue

- `TP-STATIC-001`
- `TP-TYPE-001`
- `TP-CORE-001`
- `TP-ARCH-005`
