# security / UX review: web behavior

対象 commit: `501bb73fe53a32bdbc100303498a9dc438ff85aa`

## 結論

web 版は利用者の browser 環境に直接触るため、clipboard、localStorage、custom problem import の失敗を明確に扱う必要がある。現状は silent fallback と implicit side effect が混在している。

## clipboard side effect

初回 pointerdown で clipboard へ固定文字列を書き込む処理は、ユーザーの明示操作と目的が一致していない。typing app として期待される挙動ではないため削除対象である。

## custom problem import

custom problem load は JSON parse / schema mismatch / oversized content などの失敗を user に説明できる必要がある。parser が `Content` だけを返す現状では、web UI 側も失敗の粒度を持てない。

修正方針:

- parser diagnostic と import diagnostic を分ける。
- localStorage failure は visible status に出す。
- size limit と schema version を持たせる。

## event cancellation

web input event で `prevent_default()` を呼んでも、input event の実際の text mutation を完全には取り消せない。reset / blur に頼る実装はブラウザ差に弱い。

修正方針:

- `beforeinput` / `keydown` / composition event の役割を整理する。
- hidden input と virtual keyboard の責務を明確にする。
- browser matrix smoke を用意する。

## 関連 issue

- `TP-WEB-001`
- `TP-WASM-001`
- `TP-CORE-001`
