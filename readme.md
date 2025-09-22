# Rust Multi-Backend App (GUI / TUI / WASM)

RustでGUI、TUI、WASMの3つのバックエンドを持つテキスト入力アプリケーションのサンプルです。

## 実行方法

### GUI (Desktop)
```bash
cargo run --features gui
```

### TUI (Terminal)
```bash
cargo run --features tui
```

### WASM (Web Browser)

1.  **ビルドツールのインストール**
    ```bash
    cargo install wasm-pack
    ```

2.  **WASMへのコンパイル**
    ```bash
    wasm-pack build --target web -- --features wasm
    ```

3.  **ローカルサーバーの起動**
    プロジェクトのルートディレクトリで、`pkg`ディレクトリが生成されていることを確認してから実行します。
    ```bash
    python3 -m http.server
    # または python -m http.server
    ```

4.  **ブラウザでアクセス**
    ブラウザを開き、 `http://localhost:8000` にアクセスします。


### QEMU windows
```powershell
.\run_uefi.ps1
```