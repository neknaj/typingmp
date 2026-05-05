# platform review: UEFI

対象 commit: `501bb73fe53a32bdbc100303498a9dc438ff85aa`

## 結論

UEFI backend は compile check は通るが、firmware call の `unwrap()` が残っている。frame ごとの大きな allocation は `ArgbSurface` / `RenderCache` と persistent buffer reuse により解消済みである。UEFI は OS 上の desktop app と違い、device failure が現実的な制約なので、残りは error path を優先する。

## firmware API unwrap

`src/uefi.rs` は stdout、graphics output protocol、event wait、file path などで `unwrap()` を使う。firmware 実装差や device 状態で失敗する可能性があり、panic すると診断が難しい。

修正方針:

- init phase は `Result` で段階的に失敗を返す。
- screen に最低限の error message を出す fallback を用意する。
- unrecoverable error と retriable error を分ける。

## frame allocation

frame ごとの full-screen buffer と background buffer allocation は解消済み。ARGB buffer と GOP transfer buffer は loop 外で確保し、background gradient は shared cache で再利用する。

修正方針:

- persistent framebuffer は確保して再利用する。
- background gradient は size change 時だけ再計算する。
- text / UI primitive の差分更新を検討する。

## timestamp

`src/timestamp.rs` の UEFI timestamp は月・年を粗い定数で扱い、`unwrap()` も含む。ログや ordering に使うなら、正確性と失敗時 fallback を明示する必要がある。

## 関連 issue

- `TP-UEFI-001`
- `TP-PERF-003`
- `TP-ERR-001`
