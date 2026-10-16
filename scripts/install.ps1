# Install the Semantic Engine Stash plugin on Windows.
#
# Runs on Windows PowerShell 5.1 and PowerShell 7. Contract:
# specs/002-engine-plugin-foundation/contracts/install-scripts.md
#
# Exit codes: 0 installed, 2 bad arguments, 3 no package for this platform, 4 plugins folder not
# found or not writable, 5 download or checksum failure, 6 unpack or copy failure.

[CmdletBinding(PositionalBinding = $false)]
param(
    [string]$PluginsDir = '',
    [string]$Version = 'latest',
    [string]$FromFile = '',
    [switch]$Yes,
    [switch]$Help,
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$Rest
)

$ErrorActionPreference = 'Stop'
$PluginDirName = 'semantic-engine'
# The project's releases page (GitHub layout: <url>/download/vX.Y.Z/<file> and
# <url>/latest/download/<file>). SEMANTIC_ENGINE_RELEASE_URL overrides it, for mirrors and tests.
$ReleaseUrl = if ($env:SEMANTIC_ENGINE_RELEASE_URL) { $env:SEMANTIC_ENGINE_RELEASE_URL } else { 'https://github.com/turingcomplete790/Semantic-Engine/releases' }
$BuildDocs = "Build from source: install Rust, a C++17 compiler (Visual Studio Build Tools), and clang (LLVM), then run scripts\package.ps1 in the source tree and install the result with -FromFile."

function Show-Usage {
    @'
Install the Semantic Engine plugin into Stash's plugins folder.

usage: install.ps1 [-PluginsDir DIR] [-Version X.Y.Z] [-FromFile PACKAGE] [-Yes]

  -PluginsDir DIR   Stash's plugins folder (default: from Stash's config.yml)
  -Version X.Y.Z    version to download (default: latest)
  -FromFile PKG     install a package already on disk; PKG.sha256 must sit beside it
  -Yes              do not ask for confirmation
  -Help             show this help

Nothing outside <plugins>\semantic-engine is changed. Reload plugins in Stash afterwards
(Settings > Plugins > Reload plugins).
'@ | Write-Host
}

function Fail([int]$Code, [string]$Message) {
    [Console]::Error.WriteLine("error: $Message")
    exit $Code
}

if ($Rest) { Show-Usage; Fail 2 "unknown argument: $($Rest -join ' ')" }
if ($Help) { Show-Usage; exit 0 }

# ---- 1. Platform (research R7). SE_TEST_SKIP_PLATFORM exists only for the test suite.
if (-not $env:SE_TEST_SKIP_PLATFORM) {
    $onWindows = ($PSVersionTable.PSVersion.Major -lt 6) -or $IsWindows
    $arch = $env:PROCESSOR_ARCHITECTURE
    if (-not $onWindows) {
        [Console]::Error.WriteLine("This script installs on Windows. On BSD, macOS, or Linux use install.sh.")
        [Console]::Error.WriteLine($BuildDocs)
        exit 3
    }
    if ($arch -ne 'AMD64') {
        [Console]::Error.WriteLine("No prebuilt Semantic Engine package exists for Windows ($arch).")
        [Console]::Error.WriteLine("Packages are published for Windows x86_64, and for Linux and macOS on x86_64 and aarch64.")
        if ($ReleaseUrl) { [Console]::Error.WriteLine("Release page: $ReleaseUrl") }
        [Console]::Error.WriteLine($BuildDocs)
        exit 3
    }
}
$target = 'x86_64-pc-windows-msvc'

# ---- 2. Plugins folder (research R8)
function Find-PluginsDir {
    $userHome = if ($env:USERPROFILE) { $env:USERPROFILE } else { $HOME }
    foreach ($cfg in @((Join-Path (Get-Location) 'config.yml'), (Join-Path $userHome '.stash\config.yml'))) {
        if (-not (Test-Path -LiteralPath $cfg -PathType Leaf)) { continue }
        $cfgDir = Split-Path -Parent (Resolve-Path -LiteralPath $cfg)
        $line = Select-String -LiteralPath $cfg -Pattern '^plugins_path:\s*(.*)$' | Select-Object -First 1
        if ($line) {
            $p = $line.Matches[0].Groups[1].Value.Trim().Trim('"', "'")
            if ([System.IO.Path]::IsPathRooted($p)) { return $p }
            return (Join-Path $cfgDir $p)
        }
        return (Join-Path $cfgDir 'plugins')
    }
    return $null
}
if (-not $PluginsDir) {
    $PluginsDir = Find-PluginsDir
    if (-not $PluginsDir) { Fail 4 "cannot find Stash's config.yml; pass -PluginsDir" }
}
if (-not (Test-Path -LiteralPath $PluginsDir -PathType Container)) { Fail 4 "plugins folder not found: $PluginsDir (pass -PluginsDir)" }
$PluginsDir = (Resolve-Path -LiteralPath $PluginsDir).Path
try {
    $probe = Join-Path $PluginsDir (".se-write-test-" + [guid]::NewGuid())
    New-Item -ItemType File -Path $probe | Out-Null
    Remove-Item -LiteralPath $probe -Force
} catch { Fail 4 "plugins folder is not writable: $PluginsDir" }

