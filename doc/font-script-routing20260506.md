# Font Script Routing Design

## 目的

typingmp のフォント設定は、UI 文字用のフォントと、問題文の script 別フォントを明示的に切り替える機能である。問題文については、日本語、中国語、英語が混在する各表示 segment に対して、reading や base の文字種から適切な script を自動判定し、その script に対応するフォントで描画する。

## フォントスロット

フォントは次の 5 スロットとして管理する。

| slot | 既定フォント | 用途 |
|---|---|---|
| UI Font | `NotoSerifJP` | メニュー、設定、ステータス、ヘルプ、結果、問題一覧、問題ソースなど、問題文の ruby/base 表示ではない UI 文字として表示する。 |
| Japanese | `YujiSyuku` | reading が平仮名または片仮名の segment。日本語漢字、日本語 kana、日本語約物として表示する。 |
| Simplified Chinese | `MaShanZheng` | reading がピンインの segment。簡体字と中国語文脈の約物として表示する。 |
| Traditional Chinese | `MaShanZheng` | reading が注音符号の segment。繁体字と繁体字中国語文脈の約物として表示する。現時点では専用フォントが未用意のため簡体字と同じ既定フォントを使う。 |
| English | `Kalam` | アルファベット、Latin 系文字、ASCII/英語約物として表示する。 |

Settings 画面はこの 5 スロットへフォントを割り当てる画面である。各スロットには `fonts/` 配下の `.ttf` / `.otf` をすべて候補として出す。ファイルシステムを列挙できるデスクトップ系バックエンドでは `fonts/` とシステムフォントを列挙し、WASM/WASI/UEFI ではビルド時に列挙した `fonts/` の埋め込み/配布フォントを候補として使う。通常の端末セル描画では任意フォントを適用できないため、TUI/WASI の plain text は端末側フォントに従う。

## 判定規則

問題文の `[base/reading]` の `reading` を見て `FontScript` を決める。

| reading | FontScript |
|---|---|
| 平仮名、片仮名、半角片仮名を含む | `Japanese` |
| 注音符号を含む | `TraditionalChinese` |
| ASCII / Latin 系文字を含む | `ChineseSimplified` |
| 判定材料がない plain segment | `Japanese` |

`{inner/annotation}` は annotation ではなく inner 側の reading を連結して判定する。日本語 kana と注音や pinyin が同じ reading に混在する malformed な segment では、より明確な非日本語表記である注音を優先し、次に kana、最後に Latin を見る。

plain segment は reading を持たないため base の文字種から script run に分割する。平仮名、片仮名、日本語固有の約物は `Japanese`、注音符号は `TraditionalChinese`、アルファベットや英語約物は `English` とする。CJK 共有約物や空白は周辺文脈を継承するが、CJK 共有約物は `English` を継承しない。漢字だけで判定材料がない plain segment は `Japanese` に倒す。

## 描画と計測

script 判定は描画だけでなく、スクロール計算、segment 幅、typing 下段の入力済み幅にも使う。描画で使うフォントと計測で使うフォントがずれると、上段の全文、下段の入力済み内容、カーソル位置、スクロール位置が再びずれるため、必ず同じ `FontScript` と同じ `Fonts` から width を計算する。mixed-script の plain segment や `{inner/annotation}` は segment 全体を単一フォントで測らず、script run ごとに描画・計測する。

アプリ UI chrome、メニュー、ステータスなど問題文ではない text は UI Font スロットを使う。title、現在行、前後の文脈行など問題ファイルから構造化された `Line` は、UI text ではなく ruby/base と script 別フォントを持つ問題コンテンツとして描画する。問題ソース表示は raw `.ntq` の確認画面なので、`[base/reading]` 構文を ruby に変換せず UI Font で表示する。

## 型安全性

script は文字列や数値ではなく `FontScript` enum で保持する。フォント取得、設定適用、描画分岐は `match FontScript` で実装し、スロット追加時に網羅性検査が効く形にする。

`Fonts` は直接フィールドを書き換えられる bag ではなく、`Fonts::new` と `set_for_script` で構築・更新する。更新時には generation を進め、renderer の text measure cache は generation 変化で破棄する。これによりフォント差し替え後に旧フォントの計測値が残ることを防ぐ。

## 表示設定

Settings 画面は font slot に加えて表示 viewport も扱う。ただし、viewport 設定は問題文の font routing と独立した表示設定であり、segment の script 判定を変更しない。

| setting | type | 動作 |
|---|---|---|
| Aspect Ratio | `DisplayAspectRatio` enum | 物理画面内に選択比率の viewport を最大内接させる。余白は letterbox / pillarbox として黒で埋める。 |
| Display Scale | `DisplayScale` enum | viewport 座標系は維持し、文字系 UI の font size に倍率を掛ける。 |

aspect ratio は `Native`, `16:9`, `4:3`, `1:1`, `3:4`, `9:16` を持つ。display scale は `75%`, `100%`, `125%`, `150%`, `200%` の段階を持つ。どちらも raw number や free string では保持しない。

renderer は app が持つ `DisplaySettings` から `DisplayViewport` を計算し、UI 構築、スクロール計測、描画に同じ viewport 幅・高さと scale を渡す。これにより aspect/scale 変更時にも typing 上段、下段、カーソル、スクロールの座標が同じ基準で揃う。
