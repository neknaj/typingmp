# スクロール描画カリング設計 2026-05-07

## 現状確認

- `ui::build_typing_ui` は下段の completed segment だけを `typing_visible_line_bounds` で減らしているが、上段は全文の全 segment を `Renderable::TypingUpper` に渡している。
- `Renderable::TypingUpper` は `line_width` だけを持つため、途中の segment を省略すると描画開始位置を復元できない。下段だけが `TypingLineAlignment { full_line_width, visible_start_width }` を持つ。
- `renderer::draw_typing_lower` は `visible_left = -100` / `visible_right = width + 100` の固定値で描画有無を判定している。これは font size、display scale、aspect viewport、ruby の幅を反映しない。
- `gui_renderer::draw_text` は glyph pixel を frame 全体でしか clip しておらず、aspect ratio による viewport 外の letterbox/pillarbox へ描画を試みる可能性がある。また viewport 外 glyph でも outline/rasterize まで進む。

## 適切な挙動

- スクロールは「現在入力位置の周辺を viewport に表示する」ための変換であり、表示対象の連続した segment 範囲は `scroll_offset` と viewport 幅から決まる。
- UI 層はスクロール範囲外の segment を可能な範囲で `Renderable` に含めない。ただし、渡す範囲は連続範囲とし、開始 prefix 幅を `visible_start_width` として必ず持つ。
- renderer 層は最終防衛線として、すべての text/glyph/rect を display viewport に clip する。UI 層が過剰に渡しても viewport 外 pixel は描かれない。
- 上段と下段は同じ full-line 座標系を使う。上段は「タイピングする全文」、下段は「入力済み内容」だが、どちらも `full_line_width` と `visible_start_width` で左端を復元する。
- ruby は base segment と同じ segment box として扱う。ruby が base より広い場合も renderer の glyph clip に任せ、viewport 外 pixel は描かない。

## 設計

1. `Renderable::TypingUpper` を `line_width: u32` から `line_alignment: TypingLineAlignment` へ変更する。
2. `build_typing_ui` で scroll cache が使える場合、上段も `typing_visible_line_bounds` から visible segment range を計算し、その連続範囲だけを構築する。
3. visible range の開始 prefix を `visible_start_width` とし、renderer は `full_line_width` で line left を決めた後に `visible_start_width` を加算する。
4. renderer に viewport-local rect 判定と frame-coordinate clip rect を追加する。text draw は viewport と交差しない rect を呼ばず、部分交差は glyph 単位で clip する。
5. `gui_renderer::draw_text_clipped` を追加し、glyph bounds が clip rect と交差しない glyph は rasterize しない。既存 `draw_text` は full-buffer clip の互換 wrapper とする。

## 検証

- scroll cache 使用時に上段・下段の `TypingLineAlignment` が同じ full line width を保持し、長文では上段の visible prefix が非 0 になることを unit test で確認する。
- renderer は aspect viewport 外の text pixel を描かないことを unit test で確認する。
- `cargo fmt`、`cargo test --no-default-features`、必要に応じて feature check を通す。
