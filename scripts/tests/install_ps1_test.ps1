# Tests for scripts/install.ps1. Run: pwsh -NoProfile -File scripts/tests/install_ps1_test.ps1
$ErrorActionPreference = 'Stop'
$script = Join-Path $PSScriptRoot '..' 'install.ps1'
$work = Join-Path ([System.IO.Path]::GetTempPath()) ("se-ps1-test-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $work | Out-Null
$pass = 0; $fail = 0
function Ok($m) { $script:pass++; Write-Host "ok   $m" }
function Bad($m) { $script:fail++; Write-Host "FAIL $m" }
function Run([string[]]$a) {
    $p = Start-Process -FilePath (Get-Process -Id $PID).Path -ArgumentList (@('-NoProfile', '-File', $script) + $a) `
        -Wait -PassThru -NoNewWindow -RedirectStandardOutput (Join-Path $work 'out.txt') -RedirectStandardError (Join-Path $work 'err.txt')
    return $p.ExitCode
}

try {
    # Fake package (zip) for this machine.
    $pkgRoot = Join-Path $work 'pkg'
    $inner = Join-Path $pkgRoot 'semantic-engine'
    New-Item -ItemType Directory -Path (Join-Path $inner 'vocabulary') -Force | Out-Null
    Set-Content -Path (Join-Path $inner 'semantic-engine.yml') -Value "name: Semantic Engine`nversion: 9.9.9"
    Set-Content -Path (Join-Path $inner 'semantic-engine.exe') -Value 'fake'
    Set-Content -Path (Join-Path $inner 'VERSIONS.json') -Value '{"engineVersion":"9.9.9"}'
    $pkg = Join-Path $work 'semantic-engine-9.9.9-test.zip'
    Compress-Archive -Path $inner -DestinationPath $pkg
    $hash = (Get-FileHash -Algorithm SHA256 $pkg).Hash.ToLowerInvariant()
    Set-Content -Path "$pkg.sha256" -Value "$hash  semantic-engine-9.9.9-test.zip"

    if ((Run @('-Help')) -eq 0) { Ok '-Help exits 0' } else { Bad '-Help exits 0' }

    $plugins = Join-Path $work 'plugins'
    New-Item -ItemType Directory -Path $plugins | Out-Null
    Set-Content -Path (Join-Path $plugins 'other-plugin.yml') -Value 'keep'
    $env:SE_TEST_SKIP_PLATFORM = '1'   # the fake package installs on any OS in tests
    $code = Run @('-FromFile', $pkg, '-PluginsDir', $plugins, '-Yes')
    if ($code -eq 0 -and (Test-Path (Join-Path $plugins 'semantic-engine/semantic-engine.yml'))) { Ok 'installs from file' }
    else { Bad "installs from file (exit $code)"; Get-Content (Join-Path $work 'out.txt'), (Join-Path $work 'err.txt') }

    Set-Content -Path (Join-Path $plugins 'semantic-engine/stale-file') -Value 'stale'
    Run @('-FromFile', $pkg, '-PluginsDir', $plugins, '-Yes') | Out-Null
    if (-not (Test-Path (Join-Path $plugins 'semantic-engine/stale-file')) -and ((Get-Content (Join-Path $plugins 'other-plugin.yml')) -eq 'keep')) {
        Ok 're-run replaces plugin files only'
    } else { Bad 're-run replaces plugin files only' }

    $bad = Join-Path $work 'bad.zip'
    Copy-Item $pkg $bad
    Set-Content -Path "$bad.sha256" -Value ('0' * 64 + '  bad.zip')
    $plugins2 = Join-Path $work 'plugins2'
    New-Item -ItemType Directory -Path $plugins2 | Out-Null
    $code = Run @('-FromFile', $bad, '-PluginsDir', $plugins2, '-Yes')
    if ($code -eq 5 -and -not (Get-ChildItem $plugins2)) { Ok 'bad checksum exits 5' } else { Bad "bad checksum exits 5 (exit $code)" }

    $code = Run @('-FromFile', $pkg, '-PluginsDir', (Join-Path $work 'missing'), '-Yes')
    if ($code -eq 4) { Ok 'missing dir exits 4' } else { Bad "missing dir exits 4 (exit $code)" }

    $code = Run @('-Bogus')
    if ($code -eq 2) { Ok 'bad arguments exit 2' } else { Bad "bad arguments exit 2 (exit $code)" }

    Remove-Item Env:\SE_TEST_SKIP_PLATFORM
    if (-not $IsWindows -and $PSVersionTable.PSVersion.Major -ge 6) {
        $code = Run @('-FromFile', $pkg, '-PluginsDir', $plugins, '-Yes')
        if ($code -eq 3) { Ok 'non-Windows platform exits 3' } else { Bad "non-Windows platform exits 3 (exit $code)" }
    }
}
finally {
    Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
}
Write-Host "$pass passed, $fail failed (pwsh)"
if ($fail -gt 0) { exit 1 } else { exit 0 }