# ---- 3. Package and checksum
$work = Join-Path ([System.IO.Path]::GetTempPath()) ("se-install-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $work | Out-Null
try {
    if ($FromFile) {
        if (-not (Test-Path -LiteralPath $FromFile -PathType Leaf)) { Fail 5 "package not found: $FromFile" }
        if (-not (Test-Path -LiteralPath "$FromFile.sha256" -PathType Leaf)) { Fail 5 "checksum file not found: $FromFile.sha256" }
        $package = (Resolve-Path -LiteralPath $FromFile).Path
        $expected = ((Get-Content -LiteralPath "$FromFile.sha256" -TotalCount 1) -split '\s+')[0].ToLowerInvariant()
    } else {
        $base = if ($Version -eq 'latest') { "$ReleaseUrl/latest/download" }
                elseif ($Version -like 'v*') { "$ReleaseUrl/download/$Version" }
                else { "$ReleaseUrl/download/v$Version" }
        $sums = Join-Path $work 'SHA256SUMS'
        try { Invoke-WebRequest -UseBasicParsing -Uri "$base/SHA256SUMS" -OutFile $sums } catch { Fail 5 "cannot download $base/SHA256SUMS" }
        $entry = Get-Content -LiteralPath $sums | Where-Object { $_ -match "semantic-engine-.*-$target\.zip$" } | Select-Object -First 1
        if (-not $entry) { Fail 3 "no package for $target in release $Version" }
        $parts = $entry -split '\s+'
        $expected = $parts[0].ToLowerInvariant()
        $file = $parts[-1].TrimStart('*')
        $package = Join-Path $work $file
        try { Invoke-WebRequest -UseBasicParsing -Uri "$base/$file" -OutFile $package } catch { Fail 5 "cannot download $base/$file" }
    }
    $actual = (Get-FileHash -Algorithm SHA256 -LiteralPath $package).Hash.ToLowerInvariant()
    if ($actual -ne $expected) { Fail 5 "checksum mismatch for $package (expected $expected, got $actual)" }

    # ---- 4. Unpack and replace <plugins>\semantic-engine only
    $unpack = Join-Path $work 'unpack'
    try { Expand-Archive -LiteralPath $package -DestinationPath $unpack } catch { Fail 6 "cannot unpack ${package}: $_" }
    $new = Join-Path $unpack $PluginDirName
    if (-not (Test-Path -LiteralPath (Join-Path $new 'semantic-engine.yml'))) { Fail 6 "package has no $PluginDirName\semantic-engine.yml" }

    $dest = Join-Path $PluginsDir $PluginDirName
    if (-not $Yes -and [Environment]::UserInteractive -and -not [Console]::IsInputRedirected) {
        $answer = Read-Host "Install into ${dest}? [y/N]"
        if ($answer -notmatch '^(y|yes)$') { Write-Host 'cancelled'; exit 0 }
    }
    $staged = Join-Path $PluginsDir ".$PluginDirName.new.$PID"
    $old = Join-Path $PluginsDir ".$PluginDirName.old.$PID"
    try { Copy-Item -LiteralPath $new -Destination $staged -Recurse } catch { Fail 6 "cannot copy into $PluginsDir" }
    if (Test-Path -LiteralPath $dest) {
        try { Move-Item -LiteralPath $dest -Destination $old -Force } catch {
            Remove-Item -LiteralPath $staged -Recurse -Force; Fail 6 'cannot move the existing install aside'
        }
    }
    try { Move-Item -LiteralPath $staged -Destination $dest -Force } catch {
        if (Test-Path -LiteralPath $old) { Move-Item -LiteralPath $old -Destination $dest -Force }
        Remove-Item -LiteralPath $staged -Recurse -Force -ErrorAction SilentlyContinue
        Fail 6 'cannot move the new install into place; the previous install was restored'
    }
    if (Test-Path -LiteralPath $old) { Remove-Item -LiteralPath $old -Recurse -Force }

    # ---- 5. Report
    $installed = '(unknown version)'
    $versionsFile = Join-Path $dest 'VERSIONS.json'
    if (Test-Path -LiteralPath $versionsFile) {
        try { $installed = (Get-Content -LiteralPath $versionsFile -Raw | ConvertFrom-Json).engineVersion } catch { }
    }
    Write-Host "Installed Semantic Engine $installed for $target into $dest"
    Write-Host 'Reload plugins in Stash: Settings > Plugins > Reload plugins.'
    exit 0
}
finally {
    Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
}
