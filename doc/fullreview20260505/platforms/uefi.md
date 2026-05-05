# platform review: UEFI

対象 commit: `501bb73fe53a32bdbc100303498a9dc438ff85aa`

## 結論

UEFI backend は compile check が通り、firmware call の `unwrap()` は `Status` return path に置き換え済み。frame ごとの大きな allocation は `ArgbSurface` / `RenderCache` と persistent buffer reuse により解消済みである。QEMU/OVMF は local environment に無いため smoke は未実行だが、`x86_64-unknown-uefi` target check は通る。

## firmware API unwrap

`src/uefi.rs` は `run_inner() -> Result<(), Status>` を持ち、UEFI helper init、graphics output protocol、timer、event wait、blit、font parse failure を `Status` に変換する。firmware 実装差や device 状態による failure で panic しない。

修正方針:

- init phase は `Result` で段階的に失敗を返す。完了済み。
- screen に最低限の error message を出す fallback は `report_startup_failure()` で UEFI console に出す。firmware caller には同じ `Status` を返す。
- unrecoverable error と retriable error は `Status` 境界で扱う。再試行 policy は今後の backend policy に残る。

## frame allocation

frame ごとの full-screen buffer と background buffer allocation は解消済み。ARGB buffer と GOP transfer buffer は loop 外で確保し、background gradient は shared cache で再利用する。

修正方針:

- persistent framebuffer は確保して再利用する。
- background gradient は size change 時だけ再計算する。
- text / UI primitive の差分更新を検討する。

## timestamp

`src/timestamp.rs` の UEFI timestamp は `runtime::get_time()` の failure を fallback し、leap year と month length を含む Unix millisecond 変換にした。invalid date は `0.0` に落とし、unit test で leap day と invalid date を固定している。

## 関連 issue

- `TP-UEFI-001`
- `TP-PERF-003`
- `TP-ERR-001`
