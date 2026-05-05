# レビュー方法

対象 commit: `501bb73fe53a32bdbc100303498a9dc438ff85aa`

## 目的

このレビューは、現行実装の bug と不適切なコードを修正可能な issue に分解するために行った。単に compile 可否を見るのではなく、次を確認対象にした。

- feature / target ごとの buildability
- runtime crash につながる `unwrap()` / `expect()` / unchecked cast
- state machine、rendering、platform backend の責務分離
- parser / typing logic / model cursor の型安全性
- docs / examples / implementation の整合
- CI、format、lint、test coverage の防壁

## 参照した外部構成

- `C:\projects\NEPLg2_2\doc\fullreview20260430`
  - 章別レビュー、method、risk map、summary、validity の構成を参照した。
- `C:\projects\NEPLg2_2\issues`
  - `issues/index.md`、`issues/index.json`、`issues/items/*.md` の管理形式を参照した。

## 実行した確認コマンド

```powershell
git status --short --branch
git rev-parse HEAD
cargo test --no-default-features
cargo check --no-default-features --features gui
cargo check --no-default-features --features tui
cargo check --no-default-features --features wasm --target wasm32-unknown-unknown
$env:WEBSOCKET_ADDRESS='ws://localhost:8081'; cargo check --no-default-features --features wasm --target wasm32-unknown-unknown
cargo check --no-default-features --features mobile
cargo check --no-default-features --features uefi --target x86_64-unknown-uefi
cargo clippy --no-default-features --all-targets -- -W clippy::all
cargo clippy --no-default-features --features gui -- -W clippy::all
cargo fmt --check
npm audit --omit=dev
```

`rg` は実行環境で `Access is denied` になったため、PowerShell の `Get-ChildItem` / `Select-String` で代替した。

## 判断基準

- 後方互換より正しい設計を優先する。誤った記法や API に合わせるために設計を曲げない。
- 暫定実装は許容するが、暫定設計は採用しない。
- 型安全とメモリ安全は必達とし、検査が効く構造にする。
- 有限状態や command は raw number / raw string ではなく enum で表し、`match` の網羅性検査を使う。
- `cargo fmt --check` が失敗する状態は、今後の差分監査を難しくするため issue 化する。
- `clippy` warning は即 bug でなくても、feature matrix の防壁として扱う。
- platform backend の `unwrap()` は、desktop より WASM / mobile / UEFI で影響が大きいため優先度を高くする。
- public field と private type warning は API 境界の歪みとして扱う。
- parser は失敗を `Content` に混ぜるのではなく、diagnostic を返せる設計を目標にする。
- web / dev server / logger の問題は、公開 deployment と local dev の境界を明示した上で issue 化する。

## レビューの限界

今回の確認は repository 全体を対象にしたが、全 path の runtime 動作を手動操作で網羅したものではない。特に GUI / WASM / mobile / UEFI は build/check と source review が中心であり、実機操作、ブラウザ操作、firmware runtime は別途 smoke test を整備する必要がある。
