# typingmp 総レビュー 2026-05-05: 目次

対象 commit: `501bb73fe53a32bdbc100303498a9dc438ff85aa`

このディレクトリは、2026-05-05 時点の `typingmp` 全体レビューを記録する。`C:\projects\NEPLg2_2\doc\fullreview20260430` のように章ごとのレビューを残し、`C:\projects\NEPLg2_2\issues` のように findings を issue として追跡できる構成にする。

## 0. レビュー運用

- `README.md`: レビュー基準、対象 commit、成果物の読み方
- `index.md`: 本目次、レビュー順序、作成ファイル一覧
- `meta/review-method.md`: 調査方法、実行した確認コマンド、判断基準
- `meta/review-validity.md`: レビュー完了後の妥当性確認、見落としリスク
- `issues/README.md`: issue 管理スキーマ、status / priority / type の説明
- `issues/index.md`: 人間向け issue 一覧
- `issues/index.json`: 機械処理向け issue 一覧
- `issues/items/*.md`: 個別 issue

## 1. プロジェクト状態とリスク

- `project/verification-status.md`
  - local で実行した確認コマンド
  - 成功した対象、失敗した対象、警告の傾向
  - CI / release workflow に対する懸念
- `project/risk-map.md`
  - 修正優先順位
  - マルチプラットフォーム対応の blocker
  - 設計負債、検証不足、ドキュメント不整合

## 2. アーキテクチャとモジュール分割

- `architecture/core-ui-boundary.md`
  - no_std レベルの typing core と target-specific UI の境界
  - crate / feature / adapter 分割
  - alloc 前提、std 前提、platform I/O の切り分け
- `architecture/neplg2-reference.md`
  - NEPLg2 の `nepl-core` / `nepl-cli` / `nepl-web` 分離から借りられる設計
  - typingmp へ適用する場合の対応関係と注意点
- `architecture/io-virtualization.md`
  - NEPLg2 の loader / SourceMap / VFS / preopen 設計から借りる I/O 仮想化
  - problem source、font asset、storage、logger の provider 化
- `architecture/modularization.md`
  - `src/app.rs` の god object 化
  - UI / rendering / backend integration の責務分離
  - public mutable state と private type warning
- `architecture/rendering.md`
  - GUI / WASM / mobile / UEFI の描画重複
  - text measurement / gradient / rasterization の性能
  - backend 共通化の方針

## 3. コアロジック

- `core/parser.md`
  - 問題文 parser の仕様、異常系、diagnostic 化
  - README / `doc/ntq-format.md` / examples の記法整合
- `core/typing-model.md`
  - 入力処理、cursor/index 型、model mutation、logging
  - typing metrics と correctness state
- `core/model-layout.md`
  - `TypingModel` / `Layout` / key mapping の型安全性
  - static data 化、lookup 構造、allocation 削減

## 4. プラットフォーム別レビュー

- `platforms/gui.md`
  - winit / pixels backend
  - event loop、unused parameter、render API
- `platforms/tui.md`
  - terminal raw mode、alternate screen、unwrap、polling
  - TUI 固有の性能と復旧性
- `platforms/wasm-web.md`
  - WASM debug logger、DOM unwrap、storage、mobile keyboard JS
  - `index.html` の clipboard 書き込み
- `platforms/mobile.md`
  - Slint / Android entrypoint、`Arc<Mutex<App>>`、callback error handling
- `platforms/uefi.md`
  - firmware API unwrap、framebuffer allocation、time handling

## 5. 品質、防壁、保守性

- `quality/static-validation.md`
  - `cargo fmt` / `clippy` / feature matrix / target check
  - warning を CI gate にする方針
- `quality/static-safety.md`
  - enum / match による状態表現
  - 型安全・メモリ安全を検査可能にする設計基準
  - raw number / raw string state の撤廃方針
- `quality/tests.md`
  - 現状 test coverage
  - parser 以外の regression test 不足
  - platform smoke test 方針
- `quality/docs-assets.md`
  - README と実装のずれ
  - generated artifact、開発用ログ、不要生成物

## 6. セキュリティとツール

- `security/dev-tools.md`
  - `serve.js` path traversal guard
  - `logger_server.js` の dev-only 境界
  - UEFI 実行 script の破壊的操作と安全確認
- `security/web-behavior.md`
  - clipboard write
  - localStorage / custom problem import
  - web UI での失敗表示

## 7. 最終まとめ

- `summary/findings.md`
  - 重要 findings
  - issue 一覧への対応
  - 修正優先順位
- `summary/refactor-plan.md`
  - refactoring の推奨順序
  - 小さく安全に進めるための commit 分割
  - 各段階の検証コマンド

## 8. 今回の issue 管理

今回のレビューで抽出した修正対象は `issues/items/` に個別 issue として記録する。各 issue は YAML front matter を持ち、`issues/index.md` と `issues/index.json` から参照する。

優先度の意味:

- `P0`: 現在の build / release / 実行を直接壊す、または重大な安全上の問題
- `P1`: 実装品質、クラッシュ耐性、設計分割、platform 対応の主要 blocker
- `P2`: 保守性、性能、検証、防壁の改善
- `P3`: ドキュメント、整理、将来の改善

## 9. レビュー観点の対応

| 観点 | 主なレビュー文書 |
|---|---|
| マルチプラットフォーム対応 | `platforms/*.md`, `project/verification-status.md` |
| no_std レベルの typing core | `architecture/core-ui-boundary.md`, `core/*.md` |
| モジュール化 / ファイル分割 | `architecture/modularization.md`, `architecture/rendering.md` |
| 計算効率 / パフォーマンス | `architecture/rendering.md`, `core/model-layout.md`, `platforms/uefi.md` |
| 可読性 / 関数分割 | `architecture/modularization.md`, `core/typing-model.md` |
| 型安全性 | `core/typing-model.md`, `core/model-layout.md`, `quality/static-validation.md` |
| 静的検証の活用 | `quality/static-validation.md`, `quality/tests.md` |
| エラーハンドリング | `platforms/*.md`, `security/dev-tools.md` |
| ドキュメント整合 | `quality/docs-assets.md`, `core/parser.md` |
