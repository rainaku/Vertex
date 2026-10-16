# Shared PowerShell 5.1/7 build helpers.
function Remove-VertexBuildDirectory {
    param([Parameter(Mandatory=$true)][string]$LiteralPath, [Parameter(Mandatory=$true)][string]$WorkspaceRoot)
    $rootPath = [IO.Path]::GetFullPath($WorkspaceRoot).TrimEnd('\', '/')
    $targetPath = [IO.Path]::GetFullPath($LiteralPath)
    if (-not $targetPath.StartsWith($rootPath + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to remove a build directory outside the workspace: $targetPath"
    }
    if (Test-Path -LiteralPath $targetPath) { Remove-Item -LiteralPath $targetPath -Recurse -Force }
}

function Test-VertexTcpPort {
    param([int]$Port, [int]$TimeoutMs = 200)
    $client = New-Object Net.Sockets.TcpClient
    try {
        $pending = $client.ConnectAsync('127.0.0.1', $Port)
        if (-not $pending.Wait($TimeoutMs)) { return $false }
        return $client.Connected
    } catch { return $false }
    finally { $client.Dispose() }
}

function Stop-VertexProcessTree {
    param([System.Diagnostics.Process]$Process)
    if ($Process -and -not $Process.HasExited) {
        & taskkill.exe /PID $Process.Id /T /F 2>$null | Out-Null
        if (-not $Process.HasExited) { Stop-Process -Id $Process.Id -Force -ErrorAction SilentlyContinue }
    }
}

function Show-VertexFileInExplorer {
    param([Parameter(Mandatory=$true)][string]$LiteralPath)
    $item = Get-Item -LiteralPath $LiteralPath
    if (-not ('Vertex.BuildShell' -as [type])) {
        Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
namespace Vertex {
    public static class BuildShell {
        [DllImport("shell32.dll", CharSet = CharSet.Unicode)]
        private static extern int SHParseDisplayName(string name, IntPtr context, out IntPtr pidl, uint attributes, out uint result);
        [DllImport("shell32.dll")]
        private static extern int SHOpenFolderAndSelectItems(IntPtr pidl, uint count, IntPtr children, uint flags);
        public static void Reveal(string path) {
            IntPtr pidl;
            uint attributes;
            Marshal.ThrowExceptionForHR(SHParseDisplayName(path, IntPtr.Zero, out pidl, 0, out attributes));
            try { Marshal.ThrowExceptionForHR(SHOpenFolderAndSelectItems(pidl, 0, IntPtr.Zero, 0)); }
            finally { Marshal.FreeCoTaskMem(pidl); }
        }
    }
}
"@
    }
    [Vertex.BuildShell]::Reveal($item.FullName)
}