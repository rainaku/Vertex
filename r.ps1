param(
    [switch]$Release,
    [switch]$Clean,
    [switch]$Test,
    [switch]$BuildOnly
)

$ErrorActionPreference = 'Stop'
$WorkspaceRoot = $PSScriptRoot
$FrontendDir = Join-Path $WorkspaceRoot 'frontend'
. (Join-Path $WorkspaceRoot 'scripts\BuildTools.ps1')

$viteProcess = $null
$process = $null
$vertexExitCode = 0
Push-Location $WorkspaceRoot
try {
    if (-not (Test-Path -LiteralPath (Join-Path $FrontendDir 'node_modules'))) {
        Push-Location $FrontendDir
        try { npm ci; if ($LASTEXITCODE -ne 0) { throw 'npm ci failed' } }
        finally { Pop-Location }
    }
    if ($Clean) {
        Remove-VertexBuildDirectory -LiteralPath (Join-Path $FrontendDir 'dist') -WorkspaceRoot $WorkspaceRoot
        cargo clean -p vertex-app
        if ($LASTEXITCODE -ne 0) { throw 'cargo clean failed' }
    }
    if ($Test) {
        cargo test --workspace --locked
        $vertexExitCode = $LASTEXITCODE
    } else {
        if ($Release -or $BuildOnly) {
            Push-Location $FrontendDir
            try {
                npm run build
                if ($LASTEXITCODE -ne 0) { throw 'Frontend build failed' }
            } finally { Pop-Location }
        }
        if ($Release) {
            cargo build --release -p vertex-app --locked --features tauri/custom-protocol
            if ($LASTEXITCODE -ne 0) { throw 'Release build failed' }
            if (-not $BuildOnly) {
                $metadata = cargo metadata --no-deps --format-version 1 | ConvertFrom-Json
                $exePath = Join-Path $metadata.target_directory 'release\vertex-app.exe'
                $process = Start-Process -FilePath $exePath -WorkingDirectory $WorkspaceRoot -WindowStyle Hidden -PassThru
                $process.WaitForExit()
                $vertexExitCode = $process.ExitCode
            }
        } elseif ($BuildOnly) {
            cargo build -p vertex-app --locked
            $vertexExitCode = $LASTEXITCODE
        } else {
            if (-not (Test-VertexTcpPort -Port 5173)) {
                $viteScript = Join-Path $FrontendDir 'node_modules\vite\bin\vite.js'
                $node = (Get-Command node.exe -ErrorAction Stop).Source
                $viteProcess = Start-Process -FilePath $node -ArgumentList ('"{0}" --host 127.0.0.1 --port 5173 --strictPort' -f $viteScript) -WorkingDirectory $FrontendDir -WindowStyle Hidden -PassThru
                $deadline = [DateTime]::UtcNow.AddSeconds(10)
                while (-not (Test-VertexTcpPort -Port 5173)) {
                    if ($viteProcess.HasExited -or [DateTime]::UtcNow -ge $deadline) { throw 'Vite did not start on port 5173' }
                    Start-Sleep -Milliseconds 50
                }
            }
            cargo run -p vertex-app --locked
            $vertexExitCode = $LASTEXITCODE
        }
    }
} catch {
    Write-Error $_ -ErrorAction Continue
    $vertexExitCode = 1
} finally {
    Stop-VertexProcessTree -Process $process
    Stop-VertexProcessTree -Process $viteProcess
    Pop-Location
}
exit $vertexExitCode
