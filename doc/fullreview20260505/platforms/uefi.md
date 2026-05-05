# platform review: UEFI

対象 commit: `501bb73fe53a32bdbc100303498a9dc438ff85aa`

## 結論

UEFI backend は compile check は通るが、firmware call の `unwrap()` と frame ごとの大きな allocation が多い。UEFI は OS 上の desktop app と違い、allocation failure や device failure が現実的な制約なので、error path と buffer reuse を優先するべきである。

## firmware API unwrap

`src/uefi.rs` は stdout、graphics output protocol、event wait、file path などで `unwrap()` を使う。firmware 実装差や device 状態で失敗する可能性があり、panic すると診断が難しい。

修正方針:

- init phase は `Result` で段階的に失敗を返す。
- screen に最低限の error message を出す fallback を用意する。
- unrecoverable error と retriable error を分ける。

## frame allocation

frame ごとに full-screen buffer と background buffer を確保・変換している。高解像度 firmware では memory / time の両方で重い。

修正方針:

- persistent framebuffer を確保して再利用する。
- background gradient は size change 時だけ再計算する。
- text / UI primitive の差分更新を検討する。

## timestamp

`src/timestamp.rs` の UEFI timestamp は月・年を粗い定数で扱い、`unwrap()` も含む。ログや ordering に使うなら、正確性と失敗時 fallback を明示する必要がある。

## 関連 issue

- `TP-UEFI-001`
- `TP-PERF-003`
- `TP-ERR-001`
