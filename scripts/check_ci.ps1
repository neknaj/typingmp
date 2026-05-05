param(
    [switch]$SkipNode,
    [switch]$SkipTargets
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Invoke-Step {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][scriptblock]$Command
    )

    Write-Host "==> $Name"
    & $Command
}

function Invoke-Native {
    param(
        [Parameter(Mandatory = $true)][string]$File,
        [Parameter(Mandatory = $true)][string[]]$Arguments
    )

    & $File @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw ("{0} {1} failed with exit code {2}" -f $File, ($Arguments -join " "), $LASTEXITCODE)
    }
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
Set-Location $repoRoot

Invoke-Step "text encoding" {
    & (Join-Path $PSScriptRoot "check_text_encoding.ps1")
    if ($LASTEXITCODE -ne 0) {
        throw "text encoding check failed"
    }
}

if (-not $SkipNode) {
    Invoke-Step "dev server tests" {
        Invoke-Native "npm" @("run", "test:serve")
    }
}

Invoke-Step "cargo fmt" {
    Invoke-Native "cargo" @("fmt", "--check")
}

Invoke-Step "cargo test" {
    Invoke-Native "cargo" @("test", "--no-default-features")
}

Invoke-Step "core clippy" {
    Invoke-Native "cargo" @("clippy", "--no-default-features", "--all-targets", "--", "-D", "warnings")
}

Invoke-Step "tui clippy" {
    Invoke-Native "cargo" @("clippy", "--no-default-features", "--features", "tui", "--all-targets", "--", "-D", "warnings")
}

Invoke-Step "gui clippy" {
    Invoke-Native "cargo" @("clippy", "--no-default-features", "--features", "gui", "--all-targets", "--", "-D", "warnings")
}

Invoke-Step "mobile clippy" {
    Invoke-Native "cargo" @("clippy", "--no-default-features", "--features", "mobile", "--all-targets", "--", "-D", "warnings")
}

if (-not $SkipTargets) {
    Invoke-Step "install cross targets" {
        Invoke-Native "rustup" @("target", "add", "wasm32-unknown-unknown", "x86_64-unknown-uefi")
    }

    Invoke-Step "wasm debug check" {
        Invoke-Native "cargo" @("check", "--target", "wasm32-unknown-unknown", "--no-default-features", "--features", "wasm")
    }

    Invoke-Step "wasm release check" {
        Invoke-Native "cargo" @("check", "--release", "--target", "wasm32-unknown-unknown", "--no-default-features", "--features", "wasm")
    }

    Invoke-Step "uefi no_std check" {
        Invoke-Native "cargo" @("check", "--target", "x86_64-unknown-uefi", "--no-default-features", "--features", "uefi")
    }
}
