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
[IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($privatePath)) | Out-Null
[IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($publicPath)) | Out-Null
if ($IsWindows) {
    # Protect the parent directory before creating the private key.
    $directory = [IO.DirectoryInfo]::new([IO.Path]::GetDirectoryName($privatePath))
    $acl = [Security.AccessControl.DirectorySecurity]::new()
    $acl.SetAccessRuleProtection($true, $false)
    $sid = [Security.Principal.WindowsIdentity]::GetCurrent().User
    $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new(
        $sid, 'FullControl', 'ContainerInherit,ObjectInherit', 'None', 'Allow'))
    [IO.FileSystemAclExtensions]::SetAccessControl($directory, $acl)
} else {
    throw 'Generate the production key on Windows in a dedicated private directory.'
}
$key = [Security.Cryptography.ECDsa]::Create([Security.Cryptography.ECCurve]::CreateFromFriendlyName('nistP256'))
try {
    [IO.File]::WriteAllText($privatePath, $key.ExportPkcs8PrivateKeyPem())
    [IO.File]::WriteAllText($publicPath, $key.ExportSubjectPublicKeyInfoPem())
    Write-Host 'Vertex signing key generated successfully.' -ForegroundColor Green
    Write-Host "Public key:  $publicPath" -ForegroundColor Cyan
    Write-Host "Private key: $privatePath" -ForegroundColor Yellow
    Write-Host 'Back up the private key securely; never commit, leak, or log it.' -ForegroundColor Magenta
} finally { $key.Dispose() }
