# Packages the release build as an MSIX for the Microsoft Store: the tray app
# (from `npx tauri build`), with msix/AppxManifest.xml and msix/Assets. The
# Store signs the package; this script doesn't.
#
#   make msix                         (from the repo root), or
#   npm run msix                      (in windows/app): builds, then packages
#   scripts/package-msix.ps1 [-Build] [-Out <folder>]
#
# -Build runs the build first; without it, it must already be there.
# The identity defaults to QC Buddy's (Partner Center -> QCBuddy -> Product
# identity; it's public, in every package the Store ships). Writes
# <Out>/QCBuddy_<version>.0_x64.msix (default Out: src-tauri/target/msix) and
# leaves the staged files in <Out>/msix: `Add-AppxPackage -Register
# <Out>\msix\AppxManifest.xml` installs them unsigned, with Developer Mode on.

param(
    [string] $IdentityName = "HangkunUng.QCBuddy",
    [string] $Publisher = "CN=2B0185DD-C3A5-4E7C-8B91-7F00251BD60B",
    [string] $PublisherDisplayName = "Hangkun Ung",
    [string] $Out,
    [switch] $Build
)

$ErrorActionPreference = "Stop"
$app = Split-Path $PSScriptRoot                      # windows/app
if (-not $Out) { $Out = Join-Path $app "src-tauri/target/msix" }

if ($Build) {
    # Each step's exit code, since native commands don't stop the script.
    function Invoke-Step([string] $what, [scriptblock] $step) {
        Write-Output "== $what"
        & $step
        if ($LASTEXITCODE -ne 0) { throw "$what failed ($LASTEXITCODE)" }
    }
    Push-Location $app
    try {
        Invoke-Step "npm ci" { npm ci }
        Invoke-Step "tray app (npx tauri build)" { npx tauri build }
    } finally {
        Pop-Location
    }
}

$tray = Join-Path $app "src-tauri/target/release/QCBuddy.exe"
if (-not (Test-Path $tray)) { throw "$tray is missing: run with -Build, or build the app (npx tauri build) first." }

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
