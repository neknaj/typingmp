# コアレビュー: typing model

対象 commit: `501bb73fe53a32bdbc100303498a9dc438ff85aa`

## 結論

typing logic は core として分離されているが、model を値で受け渡す設計、`i32` index、debug log、recursive input path が混在している。入力処理は最も regression が出やすい領域なので、pure test と型の強化を先に入れるべきである。

## model mutation

`key_input(mut model: TypingModel, ...)` は model を値で受け取り、caller 側で clone / move が発生しやすい。typing state は頻繁に更新されるため、allocation と copy の観点でも不利である。

修正方針:

- `fn key_input(model: &mut TypingModel, key: KeyInput) -> TypingResult` へ寄せる。
- state transition の結果だけを返す。
- undo / replay が必要な場合は snapshot を別 layer で管理する。

## cursor と index

`TypingModel` 周辺は `i32` cursor を多用する。Rust の slice / vec index は `usize` であり、負値や cast 境界を型で表現できていない。

修正方針:

- `LineIndex`、`CharIndex`、`CandidateIndex` の newtype を導入する。
- user-visible count と internal index を分ける。
- boundary check は constructor または transition method に閉じ込める。

## logging

`src/typing.rs` の key input logging は broad cfg で stdout / console に出る。typing input は頻度が高く、production-like build では性能と privacy の両面で不適切である。

修正方針:

- `tracing` 相当の optional feature に切り出す。
- default build では入力文字を出さない。
- debug UI が必要なら platform ごとに明示的に opt-in する。

## test gap

現状の test は parser に偏っている。typing core には、`n` auto-commit、romaji candidate selection、backspace、line completion、metrics、miss count、IME-like input の regression test が必要である。

## 関連 issue

- `TP-CORE-002`
- `TP-TYPE-001`
- `TP-CORE-003`
