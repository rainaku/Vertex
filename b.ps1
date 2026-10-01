param(
    # Xoa sach thu muc bundle, frontend dist va cache truoc khi build
    [switch]$Clean,
    # Tu dong mo thu muc va chon file setup.exe trong Explorer sau khi build xong
    [switch]$Open,
    # Chay file setup installer ngay sau khi build xong de test cai dat
    [switch]$Run,
    # Build goi cai dat MSI thay vi NSIS exe
    [switch]$Msi,
    # Build ca hai goi NSIS setup exe va MSI
    [switch]$All,
    # Bat che do log chi tiet (verbose)
    [switch]$VerboseLog
)

Set-ExecutionPolicy -ExecutionPolicy Bypass -Scope Process -Force
$ErrorActionPreference = "Continue"

$AppName = "vertex-app"
$WorkspaceRoot = $PSScriptRoot
$FrontendDir = Join-Path $WorkspaceRoot "frontend"
$BundleDir = Join-Path $WorkspaceRoot "target\release\bundle"

Write-Host ""
Write-Host "=================================================" -ForegroundColor DarkCyan
Write-Host "   Vertex - Build Installer (Setup EXE)          " -ForegroundColor Cyan
Write-Host "=================================================" -ForegroundColor DarkCyan

# --- 1. Kiem tra & tat tien trinh cu de tranh loi khoa file (File Lock) ---
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

Write-Host ">>> [1/5] Kiem tra va tat cac tien trinh Vertex cu..." -ForegroundColor Cyan
$running = Get-Process -Name $AppName -ErrorAction SilentlyContinue
if ($running) {
    Write-Host "    Phat hien $AppName dang chay, tien hanh dung..." -ForegroundColor Yellow
    $running | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 300
    try { & taskkill /F /IM "$AppName.exe" /T 2>$null | Out-Null } catch {}
    if (-not (Wait-ProcessExit -Name $AppName -TimeoutMs 5000)) {
        Write-Host "[!] Khong the tat tien trinh $AppName. Vui long tat thu cong truoc khi build!" -ForegroundColor Red
        exit 1
    }
}

# Don dep cac tien trinh WebView2 con sot lai cua Vertex
try {
    Get-CimInstance Win32_Process -Filter "Name like '%webview2%'" -ErrorAction SilentlyContinue |
        Where-Object { $_.CommandLine -like "*vertex*" } |
        ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
} catch {}

# --- 2. Kiem tra cong cu he thong ---
Write-Host ">>> [2/5] Kiem tra moi truong build (Rust, Node, Tauri CLI)..." -ForegroundColor Cyan

$cargoCmd = Get-Command "cargo" -ErrorAction SilentlyContinue
if (-not $cargoCmd) {
    Write-Host "[!] Khong tim thay 'cargo'. Vui long cai dat Rust toolchain tai: https://rustup.rs" -ForegroundColor Red
    exit 1
}

$npmCmd = Get-Command "npm" -ErrorAction SilentlyContinue
if (-not $npmCmd) {
    Write-Host "[!] Khong tim thay 'npm'. Vui long cai dat Node.js tai: https://nodejs.org" -ForegroundColor Red
    exit 1
}

# Kiem tra thu muc node_modules cua frontend
if (-not (Test-Path (Join-Path $FrontendDir "node_modules"))) {
    Write-Host "    Chua co node_modules, dang chay 'npm install' tai frontend..." -ForegroundColor Yellow
    Push-Location $FrontendDir
    npm install
    if ($LASTEXITCODE -ne 0) {
        Write-Host "[!] 'npm install' that bai!" -ForegroundColor Red
        Pop-Location
        exit $LASTEXITCODE
    }
    Pop-Location
} else {
    Write-Host "    Frontend dependencies: OK" -ForegroundColor Green
}

# --- 3. Xu ly tuy chon -Clean ---
if ($Clean) {
    Write-Host ">>> [3/5] Dang don dep du lieu build cu (-Clean)..." -ForegroundColor Cyan
    Remove-Item (Join-Path $FrontendDir "dist") -Recurse -Force -ErrorAction SilentlyContinue
    Remove-Item $BundleDir -Recurse -Force -ErrorAction SilentlyContinue
    cargo clean -p vertex-app
    Write-Host "    Don dep hoan tat." -ForegroundColor Green
} else {
    Write-Host ">>> [3/5] Bo qua buoc Clean (su dung lai cache de build nhanh)." -ForegroundColor DarkGray
}

