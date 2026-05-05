# アーキテクチャレビュー: 描画と性能

対象 commit: `501bb73fe53a32bdbc100303498a9dc438ff85aa`

## 結論

描画は `src/renderer.rs` に共通処理がある一方で、GUI / WASM / mobile / UEFI がそれぞれ frame buffer、canvas、image data、font handling を個別に組み立てている。結果として、同じ UI state の描画に複数の実装が存在し、性能改善や bug fix の反映漏れが起きやすい。

## backend 重複

`src/gui.rs`、`src/wasm.rs`、`src/mobile.rs`、`src/uefi.rs` はそれぞれ render target を確保し、`App` を更新し、rendering output を画面へ転送する。WASM は JS canvas 側の描画や keyboard handling も大きく、Rust renderer と JS UI の境界が曖昧である。

修正方針:

- `RenderSurface` trait を作り、pixel buffer / canvas / firmware blt を抽象化する。
- backend は input と surface 管理に集中し、render tree の構築は shared path に寄せる。
- WASM / mobile / UEFI 固有の transfer cost は backend module に閉じ込める。

## 計算効率

`draw_linear_gradient` は pixel 全体を走査する。WASM では gradient cache があるが、GUI / mobile / UEFI では毎 frame の処理が残る可能性がある。UEFI は full-screen buffer と background buffer を frame ごとに大きく確保しており、firmware 環境では特に重い。

修正方針:

- viewport size ごとの background cache を shared renderer 側に持つ。
- UEFI は persistent buffer を再利用し、差分更新または必要最小限の blit に寄せる。
- text measurement と rasterization の結果を font / size / string / style key で cache する。

## text measurement

`measure_text` は fractional width を `u32` へ落としている。typing text の cursor visibility や wrap 判定は、誤差が蓄積すると platform ごとに scroll 位置がずれる。

修正方針:

- 内部計算は `f32` / logical pixel のまま保持する。
- render 最終段で round / floor を backend ごとに決める。
- scroll cache は measured width の単位を明示する。

## 関連 issue

- `TP-ARCH-002`
- `TP-PERF-002`
- `TP-PERF-003`
