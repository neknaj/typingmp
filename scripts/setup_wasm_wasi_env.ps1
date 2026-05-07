$ErrorActionPreference = "Stop"

rustup target add wasm32-unknown-unknown wasm32-wasip1 wasm32-wasip2

$runtimePaths = @(
    "C:\Program Files\Wasmtime\bin",
    "C:\Program Files (x86)\Wasmer\bin",
    "C:\Program Files (x86)\Wasmer\globals\wapm_packages\.bin"
)

foreach ($path in $runtimePaths) {
    if ((Test-Path -LiteralPath $path) -and (($env:Path -split ";") -notcontains $path)) {
        $env:Path = "$env:Path;$path"
    }
}

if (-not (Get-Command wasm-pack -ErrorAction SilentlyContinue)) {
    cargo install wasm-pack --locked
}

if (-not (Get-Command cargo-wasix -ErrorAction SilentlyContinue)) {
    cargo install cargo-wasix
}

if (-not (Get-Command wasmtime -ErrorAction SilentlyContinue)) {
    winget install --exact --id BytecodeAlliance.Wasmtime --accept-package-agreements --accept-source-agreements
}

if (-not (Get-Command wasmer -ErrorAction SilentlyContinue)) {
    winget install --exact --id Wasmer.Wasmer --accept-package-agreements --accept-source-agreements
}

foreach ($path in $runtimePaths) {
    if ((Test-Path -LiteralPath $path) -and (($env:Path -split ";") -notcontains $path)) {
        $env:Path = "$env:Path;$path"
    }
}

Write-Host "Rust WASM targets:"
rustup target list --installed | Select-String "wasm32"

Write-Host "Runtime versions:"
wasm-pack --version
cargo wasix --version
wasmtime --version
wasmer --version

Write-Host "WASIX check:"
cargo wasix check --no-default-features --features wasi-tui --bin rust_multibackend_app
