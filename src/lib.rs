// アプリケーションの共通モジュールを宣言
pub mod app;
pub mod renderer;
pub mod ui;

// "gui" featureが有効な時だけコンパイルされるGUIバックエンドモジュール
#[cfg(feature = "gui")]
pub mod gui;

// "tui" featureが有効な時だけコンパイルされるTUIバックエンドモジュール
#[cfg(feature = "tui")]
pub mod tui;

// "wasm" featureが有効な時だけコンパイルされるWASMバックエンドモジュール
#[cfg(feature = "wasm")]
pub mod wasm;