// ./src/lib.rs

// uefi featureが有効な場合、no_stdとno_mainでコンパイルする
#![cfg_attr(feature = "uefi", no_std)]
#![cfg_attr(feature = "uefi", no_main)]

// uefi featureが有効な場合にのみ必要となる設定
#[cfg(feature = "uefi")]
mod uefi_setup {
    use core::panic::PanicInfo;

    /// uefiクレートが提供するアロケータをグローバルアロケータとして設定
    #[global_allocator]
    static ALLOCATOR: uefi::allocator::Allocator = uefi::allocator::Allocator;
}

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

// "uefi" featureが有効な時だけコンパイルされるUEFIバックエンドモジュール
#[cfg(feature = "uefi")]
pub mod uefi;