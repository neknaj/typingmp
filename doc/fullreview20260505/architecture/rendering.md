# アーキテクチャレビュー: 描画と性能

対象 commit: `501bb73fe53a32bdbc100303498a9dc438ff85aa`

## 結論

描画は `src/renderer.rs` の `ArgbSurface` / `RenderCache` に集約した。GUI / WASM / mobile / UEFI は shared render tree を同じ ARGB renderer に渡し、各 backend は canvas / image data / Slint Image / GOP blt への転送に集中する。

## backend 重複

`src/gui.rs`、`src/wasm.rs`、`src/mobile.rs`、`src/uefi.rs` の render match 重複は削除した。backend は `ui::build_ui` の結果を `ArgbSurface` に渡し、描画規則は shared renderer 側に閉じ込めている。

修正方針:

- `ArgbSurface` が ARGB pixel buffer を抽象化する。
- backend は input と surface 管理に集中し、render tree の構築と描画規則は shared path に寄せる。
- WASM / mobile / UEFI 固有の transfer cost は backend module に閉じ込める。

## 計算効率

`draw_linear_gradient` は pixel 全体を走査するが、`RenderCache` が background gradient を size / color key で保持するため、同一条件の frame では再計算しない。UEFI は full-screen ARGB buffer と GOP transfer buffer を loop 外で確保し、frame ごとに再利用する。

修正方針:

- viewport size ごとの background cache は shared renderer 側に移した。
- UEFI は persistent buffer を再利用する。
- text measurement は font / size / string key で cache する。

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
