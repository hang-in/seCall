# seCall installer for Windows — downloads the latest release and places it on PATH.
#
#   irm https://raw.githubusercontent.com/hang-in/seCall/main/install.ps1 | iex
#
# Environment overrides:
#   $env:SECALL_VERSION   Pin a release tag (e.g. v0.6.2). Default: latest release.
#   $env:SECALL_INSTALL   Install directory. Default: %LOCALAPPDATA%\secall\bin
$ErrorActionPreference = 'Stop'

$Repo = 'hang-in/seCall'
$Target = 'x86_64-pc-windows-msvc'
$InstallDir = if ($env:SECALL_INSTALL) { $env:SECALL_INSTALL } else { Join-Path $env:LOCALAPPDATA 'secall\bin' }

function Info($msg) { Write-Host "  $msg" }

# Resolve version (env override or latest release tag).
if ($env:SECALL_VERSION) {
    $Version = $env:SECALL_VERSION
} else {
    Info 'Resolving latest release...'
    $latest = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest"
    $Version = $latest.tag_name
    if (-not $Version) { throw 'could not determine latest release tag' }
}

$Asset = "secall-$Target.zip"
$Url = "https://github.com/$Repo/releases/download/$Version/$Asset"

Info "Installing seCall $Version ($Target)"
Info "From: $Url"

# Detect an existing install (used later to warn about shadowing copies).
$prev = Get-Command secall -CommandType Application -ErrorAction SilentlyContinue
if ($prev) {
    $prevVer = try { (& $prev.Source --version) 2>$null } catch { $null }
    Info "Existing install: $($prev.Source) $prevVer"
}

# A running secall.exe is file-locked and cannot be overwritten — fail fast with a clear message.
$running = Get-Process -Name secall -ErrorAction SilentlyContinue
if ($running) {
    throw "secall is running (PID $($running.Id -join ', ')). Stop it (e.g. 'secall serve' / MCP server) and re-run."
}

$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("secall-" + [System.Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $tmp -Force | Out-Null
try {
    $zip = Join-Path $tmp $Asset
    Invoke-WebRequest -UseBasicParsing -Uri $Url -OutFile $zip
    Expand-Archive -Path $zip -DestinationPath $tmp -Force

    $exe = Join-Path $tmp 'secall.exe'
    if (-not (Test-Path $exe)) { throw "binary 'secall.exe' not found in $Asset" }

    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    # Copy both secall.exe and onnxruntime.dll — the DLL must sit beside the binary.
    Copy-Item -Path (Join-Path $tmp 'secall.exe') -Destination $InstallDir -Force
    $dll = Join-Path $tmp 'onnxruntime.dll'
    if (Test-Path $dll) {
        Copy-Item -Path $dll -Destination $InstallDir -Force
    }
} finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}

Info "Installed to: $InstallDir\secall.exe"

# Add install dir to the user PATH if missing.
# Write via the registry preserving the original value kind: if the user PATH is
# REG_EXPAND_SZ, plain SetEnvironmentVariable would rewrite it as REG_SZ and freeze
# %VAR% entries (e.g. %USERPROFILE%\bin) into literal paths. Reading with
# DoNotExpandEnvironmentNames keeps those tokens intact.
$key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $true)
if (-not $key) { $key = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey('Environment') }
try {
    $rawPath = ''
    $kind = [Microsoft.Win32.RegistryValueKind]::ExpandString  # Windows default for user PATH
    if ($key.GetValueNames() -contains 'Path') {
        $rawPath = [string]$key.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
        $kind = $key.GetValueKind('Path')
    }

    $entries = @($rawPath -split ';' | Where-Object { $_ -ne '' })
    $already = $entries | Where-Object { $_.TrimEnd('\') -ieq $InstallDir.TrimEnd('\') }
    if (-not $already) {
        $newPath = if ($rawPath) { "$rawPath;$InstallDir" } else { $InstallDir }
        $key.SetValue('Path', $newPath, $kind)
        Info "Added $InstallDir to your user PATH (restart your terminal to pick it up)."
    }
} finally {
    if ($key) { $key.Close() }
}

# Warn if a different secall on PATH would shadow the one just installed.
$targetBin = Join-Path $InstallDir 'secall.exe'
if ($prev -and ($prev.Source -ne $targetBin)) {
    Write-Host ''
    Info "WARNING: another 'secall' exists at $($prev.Source)"
    Info "         Depending on PATH order it may shadow the version just installed."
    Info "         Remove the old copy if you want this one to take effect."
}

Write-Host ''
Info 'Done. Next steps:'
Info '  secall init      # interactive setup'
Info '  secall --help'
