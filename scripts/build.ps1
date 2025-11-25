# Build script for creating distributable ZenSignal binaries on Windows
# Location: scripts\build.ps1
# Usage: .\scripts\build.ps1

$ErrorActionPreference = "Stop"

Write-Host "ZenSignal Build Script for Windows" -ForegroundColor Green
Write-Host "======================================" -ForegroundColor Green
Write-Host ""

# Check if cargo is installed
if (!(Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "Error: Cargo not found" -ForegroundColor Red
    Write-Host "Install Rust from: https://rustup.rs/"
    exit 1
}

# Check if target is installed
$target = "x86_64-pc-windows-msvc"
$targetList = rustup target list | Out-String

if ($targetList -notmatch "$target \(installed\)") {
    Write-Host "Installing $target target..." -ForegroundColor Yellow
    rustup target add $target
}

Write-Host "Building for Windows (x86_64-msvc)..." -ForegroundColor Yellow
cargo build --release --target $target

if ($LASTEXITCODE -ne 0) {
    Write-Host "Build failed!" -ForegroundColor Red
    exit $LASTEXITCODE
}

# Create release directory
$releaseDir = "release"
if (!(Test-Path $releaseDir)) {
    New-Item -ItemType Directory -Path $releaseDir | Out-Null
}

Write-Host "Creating archive..." -ForegroundColor Yellow
$binaryPath = "target\$target\release\zen-signal.exe"
$archivePath = "$releaseDir\zen-signal-windows-x86_64.zip"

if (Test-Path $archivePath) {
    Remove-Item $archivePath
}

Compress-Archive -Path $binaryPath -DestinationPath $archivePath

Write-Host ""
Write-Host "Windows binary created:" -ForegroundColor Green
Write-Host "  - $archivePath"

# Generate checksum
Write-Host ""
Write-Host "Generating checksum..." -ForegroundColor Yellow
$hash = Get-FileHash -Algorithm SHA256 -Path $archivePath
$checksumPath = "$releaseDir\checksums.txt"
"$($hash.Hash.ToLower())  $(Split-Path $archivePath -Leaf)" | Out-File -FilePath $checksumPath -Encoding ASCII

Write-Host "Checksum created: $checksumPath" -ForegroundColor Green

# Print summary
Write-Host ""
Write-Host "======================================" -ForegroundColor Green
Write-Host "Build completed successfully!" -ForegroundColor Green
Write-Host ""
Write-Host "Release files:"
Get-ChildItem $releaseDir | Format-Table Name, Length, LastWriteTime
Write-Host ""
Write-Host "Next steps:" -ForegroundColor Yellow
Write-Host "1. Test the binary by extracting and running zen-signal.exe"
Write-Host "2. Create a git tag: git tag v0.1.0"
Write-Host "3. Push the tag: git push origin v0.1.0"
Write-Host "4. Or create a manual release with these files"
Write-Host ""
Write-Host "See BUILDING.md for more details."
