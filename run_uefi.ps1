# run_uefi.ps1
# This script builds the UEFI application and launches it in QEMU using a FAT-formatted directory.
# This method does not require Administrator privileges or Hyper-V cmdlets.

param(
    [string]$EfiSourcePath = ".\target\x86_64-unknown-uefi\debug\rust_multibackend_app.efi",
    [string]$ImageDirPathRelative = ".\uefi_image",
    [string]$QemuExe = "C:\qemu\qemu-system-x86_64.exe",
    [string]$OvmfFile = "C:\qemu\OVMF.fd"
)

# --- Step 1: Build the UEFI application ---
Write-Host "Building the UEFI application..."
cargo build --target x86_64-unknown-uefi --no-default-features --features uefi
if ($LASTEXITCODE -ne 0) {
    Write-Error "Cargo build failed. Exiting."
    exit 1
}
if (-not (Test-Path $EfiSourcePath)) {
    Write-Error "Build succeeded, but EFI file not found at $EfiSourcePath. Exiting."
    exit 1
}

# --- Step 2: Prepare the directory for the virtual FAT drive ---
# Resolve to an absolute path for QEMU
$ImageDirPath = Resolve-Path -Path $ImageDirPathRelative

Write-Host "Preparing virtual drive directory at $ImageDirPath..."
# Clean up old directory
if (Test-Path $ImageDirPath) {
    Remove-Item -Recurse -Force $ImageDirPath
}
# Create directory structure for EFI boot
$efiBootPath = Join-Path $ImageDirPath "EFI\BOOT"
New-Item -Path $efiBootPath -ItemType Directory -Force | Out-Null

# Copy and rename the EFI application
$destEfiPath = Join-Path $efiBootPath "BOOTX64.EFI"
Write-Host "Copying EFI file to $destEfiPath..."
Copy-Item -Path $EfiSourcePath -Destination $destEfiPath

# --- Step 3: Launch QEMU ---
Write-Host "Launching QEMU with debug options..."
# Use -drive with fat:rw:<path> to treat a directory as a virtual FAT drive
# Added -d guest_errors for logging and -serial stdio to see serial output
& $QemuExe -pflash $OvmfFile -drive "file=fat:rw:$ImageDirPath" -m 256M -vga std -d guest_errors -serial stdio

Write-Host "QEMU has been closed."