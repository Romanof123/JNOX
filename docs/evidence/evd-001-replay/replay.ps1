param([Parameter(Mandatory=$true)][string]$Repository)
$ErrorActionPreference = 'Stop'
$sha = '604b03186841059ce3b4226367824fa923742e88'
$stage = Join-Path ([IO.Path]::GetTempPath()) ('jnox-replay-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $stage | Out-Null
foreach ($name in 'probe.py','inject.py','controls.py','public-check.py') {
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot $name) -Destination $stage
}
git -C $Repository archive --format=zip --output="$stage/source.zip" $sha
if ($LASTEXITCODE) { throw 'git archive failed' }
foreach ($name in 'a','b','mutant') {
    Expand-Archive -LiteralPath "$stage/source.zip" -DestinationPath "$stage/$name"
}
python "$stage/inject.py"
if ($LASTEXITCODE) { throw 'BSS clear mutation failed' }
foreach ($name in 'a','b','mutant') {
    Push-Location "$stage/$name/kernel"
    try {
        $env:CARGO_TARGET_DIR = "$stage/$name/target"
        cargo +1.95.0 build --release --locked
        if ($LASTEXITCODE) { throw 'cargo build failed' }
    } finally {
        Pop-Location
    }
}
$linux = (wsl -d Ubuntu -- wslpath -u ($stage.Replace('\','/'))).Trim()
if ($LASTEXITCODE) { throw 'path conversion failed' }
foreach ($name in 'a','b') {
    wsl -d Ubuntu -- python3 "$linux/probe.py" "$linux/$name/target/aarch64-unknown-none/release/jnox-kernel" "$linux/cold-$name.json"
    if ($LASTEXITCODE) { throw 'positive control failed' }
}
wsl -d Ubuntu -- python3 "$linux/probe.py" "$linux/mutant/target/aarch64-unknown-none/release/jnox-kernel" "$linux/negative.json" --boot-timeout 1
if ($LASTEXITCODE -ne 1) { throw 'Expected rejection of BSS-clear mutation' }
wsl -d Ubuntu -- python3 "$linux/public-check.py"
if ($LASTEXITCODE) { throw 'Public runner real-QEMU assertions failed' }
wsl -d Ubuntu -- python3 "$linux/controls.py"
if ($LASTEXITCODE) { throw 'Public runner synthetic control assertions failed' }
$hashes = foreach ($name in 'a','b') { (Get-FileHash -Algorithm SHA256 "$stage/$name/target/aarch64-unknown-none/release/jnox-kernel").Hash }
if ($hashes[0] -ne $hashes[1]) { throw 'Clean builds differ' }
Write-Output "Verified ELF $($hashes[0])"
Write-Output "Evidence $stage"
