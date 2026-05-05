# 検証状況

対象 commit: `501bb73fe53a32bdbc100303498a9dc438ff85aa`

## 結論

parser test と主要 feature の `cargo check` は概ね通る。一方で、WASM debug build は `WEBSOCKET_ADDRESS` が未設定だと compile できず、`cargo fmt --check` は失敗する。`clippy` は failure ではないが、多数の warning が残っており、CI 防壁としては弱い。

## 実行結果

| コマンド | 結果 | 判断 |
|---|---:|---|
| `cargo test --no-default-features` | pass | parser test 22 件のみで、core typing / app / backend coverage は不足している。 |
| `cargo check --no-default-features --features gui` | pass with warnings | GUI 固有の unused / unreachable / needless warning が残る。 |
| `cargo check --no-default-features --features wasm --target wasm32-unknown-unknown` | fail | debug logger が build-time env `WEBSOCKET_ADDRESS` を要求する。 |
| `WEBSOCKET_ADDRESS=... cargo check --features wasm` | pass with warnings | env を与えれば通るが CI / release workflow との整合がない。 |
| `cargo check --no-default-features --features mobile` | pass with warnings | Slint / Android path の `unwrap()` と duplicate renderer は残る。 |
| `cargo check --no-default-features --features uefi --target x86_64-unknown-uefi` | pass with warnings | firmware runtime failure と frame allocation は compile では検出されない。 |
| `cargo clippy --no-default-features --all-targets -- -W clippy::all` | pass with warnings | redundant closure、`ptr_arg`、`if_same_then_else`、`useless_vec` などが残る。 |
| `cargo clippy --no-default-features --features gui -- -W clippy::all` | pass with warnings | GUI 固有 warning が追加で出る。 |
| `cargo fmt --check` | fail | `build.rs`、`src/app.rs`、`src/wasm.rs` などで format 差分が大きい。 |
| `npm audit --omit=dev` | pass | npm dependency vulnerability は検出されなかった。 |

## 重要な build blocker

`src/wasm_debug_logger.rs:34` は debug build で `env!("WEBSOCKET_ADDRESS", ...)` を参照する。`.github/workflows/release.yml` の WASM build はこの env を設定していないため、debug 相当の build では release workflow が失敗する可能性が高い。

修正方針:

- build-time env がなくても compile できる fallback を用意する。
- debug logger を feature flag で明示的に分ける。
- CI matrix に WASM debug / release の両方を入れる。

## 警告の扱い

現状の warning は多くが即時 crash ではない。ただし、private interface warning、unused state、unreachable code、unused renderer parameter は、設計分割前に放置すると後続 refactor の信頼性を下げる。

`cargo fmt --check` と `cargo clippy -- -D warnings` をすぐに全 feature へ適用すると修正範囲が広くなるため、まず `no-default-features` と主要 feature を順に clean にするのが現実的である。

## 関連 issue

- `TP-CI-001`
- `TP-CI-002`
- `TP-CI-003`
- `TP-CORE-003`
