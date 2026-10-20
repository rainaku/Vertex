param(
    # Optional full path to an installed Python interpreter.
    [string]$Python
)

$ErrorActionPreference = 'Stop'
if (-not $env:USERPROFILE) { throw 'USERPROFILE is required on Windows.' }

if (-not $Python) {
    if (Get-Command py.exe -ErrorAction SilentlyContinue) {
        $Python = & py.exe -3 -c 'import sys; print(sys.executable)'
        if ($LASTEXITCODE -ne 0) { throw 'Python could not start. Pass -Python with its full path.' }
    } elseif (Get-Command python.exe -ErrorAction SilentlyContinue) {
        $Python = & python.exe -c 'import sys; print(sys.executable)'
        if ($LASTEXITCODE -ne 0) { throw 'Install Python, then run this script again.' }
    } else {
        throw 'Install Python, then run this script again.'
    }
}
if (-not (Test-Path -LiteralPath $Python -PathType Leaf)) {
    throw "Python executable not found: $Python"
}

# USERPROFILE also works when setup runs inside an MSIX-packaged terminal/app;
# LocalAppData can be redirected into that app's private filesystem.
$toolRoot = Join-Path $env:USERPROFILE '.vertex\tools\pdf2docx'
$toolPython = Join-Path $toolRoot 'Scripts\python.exe'
if (-not (Test-Path -LiteralPath $toolPython -PathType Leaf)) {
    & $Python -m venv $toolRoot
    if ($LASTEXITCODE -ne 0) { throw 'Could not create the Vertex document-tool environment.' }
}
& $toolPython -m pip install --disable-pip-version-check 'pdf2docx==0.5.13'
if ($LASTEXITCODE -ne 0) { throw 'Could not install pdf2docx.' }
& $toolPython -c 'from pdf2docx import Converter; print("PDF to Word engine is ready.")'
if ($LASTEXITCODE -ne 0) { throw 'pdf2docx was installed but could not be loaded.' }
Write-Output "Vertex will detect this engine automatically: $toolRoot"
