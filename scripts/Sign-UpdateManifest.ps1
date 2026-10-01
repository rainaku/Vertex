#Requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$InstallerPath,
    [Parameter(Mandatory)][string]$Version,
    [string]$PrivateKeyPath,
    [string]$PublicKeyPath = (Join-Path $PSScriptRoot '../Security/update-public-key.pem')
)
$ErrorActionPreference = 'Stop'

$installer = Get-Item -LiteralPath $InstallerPath
if ($installer.Name -notmatch '^Vertex.*(\.exe|\.msi)$' -and $installer.Name -ne 'vertex-app.exe') {
    throw "Unexpected installer name: $($installer.Name)"
}

$parsedVersion = $null
$normalizedVersion = $Version.TrimStart('v')
if (-not [Version]::TryParse($normalizedVersion, [ref]$parsedVersion)) {
    throw "Invalid release version: $Version"
}

if ($installer.Length -le 0 -or $installer.Length -gt 500MB) {
    throw "Invalid installer size: $($installer.Length) bytes"
}

$privatePem = if ($PrivateKeyPath) {
    [IO.File]::ReadAllText([IO.Path]::GetFullPath($PrivateKeyPath))
} else {
    $env:VERTEX_UPDATE_SIGNING_KEY_PEM
}

if ([string]::IsNullOrWhiteSpace($privatePem)) {
    throw 'VERTEX_UPDATE_SIGNING_KEY_PEM or -PrivateKeyPath is required. Unsigned releases are not published.'
}

$pubKeyPath = [IO.Path]::GetFullPath($PublicKeyPath)
if (-not (Test-Path -LiteralPath $pubKeyPath)) {
    throw "Public key not found at: $pubKeyPath"
}

$key = [Security.Cryptography.ECDsa]::Create()
$verifier = [Security.Cryptography.ECDsa]::Create()
try {
    $key.ImportFromPem($privatePem)
    $verifier.ImportFromPem([IO.File]::ReadAllText($pubKeyPath))
    if ($key.KeySize -ne 256 -or $key.ExportSubjectPublicKeyInfoPem() -cne $verifier.ExportSubjectPublicKeyInfoPem()) {
        throw 'Signing key does not match the public key embedded in this repository.'
    }
    
    $hash = (Get-FileHash -LiteralPath $installer.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    $manifest = [ordered]@{
        SchemaVersion = 1
        Version       = $normalizedVersion
        InstallerName = $installer.Name
        Size          = $installer.Length
        Sha256        = $hash
    }
    
    $bytes = [Text.Encoding]::UTF8.GetBytes(($manifest | ConvertTo-Json -Compress))
    $signature = $key.SignData($bytes, [Security.Cryptography.HashAlgorithmName]::SHA256,
        [Security.Cryptography.DSASignatureFormat]::IeeeP1363FixedFieldConcatenation)
        
    if (-not $verifier.VerifyData($bytes, $signature, [Security.Cryptography.HashAlgorithmName]::SHA256,
        [Security.Cryptography.DSASignatureFormat]::IeeeP1363FixedFieldConcatenation)) {
        throw 'Signature self-check failed.'
    }
    
    [IO.File]::WriteAllBytes($installer.FullName + '.manifest.json', $bytes)
    [IO.File]::WriteAllBytes($installer.FullName + '.manifest.sig', $signature)
    [IO.File]::WriteAllText($installer.FullName + '.sha256', "$hash  $($installer.Name)`n")
    
    Write-Host "Successfully signed update manifest for $($installer.Name) (v$normalizedVersion)." -ForegroundColor Green
    Write-Host "  Manifest:  $($installer.FullName).manifest.json" -ForegroundColor DarkGray
    Write-Host "  Signature: $($installer.FullName).manifest.sig" -ForegroundColor DarkGray
    Write-Host "  Checksum:  $($installer.FullName).sha256" -ForegroundColor DarkGray
} finally {
    $privatePem = $null
    $key.Dispose()
    $verifier.Dispose()
}
