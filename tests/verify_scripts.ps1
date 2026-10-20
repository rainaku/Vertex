$ErrorActionPreference = 'Stop'
$WorkspaceRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
. (Join-Path $WorkspaceRoot 'scripts\BuildTools.ps1')

foreach ($name in @('b.ps1', 'r.ps1', 'prepush.ps1', 'scripts/BuildTools.ps1', 'scripts/New-UpdateSigningKey.ps1', 'scripts/Sign-UpdateManifest.ps1')) {
    $tokens = $null
    $errors = $null
    [void][Management.Automation.Language.Parser]::ParseFile((Join-Path $WorkspaceRoot $name), [ref]$tokens, [ref]$errors)
    if ($errors.Count) { throw ($errors | Out-String) }
}

foreach ($target in @($WorkspaceRoot, [IO.Path]::GetTempPath())) {
    try { Remove-VertexBuildDirectory -LiteralPath $target -WorkspaceRoot $WorkspaceRoot; throw 'Workspace guard failed' }
    catch { if ($_.Exception.Message -notlike 'Refusing to remove*') { throw } }
}

$testDir = Join-Path $WorkspaceRoot ('target\script-tests-' + [Guid]::NewGuid().ToString('N'))
[void][IO.Directory]::CreateDirectory($testDir)
try {
    $nested = Join-Path $testDir 'a directory with spaces'
    [void][IO.Directory]::CreateDirectory($nested)
    Remove-VertexBuildDirectory -LiteralPath $nested -WorkspaceRoot $WorkspaceRoot
    if (Test-Path -LiteralPath $nested) { throw 'Build cleanup failed' }
} finally { Remove-VertexBuildDirectory -LiteralPath $testDir -WorkspaceRoot $WorkspaceRoot }

$listener = New-Object Net.Sockets.TcpListener ([Net.IPAddress]::Loopback), 0
$listener.Start()
try {
    $port = $listener.LocalEndpoint.Port
    if (-not (Test-VertexTcpPort -Port $port)) { throw 'Live TCP port was not detected' }
} finally { $listener.Stop() }
if (Test-VertexTcpPort -Port $port) { throw 'Closed TCP port reported open' }
[void](Test-VertexTcpPort -Port $port -TimeoutMs 0)

if ($PSVersionTable.PSVersion.Major -ge 7 -and $IsWindows) {
    $extTemp = Join-Path ([IO.Path]::GetTempPath()) ('vertex-key-test-' + [Guid]::NewGuid().ToString('N'))
    [void][IO.Directory]::CreateDirectory($extTemp)
    try {
        $privKey = Join-Path $extTemp 'test-private.pem'
        $pubKey = Join-Path $extTemp 'test-public.pem'
        & (Join-Path $WorkspaceRoot 'scripts/New-UpdateSigningKey.ps1') -PrivateKeyPath $privKey -PublicKeyPath $pubKey 6>$null
        $fileAcl = [IO.FileSystemAclExtensions]::GetAccessControl([IO.FileInfo]::new($privKey))
        if (-not $fileAcl.AreAccessRulesProtected) { throw 'Private key file ACL protection was not enabled' }

        $dummyInstaller = Join-Path $extTemp 'Vertex_1.2.0_x64-setup.exe'
        [IO.File]::WriteAllBytes($dummyInstaller, [byte[]](1..64))
        & (Join-Path $WorkspaceRoot 'scripts/Sign-UpdateManifest.ps1') -InstallerPath $dummyInstaller -Version 'v1.2.0-rc.1' -PrivateKeyPath $privKey -PublicKeyPath $pubKey 6>$null
        if (-not (Test-Path -LiteralPath ($dummyInstaller + '.manifest.sig'))) { throw 'Manifest signature was not created for SemVer prerelease' }

        $rejected = $false
        try {
            & (Join-Path $WorkspaceRoot 'scripts/Sign-UpdateManifest.ps1') -InstallerPath $dummyInstaller -Version 'not-a-version' -PrivateKeyPath $privKey -PublicKeyPath $pubKey | Out-Null
        } catch {
            if ($_.Exception.Message -like 'Invalid release version*') { $rejected = $true } else { throw }
        }
        if (-not $rejected) { throw 'Invalid version was not rejected' }
    } finally {
        if (Test-Path -LiteralPath $extTemp) { Remove-Item -LiteralPath $extTemp -Recurse -Force }
    }
}
Write-Output ('PowerShell ' + $PSVersionTable.PSVersion + ': syntax, workspace guard, cleanup and TCP checks passed')
