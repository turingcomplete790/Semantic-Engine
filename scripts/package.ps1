# Build a release package of the plugin (Windows: zip). Mirrors scripts/package.sh.
# Output: dist\semantic-engine-<version>-<target>.zip and a matching .sha256 file.
#
# usage: pwsh -File scripts/package.ps1 [-Target TRIPLE] [-Out DIR]

[CmdletBinding()]
param(
    [string]$Target = '',
    [string]$Out = 'dist'
)

$ErrorActionPreference = 'Stop'

# Text files in the package and the checksum file are written as UTF-8 without a BOM and with LF
# line endings on every OS; Set-Content would use CRLF (and on 5.1 a legacy encoding) on Windows.
function Write-LfText([string]$Path, [string[]]$Lines) {
    $utf8 = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, (($Lines -join "`n") + "`n"), $utf8)
}
$root = Resolve-Path (Join-Path $PSScriptRoot '..')
Set-Location $root

if (-not $Target) {
    $Target = ((rustc -vV) | Where-Object { $_ -like 'host: *' }) -replace '^host: ', ''
}

cargo build --release --locked -p plugin --target $Target
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

$exeName = if ($Target -like '*windows*') { 'semantic-engine.exe' } else { 'semantic-engine' }
$bin = Join-Path $root "target/$Target/release/$exeName"
if (-not (Test-Path -LiteralPath $bin)) { throw "build did not produce $bin" }

$version = (Select-String -Path 'crates/plugin/Cargo.toml' -Pattern '^version = "(.*)"' |
    Select-Object -First 1).Matches[0].Groups[1].Value
$name = "semantic-engine-$version-$Target"

$stage = Join-Path ([System.IO.Path]::GetTempPath()) ("se-package-" + [guid]::NewGuid())
try {
    $dest = Join-Path $stage 'semantic-engine'
    New-Item -ItemType Directory -Path (Join-Path $dest 'vocabulary') -Force | Out-Null
    $manifest = (Get-Content -LiteralPath 'plugin/semantic-engine.yml') -replace '^version: .*', "version: $version"
    Write-LfText (Join-Path $dest 'semantic-engine.yml') $manifest
    Copy-Item -LiteralPath $bin -Destination (Join-Path $dest $exeName)
    Copy-Item -Path 'vocabulary/*.ttl', 'vocabulary/CHANGELOG.md', 'vocabulary/TERMS.md' -Destination (Join-Path $dest 'vocabulary')
    $versions = & $bin --versions
    if ($LASTEXITCODE -ne 0) { throw "$bin --versions failed" }
    Write-LfText (Join-Path $dest 'VERSIONS.json') $versions

    New-Item -ItemType Directory -Path $Out -Force | Out-Null
    $archive = Join-Path (Resolve-Path $Out) "$name.zip"
    if (Test-Path -LiteralPath $archive) { Remove-Item -LiteralPath $archive }
    Compress-Archive -Path $dest -DestinationPath $archive
    $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $archive).Hash.ToLowerInvariant()
    Write-LfText "$archive.sha256" @("$hash  $name.zip")
    Write-Output $archive
}
finally {
    Remove-Item -LiteralPath $stage -Recurse -Force -ErrorAction SilentlyContinue
}