# --- 4. Tien hanh build installer voi Tauri CLI ---
$bundleTarget = "nsis"
if ($All) {
    $bundleTarget = "nsis,msi"
    Write-Host ">>> [4/5] Bat dau build cac goi: NSIS Setup EXE va MSI..." -ForegroundColor Green
} elseif ($Msi) {
    $bundleTarget = "msi"
    Write-Host ">>> [4/5] Bat dau build goi MSI..." -ForegroundColor Green
} else {
    Write-Host ">>> [4/5] Bat dau build file Setup EXE (NSIS)..." -ForegroundColor Green
}

$tauriArgs = @("build", "--bundles", $bundleTarget)
if ($VerboseLog) {
    $tauriArgs += "--verbose"
}

# Kiem tra uu tien cargo-tauri neu co, nguoc lai dung npx trong frontend
$cargoTauri = Get-Command "cargo-tauri" -ErrorAction SilentlyContinue

$stopwatch = [System.Diagnostics.Stopwatch]::StartNew()

if ($cargoTauri) {
    Write-Host "    Su dung cargo-tauri CLI..." -ForegroundColor DarkGray
    cargo tauri @tauriArgs
} else {
    Write-Host "    Su dung @tauri-apps/cli qua npx..." -ForegroundColor DarkGray
    Push-Location $WorkspaceRoot
    & npx --prefix "$FrontendDir" tauri @tauriArgs
    Pop-Location
}

$buildExitCode = $LASTEXITCODE
$stopwatch.Stop()
$elapsedMinutes = [math]::Floor($stopwatch.Elapsed.TotalMinutes)
$elapsedSeconds = [math]::Round($stopwatch.Elapsed.TotalSeconds % 60, 1)

if ($buildExitCode -ne 0) {
    Write-Host ""
    Write-Host "[!] Build that bai voi ma loi: $buildExitCode" -ForegroundColor Red
    exit $buildExitCode
}

# --- 5. Tong ket va hien thi file ket qua ---
Write-Host ""
Write-Host "=================================================" -ForegroundColor DarkGreen
Write-Host "   BUILD HOAN TAT THANH CONG! ($elapsedMinutes phut $elapsedSeconds giay)" -ForegroundColor Green
Write-Host "=================================================" -ForegroundColor DarkGreen

$nsisDir = Join-Path $BundleDir "nsis"
$msiDir = Join-Path $BundleDir "msi"

$foundInstallers = @()

if (Test-Path $nsisDir) {
    $foundInstallers += Get-ChildItem -Path $nsisDir -Filter "*.exe" -File
}
if (Test-Path $msiDir) {
    $foundInstallers += Get-ChildItem -Path $msiDir -Filter "*.msi" -File
}

if ($foundInstallers.Count -gt 0) {
    Write-Host "`n>>> Danh sach file cai dat da tao:" -ForegroundColor Yellow
    foreach ($file in $foundInstallers) {
        $sizeMb = [math]::Round($file.Length / 1MB, 2)
        $sha256 = (Get-FileHash -Path $file.FullName -Algorithm SHA256).Hash
        
        Write-Host ""
        Write-Host "  [+] Ten file:  $($file.Name)" -ForegroundColor Cyan
        Write-Host "      Dung luong: $sizeMb MB ($($file.Length.ToString('N0')) bytes)" -ForegroundColor White
        Write-Host "      Duong dan:  $($file.FullName)" -ForegroundColor DarkGray
        Write-Host "      SHA-256:    $sha256" -ForegroundColor DarkGray
    }

    # Chon file setup.exe chinh (uu tien file nsis .exe gan nhat)
    $primaryExe = $foundInstallers | Where-Object { $_.Extension -eq ".exe" } | Sort-Object LastWriteTime -Descending | Select-Object -First 1
    if (-not $primaryExe) {
        $primaryExe = $foundInstallers[0]
    }

    # Mo thu muc / Chon file trong Explorer neu co tham so -Open
    if ($Open) {
        Write-Host "`n>>> Dang mo Explorer va chon file setup..." -ForegroundColor Cyan
        explorer.exe /select, "$($primaryExe.FullName)"
    } else {
        Write-Host "`n[Goi y] Ban co the dung: .\b.ps1 -Open  (De tu dong mo thu muc chua file setup)" -ForegroundColor DarkGray
    }

    # Khoi chay installer neu co tham so -Run
    if ($Run) {
        Write-Host ">>> Dang khoi chay bo cai dat: $($primaryExe.Name)..." -ForegroundColor Green
        Start-Process -FilePath $primaryExe.FullName
    } else {
        Write-Host "[Goi y] Ban co the dung: .\b.ps1 -Run   (De tu dong chay thu bo cai dat sau khi build)" -ForegroundColor DarkGray
    }
} else {
    Write-Host "[?] Khong tim thay file installer trong thu muc: $BundleDir" -ForegroundColor Yellow
}

Write-Host ""
