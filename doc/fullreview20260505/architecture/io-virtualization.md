# アーキテクチャレビュー: I/O 仮想化と provider 境界

対象 commit: `501bb73fe53a32bdbc100303498a9dc438ff85aa`

参照元: `C:\projects\NEPLg2_2`

## 結論

NEPLg2 の compiler 設計で typingmp に参考になるのは、core が host filesystem を直接正規入力にせず、loader / SourceMap / VFS / preopen の層で I/O を扱っている点である。typingmp でも、問題文、font asset、永続 storage、logger、clipboard、時刻を core から分離し、target adapter が provider として渡す構造にするべきである。

## NEPLg2 の確認結果

### SourceMap

`nepl-core/src/source_map.rs` は `no_std + alloc` で、source text と path label を `FileId` に対応付ける。core diagnostic は host path ではなく `FileId` と span を使い、表示時に label へ戻す。

重要な点:

- file identity は `FileId`。
- path は `SourcePath` の stable label。
- source capability は file に付与される。
- host filesystem path は core の主識別子ではない。

### Loader と VFS

`nepl-core/src/loader.rs` は通常 file load と `load_inline_with_provider` を持つ。web 側は filesystem を使えないため、`nepl-web/src/lib.rs` で `BTreeMap<PathBuf, String>` の VFS を作り、provider closure で loader に source を渡している。

重要な点:

- entry source は inline で渡せる。
- import/include は provider 経由で解決できる。
- web は `/stdlib` のような仮想 root と generated stdlib entries を使う。
- missing source は loader error として diagnostic path に流れる。

### CLI と preopen

`nepl-cli/src/main.rs` の WASI host は preopen root を持ち、guest path の absolute path、parent dir、prefix/root component を拒否する。read / write / stat は root 内に canonicalize される。

重要な点:

- host FS 全体を guest に見せない。
- guest path は capability root 内へ制限する。
- read / write / dir / stat の capability を分ける。
- stdio / tty / fd state は runtime host 側に閉じる。

## typingmp への適用

typingmp の core は problem content と typing state を扱うだけにし、I/O は adapter が解決する。

推奨 provider:

- `ProblemSourceProvider`
  - builtin examples
  - user file
  - web localStorage
  - embedded demo
- `AssetProvider`
  - font bytes
  - fallback font
  - target-specific font discovery
- `PersistentStore`
  - custom problems
  - settings
  - progress / score
- `Clock`
  - metrics 用の monotonic time
  - wall clock は adapter 側
- `Logger`
  - dev websocket
  - console
  - no-op
- `Clipboard`
  - explicit user action のみ

## 現状の typingmp の問題

`src/app.rs` は font discovery、`std::fs::read_dir`、`std::fs::read`、`std::env`、`PathBuf` を直接扱っている。GUI / TUI / mobile も font file read を個別に持つ。WASM は localStorage と DOM を直接扱う。

この構造では、typing core が no_std level であること、UI が target ごとに段階的に具体化されることを保ちにくい。

## 修正方針

1. `ProblemId` / `ProblemSourceLabel` を導入し、raw path ではなく stable label で問題を識別する。
2. `ProblemRepository` は provider から `ProblemSource` を受け取り、parser diagnostic を返す。
3. font discovery は `AssetProvider` に移し、core / app state から filesystem を消す。
4. settings / custom problem は `PersistentStore` 経由にし、web localStorage、desktop file、UEFI no-op を差し替える。
5. `Clock` を adapter から注入し、typing metrics は `u64` tick または duration を扱う。
6. `Logger` は default no-op にし、debug websocket は adapter opt-in にする。

## NEPLg2 から借りる時の注意

NEPLg2 の loader は compiler の import/include を扱うため `PathBuf` が残っている。typingmp は問題文と asset を扱うだけなので、core 側では `PathBuf` ではなく string label / ID に寄せた方が no_std 目標に合う。

また、NEPLg2 の CLI は WASI runtime host を厚く持つ。typingmp では同じ厚さの runtime host は不要だが、preopen root のような capability 制限は desktop file picker や dev server に応用できる。

## 実装進捗: T05

`src/io.rs` に provider 境界を追加し、problem source、font asset、persistent store、clock、logger を trait と typed id で表現した。`App` は `ProblemRepository` と font catalog / load request を保持し、desktop filesystem path や web localStorage の詳細は backend provider 実装に閉じた。

この段階では parser diagnostic の source span と backend visible error は未完了であり、T08 / T12 / T14 の対象として残す。

## 関連 issue

- `TP-ARCH-005`
- `TP-ARCH-004`
- `TP-WASM-001`
- `TP-SEC-001`
