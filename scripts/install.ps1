# Bellona installer (Windows). Requires Rust 1.85+ (rustup).
$ErrorActionPreference = "Stop"

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Error "cargo not found. Install via https://rustup.rs first."
}

Write-Host "[bellona] building the war machine (release)..." -ForegroundColor Cyan
Push-Location $PSScriptRoot/..
try {
    cargo build --release
    if ($LASTEXITCODE -ne 0) { Write-Error "build failed" }
} finally {
    Pop-Location
}

Write-Host "[bellona] running doctrine tests..." -ForegroundColor Cyan
Push-Location $PSScriptRoot/..
try {
    cargo test --workspace --quiet
} finally {
    Pop-Location
}

Write-Host ""
Write-Host "Bellona is forged. Next:" -ForegroundColor Green
Write-Host "  cd $(Split-Path $PSScriptRoot/..)"
Write-Host "  See BELLONA.md for standing orders."
