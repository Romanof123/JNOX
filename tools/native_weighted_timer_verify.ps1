param([Parameter(Mandatory=$true)][string]$Folder, [Parameter(Mandatory=$true)][string]$ManifestHash)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
function Get-StreamSha256([string]$Path) {
    $algorithm = [System.Security.Cryptography.SHA256]::Create()
    try {
        $stream = [System.IO.File]::OpenRead($Path)
        try { return [System.BitConverter]::ToString($algorithm.ComputeHash($stream)).Replace('-','').ToLowerInvariant() }
        finally { $stream.Dispose() }
    } finally { $algorithm.Dispose() }
}
try {
    # WSL may inherit a module search path from a different PowerShell runtime.
    # Resolve the two Windows inbox modules from this interpreter's own installation.
    foreach ($module in @('Microsoft.PowerShell.Management','Microsoft.PowerShell.Utility')) {
        $manifestFile = [System.IO.Path]::Combine($PSHOME, 'Modules', $module, "$module.psd1")
        Import-Module -Name $manifestFile -ErrorAction Stop
    }
    $full = (Resolve-Path -LiteralPath $Folder).ProviderPath
    if ([IO.Path]::GetPathRoot($full) -ine 'D:\') { throw "Destination is outside D:" }
    $manifestPath = Join-Path $full 'manifest.json'
    if ((Get-StreamSha256 $manifestPath) -ine $ManifestHash) {
        throw "Manifest checksum mismatch"
    }
    $manifest = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json
    $path = Join-Path $full 'position.zip'
    $hash = Get-StreamSha256 $path
    if ($hash -cne $manifest.archive_sha256) { throw "Archive checksum mismatch" }
    if ((Get-Content -Raw -LiteralPath (Join-Path $full 'position.zip.sha256')).Trim() -cne "$hash  position.zip") {
        throw "External archive digest mismatch"
    }
    if ($manifest.session_sha256 -and (Get-Content -Raw -LiteralPath (Join-Path $full 'capture-session.sha256')).Trim() -cne $manifest.session_sha256) {
        throw "External session digest mismatch"
    }
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zip = [IO.Compression.ZipFile]::OpenRead($path)
    try {
        $expected = @{}
        foreach ($item in $manifest.members) { $expected.Add($item.name, $item) }
        $sha = [System.Security.Cryptography.SHA256]::Create()
        try {
            foreach ($entry in $zip.Entries) {
                $item = $expected[$entry.FullName]
                if (!$item -or $item.name -cne $entry.FullName -or $item.size -ne $entry.Length) {
                    throw "Unexpected or duplicate member $($entry.FullName)"
                }
                $stream = $entry.Open()
                try { $actual = [BitConverter]::ToString($sha.ComputeHash($stream)).Replace('-','').ToLowerInvariant() }
                finally { $stream.Dispose() }
                if ($actual -cne $item.sha256) { throw "Member checksum mismatch: $($entry.FullName)" }
                $expected.Remove($entry.FullName)
            }
            if ($expected.Count -ne 0) { throw "Missing archive member" }
        } finally { $sha.Dispose() }
    } finally { $zip.Dispose() }
    [ordered]@{path=$path; archive_sha256=$hash; manifest_sha256=$ManifestHash; members=@($manifest.members).Count; powershell=$PSVersionTable.PSVersion.ToString(); executable=[System.Diagnostics.Process]::GetCurrentProcess().MainModule.FileName; clr=[System.Environment]::Version.ToString()} | ConvertTo-Json -Compress
    exit 0
} catch {
    [Console]::Error.WriteLine([string]$_)
    exit 1
}
