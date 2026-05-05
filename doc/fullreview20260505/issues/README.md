# typingmp review issues

このディレクトリは、`doc/fullreview20260505` の findings を issue として管理する。

## 構成

- `README.md`: issue 管理ルール
- `index.md`: 人間向け一覧
- `index.json`: 機械処理向け一覧
- `items/*.md`: 個別 issue

## status

- `open`: 未対応
- `fixed`: 修正済みだが検証待ち
- `verified`: 修正と検証が完了

## priority

- `P0`: build / release / 実行を直接壊す blocker
- `P1`: no_std core、multi-platform、クラッシュ耐性、設計分割の主要 blocker
- `P2`: 保守性、性能、検証、防壁の改善
- `P3`: docs / comment / cleanup

## type

- `bug`: 具体的な不具合
- `architecture`: 依存方向、責務境界、module 分割
- `performance`: allocation、再計算、frame time
- `quality`: format、lint、CI、保守性
- `test`: test coverage
- `type-safety`: 型で invariant を守るための改善
- `security`: dev tool / web behavior / path guard
- `docs`: documentation / artifact policy
- `build`: build script / workflow / local script
- `ux`: user-visible behavior

## 更新ルール

修正時は個別 issue の `status`、`resolved`、`updated` を更新し、`issues/index.md` と `issues/index.json` も同じ状態に揃える。設計変更が入った場合は、対応する `doc/fullreview20260505/*` の章別レビューも更新する。
