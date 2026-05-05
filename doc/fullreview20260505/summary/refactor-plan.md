# refactor plan

対象 commit: `501bb73fe53a32bdbc100303498a9dc438ff85aa`

## 方針

修正は platform 全体へ波及しやすいため、先に防壁を整え、その後に core と backend を分割する。巨大 file を一度に分割するより、test 可能な pure core から進める。

目標構造は、`no_std + alloc` で成立する typing core と、desktop / web / mobile / firmware / terminal に具体化する UI adapter の二層である。`std`、filesystem、DOM、terminal、firmware API、font discovery は core に入れない。

後方互換は不要なので、不適切な既存設計を維持するための adapter は作らない。状態と command は enum / newtype に移し、`match` の網羅性検査が効くようにする。

## Phase 1: build / lint の blocker 解消

- WASM debug logger を `option_env!` / feature flag にする。
- `cargo fmt` を適用する。
- `clippy` warning のうち、機械的に直せるものを先に潰す。
- release workflow の `actions-rs` を更新し、feature matrix を明示する。

検証:

```powershell
cargo fmt --check
cargo clippy --no-default-features --all-targets -- -D warnings
cargo check --no-default-features --features wasm --target wasm32-unknown-unknown
```

## Phase 2: parser / typing core の testable 化

- parser / typing / model / layout を `core + alloc` 前提で整理する。
- no_std core 単体を build する feature または crate を用意する。
- problem source label / diagnostic source id を導入し、raw host path を core から外す。
- typing cursor / menu item / source kind / command を enum または newtype にする。
- parser を `Result<Content, ParseDiagnostics>` にする。
- README sample と malformed input fixture を追加する。
- `key_input` を `&mut TypingModel` に寄せる。
- cursor/index newtype の導入範囲を決める。

検証:

```powershell
cargo test --no-default-features parser
cargo test --no-default-features typing
```

## Phase 3: `App` の責務分割

- problem repository を分離する。
- problem / font / storage / clock / logger provider を adapter から注入する。
- scene state と command dispatch を分ける。
- scroll cache を private module に移す。
- backend へ view snapshot を渡す API を作る。
- font discovery、file picker、storage、clipboard、terminal、firmware は adapter 側へ閉じ込める。

検証:

```powershell
cargo check --no-default-features --features gui
cargo check --no-default-features --features tui
cargo test --no-default-features
```

## Phase 4: backend error handling

- TUI に `TerminalGuard` を導入する。
- WASM init を `Result<(), JsValue>` 化する。
- mobile callback の `Mutex` / `unwrap()` を整理する。
- UEFI init と buffer allocation の失敗を扱う。

## Phase 5: renderer 共通化と性能改善

- backend surface trait を導入する。
- gradient / text measurement cache を shared renderer に寄せる。
- UEFI frame buffer を再利用する。

## Phase 6: docs / artifacts

- README syntax を修正する。
- generated artifact の扱いを決める。
- dev-only tool の README と guard を整える。
