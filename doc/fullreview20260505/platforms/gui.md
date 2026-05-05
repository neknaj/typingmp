# platform review: GUI

対象 commit: `501bb73fe53a32bdbc100303498a9dc438ff85aa`

## 結論

GUI backend は build 可能だが、event loop と render API に小さな設計ずれが残る。`clippy` で unreachable / diverging expression、unused parameter、unused mut が検出されており、backend 共通化前に整理すべきである。

## event loop

`src/gui.rs` の `run` は `event_loop.run(...)` を `return` しており、diverging function として扱われる箇所に warning が出る。これは runtime bug ではないが、backend init の戻り値や error path を設計し直す時に邪魔になる。

修正方針:

- `run` の戻り型と winit event loop の diverging API を明示する。
- init error は event loop 開始前に `Result` で返す。
- event loop 内の error は logging / graceful exit policy を決める。

## render API

`render_frame` は `pixels: &mut Pixels` を受け取るが、現状では使っていない。frame buffer を受け取るだけで十分なら引数を削るべきであり、今後の renderer 共通化では backend surface abstraction に寄せるべきである。

## 関連 issue

- `TP-ARCH-002`
- `TP-ERR-001`
