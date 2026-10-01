param(
    # Chay ban Release toi uu hoa doc lap (khong can Vite dev server)
    [switch]$Release,
    # Xoa sach cache build de compile lai tu dau
    [switch]$Clean,
    # Chay toan bo Unit & Integration tests
    [switch]$Test,
    # Chi build ma khong khoi chay ung dung
    [switch]$BuildOnly
)

Set-ExecutionPolicy -ExecutionPolicy Bypass -Scope Process -Force
$ErrorActionPreference = "Continue"

$AppName = "vertex-app"
$WorkspaceRoot = $PSScriptRoot
$FrontendDir = Join-Path $WorkspaceRoot "frontend"

Write-Host ""
Write-Host "=================================================" -ForegroundColor DarkYellow
Write-Host "   Vertex - Radial Wheel File Converter         " -ForegroundColor Yellow
Write-Host "=================================================" -ForegroundColor DarkYellow

# --- 1. Dung cac tien trinh cu va doi giai phong file lock ---
function Wait-ProcessExit {
    param([string]$Name, [int]$TimeoutMs = 5000)
    $deadline = (Get-Date).AddMilliseconds($TimeoutMs)
    while ((Get-Date) -lt $deadline) {
        $alive = Get-Process -Name $Name -ErrorAction SilentlyContinue
        if (-not $alive) { return $true }
        Start-Sleep -Milliseconds 200
    }
    return $false
}

Write-Host ">>> [1/4] Kiem tra va dung tien trinh $AppName cu..." -ForegroundColor Cyan
$running = Get-Process -Name $AppName -ErrorAction SilentlyContinue
if ($running) {
    $running | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 300
    try { & taskkill /F /IM "$AppName.exe" /T 2>$null | Out-Null } catch {}
    if (-not (Wait-ProcessExit -Name $AppName -TimeoutMs 5000)) {
        Write-Host "[!] Khong the tat tien trinh $AppName dang chay." -ForegroundColor Red
        exit 1
    }
}
# Don dep cac tien trinh WebView2 cu de tranh loi khoa thu muc du lieu
try {
    Get-CimInstance Win32_Process -Filter "Name like '%webview2%'" -ErrorAction SilentlyContinue |
        Where-Object { $_.CommandLine -like "*vertex*" } |
        ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
} catch {}
Start-Sleep -Milliseconds 500

# --- 2. Kiem tra & cai dat Frontend Dependencies ---
if (-not (Test-Path (Join-Path $FrontendDir "node_modules"))) {
    Write-Host ">>> [2/4] Cai dat frontend dependencies (npm install)..." -ForegroundColor Yellow
    Push-Location $FrontendDir
    npm install
    Pop-Location
} else {
    Write-Host ">>> [2/4] Frontend dependencies OK." -ForegroundColor Green
}

# --- 3. Xu ly co -Clean va -Test ---
if ($Clean) {
    Write-Host ">>> Dang don dep thu muc build (-Clean)..." -ForegroundColor Cyan
    Remove-Item (Join-Path $FrontendDir "dist") -Recurse -Force -ErrorAction SilentlyContinue
    cargo clean -p vertex-app
}

if ($Test) {
    Write-Host ">>> Dang chay toan bo test workspace..." -ForegroundColor Cyan
    cargo test --workspace
    exit $LASTEXITCODE
}

# --- 4. Build Frontend & Chuan bi moi truong ---
Write-Host ">>> [3/4] Build goi giao dien frontend (Vite)..." -ForegroundColor Cyan
Push-Location $FrontendDir
npm run build
if ($LASTEXITCODE -ne 0) {
    Write-Host "[!] Build frontend that bai!" -ForegroundColor Red
    Pop-Location
    exit $LASTEXITCODE
}
Pop-Location

# --- 5. Bien dich & Khoi chay ung dung ---
$viteProcess = $null

try {
    if ($Release) {
        Write-Host ">>> [4/4] Bien dich & Khoi chay Vertex (Ban Release)..." -ForegroundColor Green
        cargo build --release -p vertex-app
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

        if (-not $BuildOnly) {
            $exePath = Join-Path $WorkspaceRoot "target\release\$AppName.exe"
            Write-Host ">>> Khởi chạy: $exePath" -ForegroundColor DarkGray
            $proc = Start-Process -FilePath $exePath -PassThru
            $proc | Wait-Process
        }
    } else {
        Write-Host ">>> [4/4] Khoi dong Vite Dev Server & Cargo Dev..." -ForegroundColor Green
        
        # Kiem tra xem port 5173 da co Vite chay chua
        $portOpen = (Test-NetConnection -ComputerName "localhost" -Port 5173 -InformationLevel Quiet -WarningAction SilentlyContinue)
        if (-not $portOpen) {
            Write-Host ">>> Khoi dong Vite dev server trong nen..." -ForegroundColor DarkGray
            $viteProcess = Start-Process -FilePath "npm.cmd" -ArgumentList "run dev" -WorkingDirectory $FrontendDir -PassThru -WindowStyle Hidden
            Start-Sleep -Seconds 2
        }

        if (-not $BuildOnly) {
            cargo run -p vertex-app
        } else {
            cargo build -p vertex-app
        }
    }
}
finally {
    if ($viteProcess -and -not $viteProcess.HasExited) {
        Write-Host "`n>>> Dang dong Vite dev server..." -ForegroundColor DarkGray
        Stop-Process -Id $viteProcess.Id -Force -ErrorAction SilentlyContinue
    }
    Stop-Process -Name $AppName -Force -ErrorAction SilentlyContinue
}
