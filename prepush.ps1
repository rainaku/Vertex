param([switch]$FixFormat, [switch]$SkipAudit)

$ErrorActionPreference = 'Stop'
$WorkspaceRoot = $PSScriptRoot
$FrontendDir = Join-Path $WorkspaceRoot 'frontend'
$vertexExitCode = 0

function Invoke-VertexCheck {
    param([string]$Label, [scriptblock]$Command)
    Write-Host $Label -ForegroundColor Cyan
    $global:LASTEXITCODE = 0
    & $Command
    if ($LASTEXITCODE -ne 0) { throw "$Label failed (exit $LASTEXITCODE)" }
}

Push-Location $WorkspaceRoot
try {
    Invoke-VertexCheck 'Cargo check' { cargo check --workspace --locked }
    if ($FixFormat) { Invoke-VertexCheck 'Rust format' { cargo fmt --all } }
    Invoke-VertexCheck 'Verify Rust formatting' { cargo fmt --all -- --check }
    Invoke-VertexCheck 'Verify scripts' { & (Join-Path $WorkspaceRoot 'tests\verify_scripts.ps1') }
    Push-Location $FrontendDir
    try {
        if (-not (Test-Path -LiteralPath 'node_modules')) { Invoke-VertexCheck 'Install frontend dependencies' { npm ci } }
        Invoke-VertexCheck 'Frontend types' { npm run check }
        Invoke-VertexCheck 'Frontend regression tests' { npm test }
        Invoke-VertexCheck 'Frontend production build' { npm run build }
        if (-not $SkipAudit) { Invoke-VertexCheck 'NPM audit' { npm audit --audit-level=high } }
    } finally { Pop-Location }
    Invoke-VertexCheck 'Clippy' { cargo clippy --workspace --all-targets --locked -- -D warnings }
    Invoke-VertexCheck 'Rust regression tests' { cargo test --workspace --locked }
    if (-not $SkipAudit) {
        if (Get-Command cargo-audit -ErrorAction SilentlyContinue) { Invoke-VertexCheck 'Cargo audit' { cargo audit } }
        else { Write-Host 'cargo-audit is unavailable; Rust dependency audit was skipped.' -ForegroundColor Yellow }
    }
    Write-Host 'All requested checks passed.' -ForegroundColor Green
} catch {
    Write-Error $_ -ErrorAction Continue
    $vertexExitCode = 1
} finally { Pop-Location }
exit $vertexExitCode
