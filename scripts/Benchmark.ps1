param(
    [Parameter(Mandatory = $true)][string]$Baseline,
    [int]$Samples = 9,
    [int]$Rounds = 3,
    [string]$OutputDirectory = 'artifacts/performance/paired'
)

$ErrorActionPreference = 'Stop'
if ($Samples -lt 3 -or $Rounds -lt 1) { throw 'Use at least 3 samples and 1 round.' }
$workspace = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$oldSamples = $env:VERTEX_BENCH_SAMPLES
Push-Location $workspace
try {
    $baselinePath = (Resolve-Path -LiteralPath $Baseline).Path
    & cargo build --release -p vertex-core --example benchmark --locked
    if ($LASTEXITCODE -ne 0) { throw 'Benchmark build failed' }
    $currentPath = (Resolve-Path -LiteralPath 'target/release/examples/benchmark.exe').Path
    New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
    $outputPath = (Resolve-Path -LiteralPath $OutputDirectory).Path
    $env:VERTEX_BENCH_SAMPLES = [string]$Samples
    $runs = @()
    for ($round = 1; $round -le $Rounds; $round++) {
        # Alternate order to reduce bias from CPU temperature and background load.
        $order = if ($round % 2) { @('before', 'after') } else { @('after', 'before') }
        foreach ($label in $order) {
            $binary = if ($label -eq 'before') { $baselinePath } else { $currentPath }
            $jsonPath = Join-Path $outputPath "$label-$round.json"
            & $binary > $jsonPath 2> (Join-Path $outputPath "$label-$round.log")
            if ($LASTEXITCODE -ne 0) { throw "$label round $round failed" }
            $result = Get-Content -LiteralPath $jsonPath -Raw | ConvertFrom-Json
            $runs += [PSCustomObject]@{ label = $label; round = $round; result = $result }
        }
    }
    $summary = foreach ($name in $runs[0].result.workloads.name) {
        $before = @($runs | Where-Object label -eq 'before' | ForEach-Object { $_.result.workloads | Where-Object name -eq $name })
        $after = @($runs | Where-Object label -eq 'after' | ForEach-Object { $_.result.workloads | Where-Object name -eq $name })
        $beforeTimes = @($before.samples_ms | Sort-Object)
        $afterTimes = @($after.samples_ms | Sort-Object)
        $beforeMedian = $beforeTimes[[int][math]::Floor($beforeTimes.Count / 2)]
        $afterMedian = $afterTimes[[int][math]::Floor($afterTimes.Count / 2)]
        # Archive headers contain filesystem timestamps. Their content equivalence
        # is covered by archive regression tests instead of a container hash.
        $same = if ($name -eq 'zip_targz_32_files') { $null } else {
            @(@($before.sha256) + @($after.sha256) | Sort-Object -Unique).Count -eq 1
        }
        if ($same -eq $false) { throw "Output changed for $name" }
        [PSCustomObject]@{
            name = $name; before_ms = $beforeMedian; after_ms = $afterMedian
            speedup = $beforeMedian / $afterMedian; time_reduction_percent = (1 - $afterMedian / $beforeMedian) * 100
            identical_output = $same; samples_per_version = $beforeTimes.Count
        }
    }
    [PSCustomObject]@{
        cpu = (Get-CimInstance Win32_Processor).Name
        os = (Get-CimInstance Win32_OperatingSystem).Caption
        rust = (& rustc --version); node = (& node --version)
        baseline_sha256 = (Get-FileHash -LiteralPath $baselinePath).Hash
        current_sha256 = (Get-FileHash -LiteralPath $currentPath).Hash
        rounds = $Rounds; samples = $Samples; summary = @($summary)
    } | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $outputPath 'summary.json')
    $summary | Format-Table name, before_ms, after_ms, speedup, identical_output
} finally {
    $env:VERTEX_BENCH_SAMPLES = $oldSamples
    Pop-Location
}
