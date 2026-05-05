# platform review: TUI

対象 commit: `501bb73fe53a32bdbc100303498a9dc438ff85aa`

## 結論

TUI backend の最大リスクは terminal state の復旧性である。raw mode と alternate screen を明示的に cleanup しているが、途中 error や panic で cleanup に到達しない場合、端末が壊れた状態で残る。

## terminal guard

`src/tui.rs` は raw mode / alternate screen を有効化し、終了時に戻す。これは RAII guard で表すべき責務であり、現在のように末尾 cleanup に依存すると、`?` や panic の導入に弱い。

修正方針:

- `TerminalGuard` を導入し、`Drop` で raw mode と alternate screen を復旧する。
- setup failure 時は有効化済みの state だけ戻す。
- panic hook で最低限の復旧を試みる。

## unwrap と state assumption

typing scene で model / line / cursor を `unwrap()` する箇所がある。UI state が想定通りなら成立するが、custom problem parse failure や scene transition bug が入ると TUI 全体が落ちる。

修正方針:

- render 前に view snapshot を作り、missing state を error view に変換する。
- parser failure / empty problem は typing scene に入れない。

## polling

input polling は 100ms cadence で、typing application としては反応が鈍くなる可能性がある。CPU 使用率との tradeoff はあるが、render tick と input wait を分けるべきである。

## 関連 issue

- `TP-TUI-001`
- `TP-ERR-001`

## 実装進捗: T12

backend 共通の `BackendError` を導入し、TUI の font parse failure は typed error として `run()` から返すようにした。runtime font load failure は `App::report_visible_error()` 経由で画面 status へ流す。

typing scene の render path で `typing_model().unwrap()` していた箇所は、model がない場合に該当 frame を描画しない形へ変更した。terminal raw mode / alternate screen の RAII guard は T13 で実装する。
