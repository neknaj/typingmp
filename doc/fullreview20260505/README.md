# typingmp 総レビュー 2026-05-05

このディレクトリは、`typingmp` の現行実装に対する総レビューである。単なる現状列挙ではなく、今後のリファクタリングとバグ修正を issue として進められるように、章別レビューと issue tracker を併置する。

## 基準

- 対象 commit: `501bb73fe53a32bdbc100303498a9dc438ff85aa`
- review date: 2026-05-05
- review scope: Rust core、parser、typing model、renderer、GUI、TUI、WASM/web、mobile、UEFI、build/CI、tooling、docs/assets
- 参照形式:
  - `C:\projects\NEPLg2_2\doc\fullreview20260430`: 章別レビュー構成
  - `C:\projects\NEPLg2_2\issues`: issue front matter / index 管理

## レビュー方針

- 技術的負債は残さない。後方互換維持のために不適切な既存設計を温存しない。
- 暫定実装は許容しても、暫定の雑設計は許容しない。設計ミスが分かった場合は再設計・再実装する。
- 静的検査の正確性を必須条件にする。型安全とメモリ安全は、実行時の慣習ではなく型と検査で守る。
- 有限状態は数値や文字列ではなく enum で表し、分岐は `match` の網羅性検査が効く形にする。
- typing game の中核は `no_std` レベルでも成立する core として扱う。
- UI は shared view model から target ごとの backend へ段階的に具体化する adapter として扱う。
- platform ごとの差異を隠さず、build target / feature ごとの失敗と警告を分ける。
- 暫定実装と設計上の責務混在を分けて記録する。
- user input、rendering、state transition、storage、file/network/dev tool の失敗経路を確認する。
- 型安全性を弱める public mutable field、`i32` cursor、unchecked cast、`unwrap()` を優先的に抽出する。
- `cargo fmt`、`clippy`、feature matrix、target check、test coverage を修正防壁として扱う。

## 成果物

- [index.md](./index.md): レビュー目次、作成ファイル一覧
- [meta/](./meta/): 調査方法、レビュー妥当性
- [project/](./project/): 検証状況、リスクマップ
- [architecture/](./architecture/): モジュール分割、描画責務
- [core/](./core/): parser、typing model、layout/model
- [platforms/](./platforms/): GUI、TUI、WASM/web、mobile、UEFI
- [quality/](./quality/): static validation、tests、docs/assets
- [security/](./security/): dev tool、web behavior
- [summary/](./summary/): findings と refactor plan
- [issues/](./issues/): issue tracker
