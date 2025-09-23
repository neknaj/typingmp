# run_uefi_hyperv.ps1
# This script builds the UEFI application and launches it in a Hyper-V virtual machine.
# This method requires Administrator privileges and the Hyper-V module.

param(
    [string]$EfiSourcePath = ".\target\x86_64-unknown-uefi\debug\rust_multibackend_app.efi",
    [string]$VhdPath = ".\uefi_disk.vhdx",
    [string]$VmName = "UEFI-Test-VM"
)

# --- Step 0: Prerequisites Check ---
Write-Host "Checking for Administrator privileges and Hyper-V module..."

# Administrator権限の確認
if (-NOT ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Write-Error "This script requires Administrator privileges. Please run it as an Administrator."
    exit 1
}

# Hyper-Vモジュールの確認
if (-NOT (Get-Module -ListAvailable -Name Hyper-V)) {
    Write-Error "Hyper-V module is not available. Please ensure Hyper-V is enabled on this system."
    exit 1
}

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

# --- Step 2: Prepare the virtual hard disk ---
Write-Host "Preparing virtual hard disk at $VhdPath..."

# クリーンアップ: 既存のVMとVHDXを削除
if (Get-VM -Name $VmName -ErrorAction SilentlyContinue) {
    Write-Host "Removing existing VM: $VmName"
    Remove-VM -Name $VmName -Force -ErrorAction SilentlyContinue
}
if (Test-Path $VhdPath) {
    Write-Host "Removing existing VHDX: $VhdPath"
    Remove-Item -Path $VhdPath -Force
}

# 新しいVHDXを作成 (サイズ: 256MB)
New-VHD -Path $VhdPath -SizeBytes 256MB

# VHDXをマウントしてディスク情報を取得
$disk = Mount-VHD -Path $VhdPath -Passthru | Get-Disk

# ディスクを初期化し、パーティションを作成してフォーマット
$disk | Initialize-Disk -PartitionStyle GPT
$partition = $disk | New-Partition -UseMaximumSize -AssignDriveLetter
$volume = $partition | Format-Volume -FileSystem FAT32
$driveLetter = $partition.DriveLetter

# EFIブート用のディレクトリ構造を作成
$efiBootPath = Join-Path "${driveLetter}:" "EFI\BOOT"
New-Item -Path $efiBootPath -ItemType Directory -Force | Out-Null

# EFIアプリケーションをコピーして名前を変更
$destEfiPath = Join-Path $efiBootPath "BOOTX64.EFI"
Write-Host "Copying EFI file to $destEfiPath..."
Copy-Item -Path $EfiSourcePath -Destination $destEfiPath

# VHDXをアンマウント
Dismount-VHD -Path $VhdPath

# --- Step 3: Create and Launch the Hyper-V VM ---
Write-Host "Creating and launching Hyper-V VM: $VmName..."

# 仮想スイッチの確認または作成
$switchName = "Default Switch"
if (-not (Get-VMSwitch -Name $switchName -ErrorAction SilentlyContinue)) {
    Write-Host "Default Switch not found. The VM will be created without a network adapter."
    $switchName = $null
}

# 第2世代VM (UEFI) を作成
New-VM -Name $VmName -Generation 2 -MemoryStartupBytes 256MB -VHDPath $VhdPath -SwitchName $switchName

# セキュアブートを無効化
Set-VMFirmware -VMName $VmName -EnableSecureBoot Off

# VMを起動
Start-VM -Name $VmName

# VM接続を開く
Write-Host "Launching Virtual Machine Connection..."
try {
    vmconnect.exe $env:COMPUTERNAME $VmName
}
catch {
    Write-Warning "Could not automatically open Virtual Machine Connection. Please open it manually from Hyper-V Manager."
}


Write-Host "VM has been started. Press any key to stop and clean up the VM and VHDX."
$host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown") | Out-Null


# --- Step 4: Cleanup ---
Write-Host "Stopping and cleaning up resources..."
Stop-VM -Name $VmName -Force -ErrorAction SilentlyContinue
Remove-VM -Name $VmName -Force -ErrorAction SilentlyContinue
Remove-Item -Path $VhdPath -Force

Write-Host "Cleanup complete."