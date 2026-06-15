$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$utf8Strict = [System.Text.UTF8Encoding]::new($false, $true)
$mojibakeChars = @(
    0x7E3A, 0x7E67, 0x8B41, 0x9B2E, 0x9AEF, 0x9A5B, 0x90E2, 0x873F,
    0x86F9, 0x9677, 0x7E5D, 0x9B2F, 0x9A4D, 0x9AEB, 0x964B, 0x96B4,
    0x8B4E, 0x965D, 0x95D5, 0x96B0, 0x90B5, 0x9AE2, 0x9B28, 0x95D6,
    0x95D4, 0x965F, 0x9076, 0x87F6, 0x8B28, 0x96DC, 0x9AF4, 0x9AE3,
    0x9A57
) | ForEach-Object { [regex]::Escape([string][char]$_) }
$mojibakePattern = [regex]::new(($mojibakeChars -join "|"))

$textExtensions = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
@(
    ".css",
    ".gitignore",
    ".html",
    ".js",
    ".json",
    ".lock",
    ".md",
    ".ntq",
    ".ps1",
    ".py",
    ".rs",
    ".slint",
    ".toml",
    ".txt",
    ".yaml",
    ".yml"
) | ForEach-Object { [void]$textExtensions.Add($_) }

function Test-TextPath {
    param([Parameter(Mandatory = $true)][string]$Path)

    $name = [System.IO.Path]::GetFileName($Path)
    if ($textExtensions.Contains($name)) {
        return $true
    }

    return $textExtensions.Contains([System.IO.Path]::GetExtension($Path))
}

$rawFiles = & git -C $repoRoot ls-files -z
if ($LASTEXITCODE -ne 0) {
    throw "git ls-files failed"
}

$errors = [System.Collections.Generic.List[string]]::new()
$files = $rawFiles -split "`0" | Where-Object { $_ -ne "" }

foreach ($relativePath in $files) {
    if (-not (Test-TextPath -Path $relativePath)) {
        continue
    }

    $path = Join-Path $repoRoot $relativePath
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        continue
    }

    $bytes = [System.IO.File]::ReadAllBytes($path)
    try {
        $content = $utf8Strict.GetString($bytes)
    } catch {
        $errors.Add(("{0}: invalid UTF-8 byte sequence" -f $relativePath))
        continue
    }

    $match = $mojibakePattern.Match($content)
    if ($match.Success) {
        $line = ($content.Substring(0, $match.Index) -split "`r?`n").Count
        $errors.Add(("{0}:{1}: mojibake-looking text near '{2}'" -f $relativePath, $line, $match.Value))
    }
}

if ($errors.Count -gt 0) {
    $errors | ForEach-Object { Write-Error $_ }
    exit 1
}

Write-Host "All tracked text files are strict UTF-8 and no mojibake-looking text was found."
