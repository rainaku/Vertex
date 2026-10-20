#Requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$PrivateKeyPath,
    [string]$PublicKeyPath = (Join-Path $PSScriptRoot '../Security/update-public-key.pem')
)
$ErrorActionPreference = 'Stop'
$privatePath = [IO.Path]::GetFullPath($PrivateKeyPath)
$publicPath = [IO.Path]::GetFullPath($PublicKeyPath)
$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
if ($privatePath.StartsWith($repoRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
    throw 'Private key must be stored outside the repository.'
}
if ((Test-Path -LiteralPath $privatePath) -or (Test-Path -LiteralPath $publicPath)) {
    throw 'Refusing to overwrite an existing signing key. Key rotation requires a migration plan.'
}
if (-not $IsWindows) {
    throw 'Generate the production key on Windows in a dedicated private directory.'
}
$privateDir = [IO.Path]::GetDirectoryName($privatePath)
$publicDir = [IO.Path]::GetDirectoryName($publicPath)
if ([string]::IsNullOrWhiteSpace($privateDir)) {
    throw "Invalid private key path (missing parent directory): $privatePath"
}
[IO.Directory]::CreateDirectory($privateDir) | Out-Null
if (-not [string]::IsNullOrWhiteSpace($publicDir)) {
    [IO.Directory]::CreateDirectory($publicDir) | Out-Null
}
$key = [Security.Cryptography.ECDsa]::Create([Security.Cryptography.ECCurve]::CreateFromFriendlyName('nistP256'))
try {
    [IO.File]::WriteAllText($privatePath, $key.ExportPkcs8PrivateKeyPem())
    $privateFile = [IO.FileInfo]::new($privatePath)
    $acl = [Security.AccessControl.FileSecurity]::new()
    $acl.SetAccessRuleProtection($true, $false)
    $sid = [Security.Principal.WindowsIdentity]::GetCurrent().User
    $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new(
        $sid, 'FullControl', 'None', 'None', 'Allow'))
    [IO.FileSystemAclExtensions]::SetAccessControl($privateFile, $acl)
    [IO.File]::WriteAllText($publicPath, $key.ExportSubjectPublicKeyInfoPem())
    Write-Host 'Vertex signing key generated successfully.' -ForegroundColor Green
    Write-Host "Public key:  $publicPath" -ForegroundColor Cyan
    Write-Host "Private key: $privatePath" -ForegroundColor Yellow
    Write-Host 'Back up the private key securely; never commit, leak, or log it.' -ForegroundColor Magenta
} finally { $key.Dispose() }
