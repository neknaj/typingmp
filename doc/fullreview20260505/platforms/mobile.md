# platform review: mobile

対象 commit: `501bb73fe53a32bdbc100303498a9dc438ff85aa`

## 結論

mobile backend の Slint UI callback state は `Rc<RefCell<App>>` に寄せた。single-threaded UI callback で不要だった `Arc<Mutex<App>>` と `.lock().unwrap()` は解消済みで、panic poison による二次 crash 経路はなくなった。

## state ownership

Slint UI 側が単一 thread で `App` を操作するなら、`Rc<RefCell<App>>` または Slint の state model に寄せる方が自然である。multi-thread が必要なら、UI thread と worker thread の message passing を明示すべきである。

修正方針:

- UI callback 内の `.lock().unwrap()` は解消済み。
- panic poison による二次 crash 経路は解消済み。
- rendering snapshot を作り、UI callback が `App` internals を直接触らないようにする。

## Android entrypoint

Android main path の `unwrap()` は解消済み。mobile は device / permission / surface lifecycle の失敗が起きやすいため、init error はログとして返す。

## rendering duplication

mobile backend は GUI / WASM と同じく render pipeline を個別に持つ。renderer 共通化で backend 固有部分を surface transfer へ限定するべきである。

## 関連 issue

- `TP-MOBILE-001`
- `TP-ARCH-002`
- `TP-ERR-001`
