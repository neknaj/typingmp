# Builds the UEFI target and launches it in a Generation 2 Hyper-V VM.

param(
    [string]$EfiSourcePath = ".\target\x86_64-unknown-uefi\debug\rust_multibackend_app.efi",
    [string]$VhdPath = ".\uefi_disk.vhdx",
    [string]$VmName = "UEFI-Test-VM",
    [switch]$DryRun,
    [switch]$KeepResources
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

function Assert-SafeVmName {
    param([Parameter(Mandatory = $true)][string]$Name)

    if ($Name -notmatch '^[A-Za-z0-9._-]+$') {
        throw "VM name may contain only letters, digits, dot, underscore, and hyphen: $Name"
    }
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

Assert-SafeVmName -Name $VmName
$efiSource = Resolve-WorkspacePath -Path $EfiSourcePath -Description "EFI source path"
$vhd = Resolve-WorkspacePath -Path $VhdPath -Description "VHDX path"
if ([System.IO.Path]::GetExtension($vhd) -ne ".vhdx") {
    throw "VHD path must use the .vhdx extension: $vhd"
}

if (-not $DryRun) {
    Write-Host "Checking Administrator privileges and Hyper-V module..."
    $principal = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
    if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw "This script requires Administrator privileges."
    }
    if (-not (Get-Module -ListAvailable -Name Hyper-V)) {
        throw "Hyper-V module is not available."
    }
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

Write-Host "Preparing virtual hard disk at $vhd..."
Invoke-Step "remove existing VM named $VmName and VHDX $vhd" {
    if (Get-VM -Name $VmName -ErrorAction SilentlyContinue) {
        Remove-VM -Name $VmName -Force -ErrorAction Stop
    }
    if (Test-Path -LiteralPath $vhd) {
        Remove-Item -LiteralPath $vhd -Force
    }
}

$mounted = $false
try {
    Invoke-Step "create and mount $vhd" {
        New-VHD -Path $vhd -SizeBytes 256MB | Out-Null
    }

    if (-not $DryRun) {
        $mountedVhd = Mount-VHD -Path $vhd -Passthru
        $mounted = $true
        $disk = $mountedVhd | Get-Disk
        $disk | Initialize-Disk -PartitionStyle GPT
        $partition = $disk | New-Partition -UseMaximumSize -AssignDriveLetter
        $partition | Format-Volume -FileSystem FAT32 | Out-Null
        $driveLetter = $partition.DriveLetter

        $efiBootPath = Join-Path "${driveLetter}:" "EFI\BOOT"
        New-Item -Path $efiBootPath -ItemType Directory -Force | Out-Null
        Copy-Item -LiteralPath $efiSource -Destination (Join-Path $efiBootPath "BOOTX64.EFI") -Force
        Dismount-VHD -Path $vhd
        $mounted = $false
    }
} finally {
    if ($mounted -and -not $DryRun) {
        Dismount-VHD -Path $vhd -ErrorAction SilentlyContinue
    }
}

Write-Host "Creating and launching Hyper-V VM: $VmName..."
if ($DryRun) {
    Write-Host "[dry-run] create Generation 2 VM using $vhd"
} else {
    $switchName = "Default Switch"
    $switch = Get-VMSwitch -Name $switchName -ErrorAction SilentlyContinue
    if ($null -eq $switch) {
        New-VM -Name $VmName -Generation 2 -MemoryStartupBytes 256MB -VHDPath $vhd | Out-Null
    } else {
        New-VM -Name $VmName -Generation 2 -MemoryStartupBytes 256MB -VHDPath $vhd -SwitchName $switchName | Out-Null
    }

    Set-VMFirmware -VMName $VmName -EnableSecureBoot Off
    Start-VM -Name $VmName

    try {
        vmconnect.exe $env:COMPUTERNAME $VmName
    } catch {
        Write-Warning "Could not open Virtual Machine Connection. Open it manually from Hyper-V Manager."
    }
}

if ($DryRun -or $KeepResources) {
    Write-Host "Resources were left in place."
    exit 0
}

Write-Host "Press any key to stop and clean up the VM and VHDX."
$host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown") | Out-Null

Write-Host "Stopping and cleaning up resources..."
Stop-VM -Name $VmName -Force -ErrorAction SilentlyContinue
Remove-VM -Name $VmName -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath $vhd -Force
Write-Host "Cleanup complete."
