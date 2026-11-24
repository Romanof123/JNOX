param([Parameter(Mandatory=$true)][string]$Folder, [Parameter(Mandatory=$true)][string]$ManifestHash)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
try {
    $full = (Resolve-Path -LiteralPath $Folder).ProviderPath
    if ([IO.Path]::GetPathRoot($full) -ine 'D:\') { throw "Destination is outside D:" }
    $manifestPath = Join-Path $full 'manifest.json'
    if ((Get-FileHash -Algorithm SHA256 -LiteralPath $manifestPath).Hash -ine $ManifestHash) {
        throw "Manifest checksum mismatch"
    }
    $manifest = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json
    $path = Join-Path $full 'position.zip'
    $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $path).Hash.ToLowerInvariant()
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
        $sha = [Security.Cryptography.SHA256]::Create()
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
    [ordered]@{path=$path; archive_sha256=$hash; manifest_sha256=$ManifestHash; members=@($manifest.members).Count} | ConvertTo-Json -Compress
    exit 0
} catch {
    [Console]::Error.WriteLine([string]$_)
    exit 1
}
