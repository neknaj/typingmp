# 品質レビュー: tests

対象 commit: `501bb73fe53a32bdbc100303498a9dc438ff85aa`

## 結論

現状の automated test は parser に偏っている。typing application として重要な入力遷移、model invariant、layout lookup、custom problem import、platform init の regression test が不足している。

## 現状

`cargo test --no-default-features` は 22 件の parser test を通した。これは有用だが、typing engine と platform integration の正しさを保証しない。

## 追加すべき test

- `typing::key_input`
  - romaji prefix / candidate selection
  - `n` auto-commit
  - miss count
  - backspace
  - line transition
- `parser`
  - malformed bracket
  - unclosed ruby
  - empty ruby / reading
  - README sample
- `model/layout`
  - mapping collision
  - first char lookup
  - prefix handling
- `app`
  - scene transition
  - custom problem load failure
  - scroll cursor visibility
- platform smoke
  - WASM init failure path
  - TUI guard cleanup
  - renderer buffer size mismatch

## testability の前提

`App` が mutable public state を抱えたままだと pure test が書きにくい。先に typing core と parser の Result 化を進め、その後 app snapshot / view model を test target にするのがよい。

## 関連 issue

- `TP-CORE-003`
- `TP-CORE-001`
- `TP-CORE-002`
