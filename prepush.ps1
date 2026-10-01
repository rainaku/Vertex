# Pre-push CI verification script for Vertex
# Mirrors .github/workflows/ci.yml locally

param(
    [switch]$FixFormat,
    [switch]$SkipAudit
)

$ErrorActionPreference = "Stop"
Set-ExecutionPolicy -ExecutionPolicy Bypass -Scope Process -Force

$WorkspaceRoot = $PSScriptRoot
$FrontendDir = Join-Path $WorkspaceRoot "frontend"

Write-Host ""
Write-Host "=================================================" -ForegroundColor DarkCyan
Write-Host "   Vertex - Pre-push CI Verification             " -ForegroundColor Cyan
Write-Host "=================================================" -ForegroundColor DarkCyan

# --- 1. Kiem tra va restore dependencies ---
Write-Host "`n=== [1/5] Kiem tra Dependencies & Cargo Check ===" -ForegroundColor Cyan
Push-Location $WorkspaceRoot
cargo check --workspace
if ($LASTEXITCODE -ne 0) {
    Write-Host "[!] Cargo check that bai!" -ForegroundColor Red
    Pop-Location
    exit 1
}

if (-not (Test-Path (Join-Path $FrontendDir "node_modules"))) {
    Write-Host ">>> Dang cai dat npm packages cho frontend..." -ForegroundColor Yellow
    Push-Location $FrontendDir
    npm install
    Pop-Location
}
Pop-Location
Write-Host "Dependencies OK." -ForegroundColor Green

# --- 2. Kiem tra dinh dang code (Format) & Typecheck ---
Write-Host "`n=== [2/5] Kiem tra Code Formatting & Frontend Check ===" -ForegroundColor Cyan
if ($FixFormat) {
    Write-Host ">>> Dang tu dong format Rust code (-FixFormat)..." -ForegroundColor Yellow
    cargo fmt --all
}

cargo fmt --all -- --check
if ($LASTEXITCODE -ne 0) {
    Write-Host "`n[!] Rust code format verification that bai!" -ForegroundColor Red
    Write-Host "[*] Chay 'cargo fmt --all' hoac '.\prepush.ps1 -FixFormat' de tu dong sua format." -ForegroundColor Yellow
    exit 1
}
Write-Host "Rust format OK." -ForegroundColor Green

Write-Host ">>> Dang kiem tra TypeScript/Svelte types trong frontend..." -ForegroundColor DarkGray
Push-Location $FrontendDir
npm run check
if ($LASTEXITCODE -ne 0) {
    Write-Host "[!] Frontend check that bai!" -ForegroundColor Red
    Pop-Location
    exit 1
}
Pop-Location
Write-Host "Frontend check OK." -ForegroundColor Green

# --- 3. Clippy Linter voi -D warnings ---
Write-Host "`n=== [3/5] Kiem tra Rust Clippy (Zero Warnings) ===" -ForegroundColor Cyan
cargo clippy --workspace --all-targets -- -D warnings
if ($LASTEXITCODE -ne 0) {
    Write-Host "`n[!] Clippy phat hien warning hoac error!" -ForegroundColor Red
    exit 1
}
Write-Host "Clippy passed with 0 warnings." -ForegroundColor Green

# --- 4. Chay toan bo Unit & Integration Tests ---
Write-Host "`n=== [4/5] Chay toan bo Tests trong Workspace ===" -ForegroundColor Cyan
cargo test --workspace
if ($LASTEXITCODE -ne 0) {
    Write-Host "`n[!] Mot so bai test bi that bai!" -ForegroundColor Red
    exit 1
}
Write-Host "All tests passed." -ForegroundColor Green

# --- 5. Kiem tra lo hong bao mat cac thu vien (Security Audit) ---
Write-Host "`n=== [5/5] Quet lo hong bao mat Dependencies (Audit) ===" -ForegroundColor Cyan

if (-not $SkipAudit) {
    # NPM audit
    Write-Host ">>> Quet lo hong npm packages..." -ForegroundColor DarkGray
    Push-Location $FrontendDir
    npm audit --audit-level=high
    if ($LASTEXITCODE -ne 0) {
        Write-Host "[!] Phat hien lo hong bao mat muc high/critical trong npm dependencies!" -ForegroundColor Red
        Pop-Location
        exit 1
    }
    Pop-Location

    # Cargo audit
    $cargoAuditCmd = Get-Command "cargo-audit" -ErrorAction SilentlyContinue
    if ($cargoAuditCmd) {
        Write-Host ">>> Quet lo hong crates bang cargo-audit..." -ForegroundColor DarkGray
        cargo audit
        if ($LASTEXITCODE -ne 0) {
            Write-Host "[!] Phat hien lo hong bao mat trong Rust crates!" -ForegroundColor Red
            exit 1
        }
    } else {
        Write-Host "    [i] Chua cai 'cargo-audit'. Ban co the cai dat bang: cargo install cargo-audit" -ForegroundColor DarkGray
    }
    Write-Host "Audit check passed." -ForegroundColor Green
} else {
    Write-Host "    Bo qua Audit (-SkipAudit)." -ForegroundColor Yellow
}

Write-Host "`n=======================================================" -ForegroundColor DarkGreen
Write-Host " TAT CA CAC BUOC KIEM TRA PRE-PUSH DA THANH CONG!     " -ForegroundColor Green
Write-Host " An toan de commit & push len remote GitHub.          " -ForegroundColor Green
Write-Host "=======================================================`n" -ForegroundColor DarkGreen
exit 0
