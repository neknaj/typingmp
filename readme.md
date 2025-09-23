# Neknaj Typing MP: A Multi-Platform Typing Game in Rust

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

**Neknaj Typing MP** is a typing practice application designed to showcase the power and portability of the Rust programming language. This project demonstrates how a single, shared core codebase can be deployed across four fundamentally different backends: a native **GUI** for desktops, a feature-rich **TUI** for terminals, a **WASM** build for web browsers, and even a bare-metal **UEFI** application that runs before an OS boots.

It serves as both a functional typing tutor for Japanese Romaji input and a technical showcase of clean architecture, platform abstraction, and Rust's versatility.

<!-- TODO: Add a GIF showcasing the different backends in action -->
![image](https://github.com/user-attachments/assets/51553c31-97b5-4b36-9b5a-7e3f81e3d930)


## ✨ Features

-   **True Multi-Platform Support**: The same logic runs seamlessly on:
    -   **Desktop (GUI)**: A minimal, fast, and responsive windowed application.
    -   **Terminal (TUI)**: A sophisticated terminal UI with multiple rendering modes.
    -   **Web (WASM)**: A fully interactive web application compatible with modern browsers.
    -   **UEFI (Bare-Metal)**: A pre-boot application running directly on firmware.

-   **Shared Core Logic**: A clean architecture separates the application's state management and typing logic from the platform-specific UI rendering.

-   **Advanced TUI Rendering**: The TUI backend isn't just text. It supports three distinct modes:
    -   `SimpleText`: A clean, classic terminal look.
    -   `AsciiArt`: Renders text using ASCII characters for a stylized feel.
    -   `Braille`: Utilizes Braille Unicode characters to achieve a surprisingly high-resolution "pixel art" effect in the terminal.

-   **Smooth, Animated UI**: The typing view features a dynamically scrolling text line that keeps the user's cursor centered, implemented with frame-rate independent animation logic that works across all backends.

-   **Flexible Japanese Romaji Support**: A comprehensive Romaji-to-Kana conversion table (`layout_data.rs`) allows for multiple typing styles (e.g., `shi` and `si` for し).

-   **Intelligent WASM IME Handling**: The web version correctly handles Input Method Editors (IMEs) for Japanese input by using a hidden input field and resetting its state after each segment, ensuring a smooth typing experience.

-   **Font Selection**: Users can switch between multiple pre-packaged fonts (Yuji Syuku and Noto Serif JP) to customize their experience.

## 🏛️ Architecture

The key to this project's portability is its carefully designed architecture, which isolates platform-agnostic code from platform-specific code.

```
+-------------------------------------------------+
|               Backends (The "Body")             |
|   [gui.rs] [tui.rs] [wasm.rs] [uefi.rs]         |
+----------------------+--------------------------+
                       |
                       | (Uses)
                       V
+-------------------------------------------------+
|            Rendering Engine (renderer.rs)       |
| [gui_renderer]          [tui_renderer]          |
+----------------------+--------------------------+
                       |
                       | (Consumes)
                       V
+-------------------------------------------------+
|         UI Abstraction Layer (ui.rs)            |
|       - Builds a list of `Renderable` items     |
+-------------------------------------------------+
                       |
                       | (Reads state from)
                       V
+-------------------------------------------------+
|            Core Logic (The "Brain")             |
| [app.rs] [typing.rs] [model.rs] [parser.rs] ... |
+-------------------------------------------------+
```

1.  **Core Logic**: This is the application's "brain". It is written in pure, platform-agnostic Rust. It manages state, handles typing logic, calculates metrics, and knows nothing about how to draw a pixel or handle a mouse click.

2.  **UI Abstraction Layer (`ui.rs`)**: This layer acts as a bridge. It inspects the state of the Core Logic and produces a platform-independent list of drawing commands called `Renderable`s (e.g., "draw this text at the center," "render the typing-prompt here").

3.  **Rendering Engine (`renderer.rs`)**: Contains the specialized "artists".
    -   `gui_renderer`: Knows how to turn text into pixels using font data. Used by GUI, WASM, and UEFI.
    -   `tui_renderer`: Knows how to turn text into styled terminal characters, including ASCII and Braille art. Used by TUI.

4.  **Backends**: This is the "body". Each backend is a thin layer responsible for:
    -   Setting up its environment (e.g., creating a window, initializing the terminal).
    -   Translating user input (keystrokes, window resizes) into events for the Core Logic.
    -   Taking the `Renderable` list from the UI layer and using the appropriate Rendering Engine to draw it to the screen.

This separation, enabled by Rust's feature flags, allows for maximum code reuse and makes adding new platforms a structured and manageable task.

## 🚀 Getting Started

### Prerequisites

-   Install the Rust toolchain via [rustup](https://rustup.rs/).
-   For the WASM target, install `wasm-pack`:
    ```bash
    cargo install wasm-pack
    ```

### 1. GUI (Desktop)

Compile and run the native desktop application.

```bash
cargo run --features gui
```

### 2. TUI (Terminal)

Compile and run the terminal-based version.

```bash
cargo run --features tui
```
*Tip: While running, press `Tab` to cycle through the `SimpleText`, `AsciiArt`, and `Braille` rendering modes.*

### 3. WASM (Web Browser)

1.  **Compile to WebAssembly:**
    This command builds the Rust code, generates JavaScript bindings, and places all necessary files in a `pkg` directory.
    ```bash
    wasm-pack build --target web -- --features wasm
    ```

2.  **Start a Local Web Server:**
    From the root of the project directory, run a simple web server. Python is a common way to do this.
    ```bash
    # For Python 3
    python3 -m http.server

    # Or for Python 2
    python -m SimpleHTTPServer
    ```

3.  **Open in Browser:**
    Navigate to `http://localhost:8000` in your web browser.

### 4. UEFI (QEMU)

Running the UEFI version requires an emulator like QEMU. Convenience scripts are provided for Windows users.

1.  **Build the UEFI Application:**
    The provided scripts handle this, but the core command uses cargo to build for a bare-metal target.

2.  **Run in an Emulator:**
    -   **On Windows (PowerShell):**
        ```powershell
        # For QEMU
        .\run_uefi.ps1

        # For Hyper-V
        .\run_uefi_hyperv.ps1
        ```
    -   On other systems, you will need to adapt the script. The process involves creating a FAT filesystem image, copying the EFI application (`.efi` file) to `\EFI\BOOT\BOOTX64.EFI`, and launching it in a QEMU instance with UEFI firmware (OVMF).

## 📜 License

This project is licensed under the **MIT License**. See the `LICENSE` file for details.