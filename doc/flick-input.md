# フリック入力 仕様書

## 概要

Neknaj Typing MP の Web 版・モバイル版において、タッチデバイス上での入力を実現するスクリーンキーボードの仕様を記す。

キー配列、フリック解決、後置修飾、レイアウト切替は `src/screen_keyboard.rs` が所有する。WASM の DOM と Slint mobile は共通コアから渡されたキー配列を描画し、タッチ座標差 `dx/dy` を共通コアへ返すアダプタである。詳細設計は `doc/screen-keyboard-platform-design20260507.md` を参照する。

---

## キーボードレイアウト

かなフリックは 5列 × 4行の固定グリッド。

```
[Esc  ] [あ行] [か行] [さ行] [⌫  ]
[▲   ] [た行] [な行] [は行] [▼  ]
[ABC ] [ま行] [や行] [ら行] [ー  ]
[     ] [大⇔小] [わ行] [。行] [↩  ]
```

- **左列 / 右列**: 特殊キー（ナビゲーション・BS・Enter）
- **中3列**: かなフリックキー（各行の行見出し文字を表示）
- **ABCキー**: QWERTY スクリーンキーボードへ切替
- **大⇔小キー**: 後置修飾キー（後述）

---

## フリック方向とかな対応

各かなキーはタップ・4方向フリックで計5文字を入力できる。

| 操作 | 段 | 例 (か行) |
|------|------|-----------|
| 中央タップ | あ段 | か |
| 上フリック | う段 | く |
| 左フリック | い段 | き |
| 右フリック | え段 | け |
| 下フリック | お段 | こ |

### 全行の対応表

| キー | タップ | 上 | 左 | 右 | 下 |
|------|--------|----|----|----|----|
| あ行 | あ | う | い | え | お |
| か行 | か | く | き | け | こ |
| さ行 | さ | す | し | せ | そ |
| た行 | た | つ | ち | て | と |
| な行 | な | ぬ | に | ね | の |
| は行 | は | ふ | ひ | へ | ほ |
| ま行 | ま | む | み | め | も |
| や行 | や | ゆ | ゃ | ゅ | よ |
| ら行 | ら | る | り | れ | ろ |
| わ行 | わ | (なし) | を | (なし) | ん |
| 。行 | 。 | ？ | 、 | ！ | … |

---

## 正誤判定の仕様

### 入力経路

スクリーンキーボードは共通コアの `ScreenKeyboardAction::Text` をバックエンドが受け取り、アプリの `AppEvent::Char` へ変換する。WASM では `send_char`、Slint mobile では Rust コールバックを経由する。かなフリックはかな1文字を直接送るため、ローマ字の中間状態（unconfirmed バッファ）を経由しない。

### typing.rs での判定ロジック

1. **直接かな一致 (Path 1)**
   - `normalize_char(input) == normalize_char(target_char)` を検査する。
   - `normalize_char` はカタカナをひらがなに変換し、大文字を小文字に正規化する。
   - 一致した場合: `is_correct = true`、`advance_chars = 1`、`unconfirmed` をクリア。

2. **ローマ字入力 (Path 2)**
   - Path 1 で一致しなかった場合に実行する。
   - `layout.mapping` を走査し、`target_slice.starts_with(key)` かつ `value.starts_with(&unconfirmed + input)` を検索する。
   - 完全一致: `is_correct = true`、`advance_chars = key.len()`。
   - 前方一致(ローマ字途中): `is_correct = true`、`is_romaji_in_progress = true`、`unconfirmed.push(input)`。

3. **誤り入力**
   - Path 1・2 どちらでも一致しなかった場合。
   - `last_wrong_keydown = Some(input)` をセット。
   - `unconfirmed` はクリアせず、正しく入力済みのローマ字 prefix を保持する。
   - `typing_correctness[line][word][seg][char]` を `Incorrect` に更新。

### Backspace の動作

- `last_wrong_keydown` が Some の場合: 誤入力だけを取り消し、同時に `typing_correctness` の該当位置を `Incorrect → Pending` にリセットする。このとき `unconfirmed` のローマ字 prefix は保持する。
- `last_wrong_keydown` が None かつ `unconfirmed` が非空の場合: 末尾の1文字を削除（ローマ字入力の訂正）。
- いずれの場合も位置（line/word/segment/char）は後退しない。

### 正誤の視覚表示

| TypingCorrectnessChar | 色 | 意味 |
|-----------------------|----|------|
| Pending | グレー | 未入力 |
| Correct | 青 | 正しく入力済み |
| Incorrect | 赤 | 誤りを含む（Backspace 前の状態が保持） |

---

## 大⇔小キー（後置修飾）

### 設計方針

Gboard と同じ**後置修飾**方式を採用する。デッドキー（先にモードを選んでから文字を入力）とは逆で、かなを入力してから変換する。

### 作動条件

**直前のフリック入力が誤りだった場合にのみ作動する。**

各バックエンドは共通の `ScreenKeyboardInputState` 相当の状態を追跡する:

