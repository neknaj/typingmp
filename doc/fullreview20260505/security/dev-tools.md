# security / tooling review: dev tools

対象 commit: `501bb73fe53a32bdbc100303498a9dc438ff85aa`

## 結論

local dev tool は便利だが、path traversal guard、debug logger の受け口、UEFI script の destructive operation が安全境界として弱い。開発専用と明示し、最低限の guard を入れるべきである。

## `serve.js`

`serve.js` は request path を `path.join(ROOT, urlPath)` し、`filePath.startsWith(ROOT)` で traversal を防ごうとしている。prefix check は sibling path や path normalization の扱いが弱い。

修正方針:

- `path.resolve(ROOT, '.' + urlPath)` を使う。
- resolved path が `ROOT` そのもの、または `ROOT + path.sep` で始まることを確認する。
- directory index と forbidden path の扱いを test する。

## `logger_server.js`

debug WebSocket logger は client 認証、message size limit、rate limit、log retention がない。local dev なら許容範囲だが、network に開くべきではない。

修正方針:

- bind address を `127.0.0.1` に固定する。
- message size と log size 上限を持つ。
- README に dev-only と明記する。

## UEFI scripts

`run_uefi.ps1` は `uefi_image` の `Resolve-Path` を作成前に呼ぶ可能性がある。`run_uefi_hyperv.ps1` は VM / VHD を削除する操作を含むため、target path の明示確認が必要である。

修正方針:

- path resolve は directory 作成後に行う。
- destructive operation は computed absolute path を表示し、named workspace 内に限定する。
- hardcoded QEMU / OVMF path は config 化する。

## 関連 issue

- `TP-SEC-001`
- `TP-SEC-002`
- `TP-BUILD-002`
