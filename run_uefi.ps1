# Builds the UEFI target and launches it in QEMU with a directory-backed FAT drive.

param(
    [string]$EfiSourcePath = ".\target\x86_64-unknown-uefi\debug\rust_multibackend_app.efi",
    [string]$ImageDirPath = ".\uefi_image",
    [string]$QemuExe = "C:\qemu\qemu-system-x86_64.exe",
    [string]$OvmfFile = "C:\qemu\OVMF.fd",
    [switch]$DryRun
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$WorkspaceRoot = [System.IO.Path]::GetFullPath($PSScriptRoot)

function Resolve-WorkspacePath {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Description,
        [switch]$MustExist
    )

    $candidate = if ([System.IO.Path]::IsPathRooted($Path)) {
        $Path
    } else {
        Join-Path $WorkspaceRoot $Path
    }
    $resolved = [System.IO.Path]::GetFullPath($candidate)
    $rootWithSeparator = $WorkspaceRoot.TrimEnd('\', '/') + [System.IO.Path]::DirectorySeparatorChar
    if ($resolved -ne $WorkspaceRoot -and -not $resolved.StartsWith($rootWithSeparator, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "$Description must stay inside the workspace: $resolved"
    }
    if ($MustExist -and -not (Test-Path -LiteralPath $resolved)) {
        throw "$Description does not exist: $resolved"
    }
    return $resolved
}

function Resolve-ExistingFile {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Description
    )

    $resolved = [System.IO.Path]::GetFullPath($Path)
    if (-not (Test-Path -LiteralPath $resolved -PathType Leaf)) {
        throw "$Description does not exist: $resolved"
    }
    return $resolved
}

function Invoke-Step {
    param(
        [Parameter(Mandatory = $true)][string]$Description,
        [Parameter(Mandatory = $true)][scriptblock]$Action
    )

    if ($DryRun) {
        Write-Host "[dry-run] $Description"
    } else {
        & $Action
    }
}

$efiSource = Resolve-WorkspacePath -Path $EfiSourcePath -Description "EFI source path"
$imageDir = Resolve-WorkspacePath -Path $ImageDirPath -Description "QEMU FAT image directory"
if ($imageDir -eq $WorkspaceRoot) {
    throw "QEMU FAT image directory must not be the workspace root."
}

Write-Host "Building the UEFI application..."
Invoke-Step "cargo build --target x86_64-unknown-uefi --no-default-features --features uefi" {
    cargo build --target x86_64-unknown-uefi --no-default-features --features uefi
    if ($LASTEXITCODE -ne 0) {
        throw "Cargo build failed."
    }
}

if (-not $DryRun -and -not (Test-Path -LiteralPath $efiSource -PathType Leaf)) {
    throw "Build succeeded, but EFI file was not found: $efiSource"
}

Write-Host "Preparing virtual drive directory at $imageDir..."
Invoke-Step "remove and recreate $imageDir" {
    if (Test-Path -LiteralPath $imageDir) {
        Remove-Item -LiteralPath $imageDir -Recurse -Force
    }
    New-Item -Path (Join-Path $imageDir "EFI\BOOT") -ItemType Directory -Force | Out-Null
}

$efiBootPath = Join-Path $imageDir "EFI\BOOT"
$destEfiPath = Join-Path $efiBootPath "BOOTX64.EFI"
Invoke-Step "copy $efiSource to $destEfiPath" {
    Copy-Item -LiteralPath $efiSource -Destination $destEfiPath -Force
}

if (-not $DryRun) {
    $qemu = Resolve-ExistingFile -Path $QemuExe -Description "QEMU executable"
    $ovmf = Resolve-ExistingFile -Path $OvmfFile -Description "OVMF firmware"
    Write-Host "Launching QEMU..."
    & $qemu -pflash $ovmf -drive "file=fat:rw:$imageDir" -m 256M -vga std -d guest_errors -serial stdio
    Write-Host "QEMU has been closed."
} else {
    Write-Host "[dry-run] launch QEMU with image directory $imageDir"
}
