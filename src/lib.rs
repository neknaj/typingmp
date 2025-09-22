// 共通モジュール
pub mod app;
pub mod renderer;

// GUIバックエンド (feature = "gui")
#[cfg(feature = "gui")]
pub mod gui;

// TUIバックエンド (feature = "tui")
#[cfg(feature = "tui")]
pub mod tui;

// WASMバックエンド (feature = "wasm")
#[cfg(feature = "wasm")]
pub mod wasm;