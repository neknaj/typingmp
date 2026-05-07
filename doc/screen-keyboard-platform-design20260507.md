# スクリーンキーボード マルチプラットフォーム設計

## 目的

スクリーンキーボードは Web DOM の付属機能ではなく、typingmp の入力装置の一つとして扱う。WASM、Slint mobile、将来の ESP32 + タッチパネルのような組み込み環境でも、同じ論理キーボードを同じ挙動で使えることを目的にする。

## 分離方針

スクリーンキーボードは次の三層に分ける。

1. `screen_keyboard` コア
   - キーボード種別、キー配列、キーの意味、フリック方向、ジェスチャー閾値、後置修飾、大⇔小変換、レイアウト切替を定義する。
   - DOM、Slint、OS IME、ファイルシステム、描画 API に依存しない。
   - `alloc` のみを使う設計にし、UEFI や組み込み向けの移植を妨げない。

2. プラットフォームアダプタ
   - WASM は Rust から受け取ったキー配列を DOM に描画し、PointerEvent の `dx/dy` をコアへ渡す。
   - Slint mobile は Rust から受け取ったキー配列を Slint のモデルとして描画し、TouchArea の `dx/dy` をコアへ渡す。
   - 組み込みでは同じキー配列から矩形を配置し、タッチパネル座標を `dx/dy` に変換してコアへ渡す。

3. アプリ入力ブリッジ
   - コアが返す `Text`、`UiCommand`、`TransformLastText`、`SwitchLayout`、`SwitchInputSource` を各バックエンドがアプリイベントへ変換する。
   - raw string は JS/Slint との境界だけで使い、アプリ内部では enum と `match` による網羅性検査を使う。

## コアが所有する責務

- `ScreenKeyboardLayoutKind`
  - `KanaFlick`
  - `Qwerty`
- `FlickDirection`
  - `Center`
  - `Up`
  - `Right`
  - `Down`
  - `Left`
- `ScreenKeyboardAction`
  - `Text`
  - `UiCommand`
  - `TransformLastText`
  - `SwitchLayout`
  - `SwitchInputSource`
  - `None`
- `ScreenKeyboardKeyClass`
  - `Text`
  - `Special`
  - `Backspace`
  - `Enter`
  - `Modifier`
  - `Spacer`
- `ScreenKeyboardKeyWidth`
  - `Normal`
  - `Wide`
  - `Space`

これらは enum とし、文字列や数値で状態を分岐しない。WASM/Slint へ公開するときだけ、描画用ラベルや CSS class 相当の値に変換する。

## キー配列

### かなフリック

既存の 5 列 × 4 行を維持しつつ、空きセルに QWERTY への切替キーを追加する。

```text
[Esc] [あ]   [か] [さ] [⌫]
[▲]   [た]   [な] [は] [▼]
[ABC] [ま]   [や] [ら] [ー]
[ ]   [大⇔小] [わ] [。] [↩]
```

フリック方向は既存仕様を維持する。

- タップ: あ段
- 上: う段
- 左: い段
- 右: え段
- 下: お段

### QWERTY

QWERTY は英字直接入力を行うスクリーンキーボードで、かなフリックと同じ `ScreenKeyboardAction::Text` を返す。かなへ戻るキー、Backspace、Enter、Space を含める。

```text
[Esc] [q] [w] [e] [r] [t] [y] [u] [i] [o] [p] [⌫]
[a]   [s] [d] [f] [g] [h] [j] [k] [l]
[かな] [z] [x] [c] [v] [b] [n] [m] [,] [.] [?]
[Space] [↩]
```

## 後置修飾

`大⇔小` はコアの `modified_kana` が変換候補を返す場合のみ作動する。直前入力が誤りとして保持されているかどうかはアプリ状態に依存するため、各バックエンドが `ScreenKeyboardInputState` に「最後にスクリーンキーボードから送った1文字」と「受理されたか」を記録する。

処理は次の順序に統一する。

1. `last_text` があり、かつ `last_accepted == false` のときだけ変換候補を探す。
2. 変換候補があれば Backspace をアプリへ送る。
3. 変換後の文字を再送する。
4. 再送後のアプリ状態で `last_accepted` を更新する。

## レイアウト切替

`SwitchLayout` は `ScreenKeyboardLayoutKind::next()` で次のレイアウトへ進む。現在は `KanaFlick` と `Qwerty` の循環だが、将来 12 キー英数、記号、テンキーを追加しても enum へ variant を追加し、`match` のコンパイルエラーで未対応箇所を検出できるようにする。

## 入力ソース切替

`SwitchInputSource` はプラットフォーム依存の操作であり、スクリーンキーボード本体の固定キー配列には含めない。

- WASM: アダプタ側の制御ボタンで、アプリ提供キーボードと OS IME を切り替える。
- Slint mobile: 現時点ではアプリ提供キーボードを主経路とし、物理キーボード入力は FocusScope で受ける。OS IME 連携を追加する場合も、この action に接続する。
- 組み込み: OS IME が存在しないため、アダプタが無効操作として扱える。

コアは「入力ソース切替」という意味だけを返し、実処理はアダプタに閉じ込める。

## 組み込み対応上の注意

- コアは画面サイズ、DPI、CSS、Slint の単位を知らない。
- キー幅は `ScreenKeyboardKeyWidth` の抽象値だけを持つ。実際の矩形計算はアダプタが行う。
- タッチパネルは pointer down/up の座標差を `dx/dy` としてコアへ渡す。
- ハードウェア依存のデバウンス、キャリブレーション、割り込み処理はコアへ入れない。

## 検証方針

- `screen_keyboard` の unit test で、かなフリック方向、QWERTY 出力、後置修飾、レイアウト切替を検証する。
- WASM は DOM の静的配列を廃止し、Rust から取得した配列だけでキーボードを生成する。
- Slint mobile はキー配列を Rust のモデルから受け取り、Slint ファイル内に文字配列を重複保持しない。
