# platform review: mobile

対象 commit: `501bb73fe53a32bdbc100303498a9dc438ff85aa`

## 結論

mobile backend は Slint UI callback の中で `Arc<Mutex<App>>` を使っている。single-threaded UI callback で mutex を多用すると、panic poison や lock ordering の問題だけが残り、設計上の利点は薄い。

## state ownership

Slint UI 側が単一 thread で `App` を操作するなら、`Rc<RefCell<App>>` または Slint の state model に寄せる方が自然である。multi-thread が必要なら、UI thread と worker thread の message passing を明示すべきである。

修正方針:

- UI callback 内の `.lock().unwrap()` をなくす。
- panic poison による二次 crash を避ける。
- rendering snapshot を作り、UI callback が `App` internals を直接触らないようにする。

## Android entrypoint

Android main path に `unwrap()` がある。mobile は device / permission / surface lifecycle の失敗が起きやすいため、init error を user-visible にする必要がある。

## rendering duplication

mobile backend は GUI / WASM と同じく render pipeline を個別に持つ。renderer 共通化で backend 固有部分を surface transfer へ限定するべきである。

## 関連 issue

- `TP-MOBILE-001`
- `TP-ARCH-002`
- `TP-ERR-001`