- `last_text`: 直前にスクリーンキーボードから送信した1文字
- `last_accepted`: 送信後にアプリが正解として受理したか

`has_wrong_input()` は Rust 側 WASM エクスポート関数で、`last_wrong_keydown.is_some() || !unconfirmed.is_empty()` を返す。

### 変換サイクル

大⇔小キーを繰り返し押すことで以下の連鎖を巡回する。

| 種別 | 連鎖 |
|------|------|
| か行・さ行・た行(て/と)・う | 清音 ↔ 濁音 (2連鎖) |
| た行 (つ) | つ → っ → づ → つ (3連鎖) |
| は行 | 清音 → 濁音 → 半濁音 → 清音 (3連鎖) |
| あ行・や行・わ | 大文字 ↔ 小文字 (2連鎖) |

### 処理フロー

```
1. `last_accepted = false` かつ `last_text` が存在する場合のみ実行
2. `screen_keyboard::modified_kana(last_text)` で次の形を取得
3. trigger_event('Backspace'):
   - last_wrong_keydown をクリア
   - typing_correctness[現在位置] を Incorrect → Pending にリセット
   - unconfirmed prefix は保持
4. 変換後の `Text` action と同じ経路で `next_char` を送信:
   - send_char(nextChar) を呼び出す
   - `last_text = next_char` を更新
   - アプリの誤り状態を確認し `last_accepted` を更新
```

### 状態リセットのタイミング

`last_text` と `last_accepted` は以下の操作でリセットされる:

- Backspace キー押下
- Enter キー押下
- Esc キー押下
- ▲ / ▼ ナビゲーションキー押下
- ー (長音符) 送信（通常の `Text` action として `last_text = 'ー'` に更新される）

---

## 古典かな (ゐ・ゑ) の代替入力

通常のフリックキーボードには ゐ・ゑ のキーが存在しないため、以下のローマ字エイリアスを `layout_data.rs` に追加することで代替入力を許容する。

| 目標文字 | 正規ローマ字 | 追加エイリアス |
|----------|-------------|----------------|
| ゐ | wyi | i (「い」と同じ) |
| ゑ | wye, we | e (「え」と同じ) |

フリックキーボードで「い」または「え」を入力した際、目標が ゐ・ゑ であれば正解と判定される。この判定は直接かな一致 (Path 1) ではなく、ローマ字マッピング (Path 2) で処理される。

---

## レイアウト切替

スクリーンキーボードは `ScreenKeyboardLayoutKind` でレイアウトを切り替える。

| レイアウト | 目的 |
|------------|------|
| `KanaFlick` | 日本語かなのフリック入力 |
| `Qwerty` | 英字・基本記号の直接入力 |

- かなフリックの `ABC` キーで QWERTY へ切り替える。
- QWERTY の `かな` キーでかなフリックへ戻る。
- レイアウト切替は WASM と Slint mobile の両方で同じ `ScreenKeyboardAction::SwitchLayout` を使う。

## 入力ソース切替

Web 版では IME Input が enabled のときだけ、OS 仮想キーボードとアプリ提供フリックキーボードを切り替えられる。既定の disabled では app モードに固定し、OS スクリーンキーボードを起動しない。

| モード | inputmode | blur 動作 | 物理キーボード |
|--------|-----------|-----------|----------------|
| app (IME disabled) | none | 即時 blur | `keydown` 経路で有効 |
| app (IME enabled) | none (タッチ時) / text (デスクトップ) | タッチ時のみ即時 blur | デスクトップでは有効 |
| device (OS IME) | text | blur しない | 有効 |

- 設定は `localStorage` の `typingmp_kb_mode` キーに永続化される。
- IME Input が disabled の間は `typingmp_kb_mode` に関係なく app モードとして扱い、printable key は WASM の `keydown` 経路で処理する。
- デスクトップ (非タッチ) で app モードかつ IME Input が enabled の場合は `inputmode="none"` および blur を適用しないため、物理キーボード入力が hidden input 経由でも機能する。
- 物理 `keydown` または hidden input の `input` を受けた場合、WASM 側のスクリーンキーボード補助状態 (`last_text` / `last_accepted`) は stale な後置修飾を避けるためリセットする。
- スクリーンキーボードの表示制御は `is_typing_active()` と `navigator.maxTouchPoints > 0` の AND 条件で行う。
- Slint mobile はアプリ提供キーボードを主経路とし、物理キーボード入力は FocusScope で受ける。OS IME 連携を追加する場合は `ScreenKeyboardAction::SwitchInputSource` に接続する。

---

## ジェスチャー閾値

| 操作 | 閾値 |
|------|------|
| フリック判定 (フリックキーボード) | 移動距離 ≥ 20px |
| タップ判定 (canvas) | 移動距離 < 12px |
| スワイプ判定 (canvas) | 移動距離 ≥ 40px かつ縦方向が優勢 |
| スワイプ 1ステップあたり | 60px |
| スワイプ最大ステップ数 | 5 |
