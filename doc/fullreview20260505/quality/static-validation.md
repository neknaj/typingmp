# 品質レビュー: 静的検証

対象 commit: `501bb73fe53a32bdbc100303498a9dc438ff85aa`

## 結論

現状の静的検証は、build が通ることと品質 gate が通ることの間に差がある。`cargo fmt --check` は失敗し、`clippy` warning は多数残る。リファクタリング前に format と lint の基準を整えないと、修正差分の安全性を評価しにくい。

## format

`cargo fmt --check` は大きな差分を出す。特に `build.rs`、`src/app.rs`、`src/wasm.rs` など、今後触る可能性が高い file が対象である。

修正方針:

- まず format-only commit を作る。
- その後の refactor は format 済みを前提にする。
- docs-only 変更と code format を同じ commit に混ぜない。

## clippy

検出された warning には、`if_same_then_else`、`ptr_arg`、`useless_vec`、redundant closure、unreachable expression、unused variable / parameter が含まれる。

修正方針:

- `no-default-features` の clippy warning を先に 0 にする。
- 次に `gui`、`wasm`、`mobile`、`uefi` の feature ごとに潰す。
- 最後に CI で `-D warnings` を段階導入する。

## feature matrix

マルチプラットフォーム project なので、単一 default build だけでは不十分である。

最低限の matrix:

- `cargo test --no-default-features`
- `cargo check --no-default-features --features gui`
- `cargo check --no-default-features --features tui`
- `cargo check --no-default-features --features wasm --target wasm32-unknown-unknown`
- `cargo check --no-default-features --features mobile`
- `cargo check --no-default-features --features uefi --target x86_64-unknown-uefi`

## 関連 issue

- `TP-CI-002`
- `TP-CI-003`
