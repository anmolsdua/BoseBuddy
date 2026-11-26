# Packages the release build as an MSIX for the Microsoft Store: the tray app
# (from `npx tauri build`) and the CLI (from `cargo build --release` in
# windows/core, shipped as qcb.exe), with msix/AppxManifest.xml and
# msix/Assets. The Store signs the package; this script doesn't.
#
#   pwsh scripts/package-msix.ps1 -IdentityName <Package/Identity/Name> `
#     -Publisher "CN=..." -PublisherDisplayName "<name>" [-Out <folder>]
#
# The three identity values are on Partner Center → QCBuddy → Product
# identity. Writes <Out>/QCBuddy_<version>.0_x64.msix (default Out:
# src-tauri/target/msix) and leaves the staged files in <Out>/msix:
# `Add-AppxPackage -Register <Out>\msix\AppxManifest.xml` installs them
# unsigned, with Developer Mode on.

param(
    [Parameter(Mandatory)] [string] $IdentityName,
    [Parameter(Mandatory)] [string] $Publisher,
    [Parameter(Mandatory)] [string] $PublisherDisplayName,
    [string] $Out
)

$ErrorActionPreference = "Stop"
$app = Split-Path $PSScriptRoot                      # windows/app
if (-not $Out) { $Out = Join-Path $app "src-tauri/target/msix" }
$tray = Join-Path $app "src-tauri/target/release/QCBuddy.exe"
$cli = Join-Path $app "../core/target/release/qcbuddy.exe"
foreach ($exe in $tray, $cli) {
    if (-not (Test-Path $exe)) { throw "$exe is missing: build the app (npx tauri build) and the CLI (cargo build --release in windows/core) first." }
}

# The Store wants four parts, the last one 0.
$version = (Get-Content (Join-Path $app "src-tauri/tauri.conf.json") -Raw | ConvertFrom-Json).version
if ($version -notmatch '^\d+\.\d+\.\d+$') { throw "tauri.conf.json version must be x.y.z, got $version" }
$msixVersion = "$version.0"

# makeappx and makepri from the newest Windows SDK.
$sdkBin = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\10.*\x64\makeappx.exe" -ErrorAction SilentlyContinue |
    Sort-Object { [version]$_.Directory.Parent.Name } | Select-Object -Last 1
if (-not $sdkBin) { throw "makeappx.exe not found: install the Windows 10/11 SDK." }
$makeappx = $sdkBin.FullName
$makepri = Join-Path $sdkBin.DirectoryName "makepri.exe"

New-Item -ItemType Directory -Force $Out | Out-Null
$Out = (Resolve-Path $Out).Path
$stage = Join-Path $Out "msix"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory $stage | Out-Null

Copy-Item $tray (Join-Path $stage "QCBuddy.exe")
Copy-Item $cli (Join-Path $stage "qcb.exe")
Copy-Item -Recurse (Join-Path $app "msix/Assets") (Join-Path $stage "Assets")

$manifest = Get-Content (Join-Path $app "msix/AppxManifest.xml") -Raw
$values = @{
    "{{IdentityName}}"         = $IdentityName
    "{{Publisher}}"            = $Publisher
    "{{PublisherDisplayName}}" = $PublisherDisplayName
    "{{Version}}"              = $msixVersion
}
foreach ($key in $values.Keys) {
    $manifest = $manifest.Replace($key, [Security.SecurityElement]::Escape($values[$key]))
}
if ($manifest -match '\{\{') { throw "AppxManifest.xml has a value this script doesn't fill in." }
[IO.File]::WriteAllText((Join-Path $stage "AppxManifest.xml"), $manifest)

# resources.pri, so Windows finds the targetsize icons (taskbar, Start).
$priConfig = Join-Path $Out "priconfig.xml"
& $makepri createconfig /cf $priConfig /dq en-US /pv 10.0.0 /o | Out-Null
if ($LASTEXITCODE -ne 0) { throw "makepri createconfig failed ($LASTEXITCODE)" }
& $makepri new /pr $stage /cf $priConfig /mn (Join-Path $stage "AppxManifest.xml") /of (Join-Path $stage "resources.pri") /o
if ($LASTEXITCODE -ne 0) { throw "makepri new failed ($LASTEXITCODE)" }
Remove-Item $priConfig

$msix = Join-Path $Out "QCBuddy_${msixVersion}_x64.msix"
& $makeappx pack /d $stage /p $msix /o
if ($LASTEXITCODE -ne 0) { throw "makeappx pack failed ($LASTEXITCODE)" }
Write-Output "Packaged $msix"
