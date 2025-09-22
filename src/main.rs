/// main関数 - featureフラグに応じて各バックエンドを起動
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // "gui" featureが有効な場合にコンパイルされるブロック
    #[cfg(feature = "gui")]
    {
        println!("Starting GUI version... (Close the window or press ESC to exit)");
        return rust_multibackend_app::gui::run();
    }

    // "gui" が無効で "tui" が有効な場合にコンパイルされるブロック
    #[cfg(all(not(feature = "gui"), feature = "tui"))]
    {
        println!("Starting TUI version... (Press 'q' to exit)");
        std::thread::sleep(std::time::Duration::from_secs(2));
        return rust_multibackend_app::tui::run();
    }

    // デスクトップ用featureが一つも有効でない場合にコンパイルされるブロック
    #[cfg(not(any(feature = "gui", feature = "tui")))]
    {
        println!("No desktop backend feature enabled. Please run with --features gui or --features tui");
        return Ok(());
    }
}