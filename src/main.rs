// ./src/main.rs
#![cfg_attr(feature = "uefi", no_std)]
#![cfg_attr(feature = "uefi", no_main)]

#[cfg(not(feature = "uefi"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "gui")]
    {
        println!("Starting GUI version... (Close the window or press ESC to exit)");
        rust_multibackend_app::gui::run()
    }

    #[cfg(all(not(feature = "gui"), feature = "wasi-tui"))]
    {
        println!("Starting WASI TUI version... (/q to exit)");
        rust_multibackend_app::wasi_tui::run()
    }

    #[cfg(all(not(feature = "gui"), not(feature = "wasi-tui"), feature = "tui"))]
    {
        println!("Starting TUI version... (Press ESC to exit)");
        rust_multibackend_app::tui::run()
    }

    #[cfg(all(
        not(feature = "gui"),
        not(feature = "wasi-tui"),
        not(feature = "tui"),
        feature = "mobile"
    ))]
    {
        println!("Starting Mobile (Slint) version...");
        rust_multibackend_app::mobile::run()
    }

    #[cfg(not(any(
        feature = "gui",
        feature = "wasi-tui",
        feature = "tui",
        feature = "mobile"
    )))]
    {
        println!(
            "No desktop backend feature enabled. Please run with --features gui, tui, wasi-tui, or mobile"
        );
        Ok(())
    }
}

#[cfg(feature = "uefi")]
#[uefi::entry]
fn efi_main() -> uefi::prelude::Status {
    rust_multibackend_app::uefi::run()
}
