# Installs the latest druk release binary for Windows, verifying its
# checksum against the SHA256SUMS.txt published alongside it. No local
# LLVM or Rust toolchain is needed: this only downloads the prebuilt
# JIT-capable binary (see README.md's "Installing a prebuilt binary"
# section for what druk build additionally needs).
#
# Usage: irm https://raw.githubusercontent.com/Etoile-Bleu/druk-lang-v2/main/install.ps1 | iex
$ErrorActionPreference = "Stop"

$repo = "Etoile-Bleu/druk-lang-v2"
$installDir = if ($env:DRUK_INSTALL_DIR) { $env:DRUK_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA "druk\bin" }

if (-not [Environment]::Is64BitOperatingSystem) {
    Write-Error "druk currently ships 64-bit Windows binaries only."
    exit 1
}

$asset = "druk-windows-x86_64.exe"
$baseUrl = "https://github.com/$repo/releases/latest/download"

$tmpDir = Join-Path $env:TEMP ([System.Guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $tmpDir | Out-Null

try {
    $exePath = Join-Path $tmpDir $asset
    $sumsPath = Join-Path $tmpDir "SHA256SUMS.txt"

    Write-Host "Downloading $asset..."
    Invoke-WebRequest -Uri "$baseUrl/$asset" -OutFile $exePath
    Invoke-WebRequest -Uri "$baseUrl/SHA256SUMS.txt" -OutFile $sumsPath

    Write-Host "Verifying checksum..."
    $expectedLine = Select-String -Path $sumsPath -SimpleMatch -Pattern $asset | Select-Object -First 1
    if (-not $expectedLine) {
        Write-Error "$asset is not listed in SHA256SUMS.txt; refusing to install an unverified binary."
        exit 1
    }
    $expectedHash = ($expectedLine.Line -split '\s+')[0].ToLower()
    $actualHash = (Get-FileHash -Path $exePath -Algorithm SHA256).Hash.ToLower()
    if ($actualHash -ne $expectedHash) {
        Write-Error "Checksum mismatch for $asset (expected $expectedHash, got $actualHash); refusing to install it."
        exit 1
    }

    New-Item -ItemType Directory -Force -Path $installDir | Out-Null
    Move-Item -Force $exePath (Join-Path $installDir "druk.exe")

    Write-Host "druk installed to $installDir\druk.exe"

    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ($userPath -notlike "*$installDir*") {
        [Environment]::SetEnvironmentVariable("Path", "$userPath;$installDir", "User")
        Write-Host ""
        Write-Host "Added $installDir to your user PATH. Open a new terminal for this to take effect."
    }

    Write-Host ""
    Write-Host "Run 'druk --version' in a new terminal to confirm the install."
}
finally {
    Remove-Item -Recurse -Force $tmpDir -ErrorAction SilentlyContinue
}
