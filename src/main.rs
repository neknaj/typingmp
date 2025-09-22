// main関数 - featureフラグに応じて各バックエンドを起動
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // `gui` featureが有効な場合に、こちらのブロックがコンパイルされる
    #[cfg(feature = "gui")]
    {
        // もし`tui`も同時に有効になっていたら、警告を出す
        #[cfg(feature = "tui")]
        {
            println!("Warning: Both 'gui' and 'tui' features are enabled.");
            println!("Prioritizing GUI backend. To run the TUI version, use:");
            println!("cargo run --no-default-features --features tui");
        }
        println!("Starting GUI version... (Close the window or press ESC to exit)");
        // gui::run()を実行して終了
        return rust_multibackend_app::gui::run();
    }

    // `gui` featureが無効、かつ `tui` featureが有効な場合にのみ、こちらのブロックがコンパイルされる
    #[cfg(all(not(feature = "gui"), feature = "tui"))]
    {
        println!("Starting TUI version... (Press 'q' to exit)");
        // TUIモードに入る前に少し待機してメッセージを読めるようにする
        std::thread::sleep(std::time::Duration::from_secs(2));
        // tui::run()を実行して終了
        return rust_multibackend_app::tui::run();
    }

    // `gui`も`tui`もどちらも有効でない場合に、こちらのブロックがコンパイルされる
    // WASMビルド時はmain関数が使われないので、このブロックは実質的にデスクトップビルド用
    #[cfg(not(any(feature = "gui", feature = "tui", feature = "wasm")))]
    {
        println!("No backend feature enabled. Please run with --features gui or --features tui");
        return Ok(());
    }

    // wasm featureのみが有効な場合は何もしない（ライブラリとしてビルドされるため）
    #[cfg(all(feature = "wasm", not(any(feature = "gui", feature = "tui"))))]
    {
      //何もしない
      return Ok(());
    }
}