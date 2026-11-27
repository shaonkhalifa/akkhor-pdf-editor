# Build the OCR language pack installer (SRS FR-10.3).
#
#   powershell -File scripts\build-ocr-pack.ps1 [-Version 1.0.0] [-DataFrom D:\dev\akkhor-tessdata\best]
#
# Gets the Bangla and English models from the Tesseract project's
# tessdata_best repository (Apache-2.0), unless -DataFrom points at a folder
# that already has ben.traineddata and eng.traineddata, then builds
# ocr-pack\dist\Akkhor-OCR-Pack-ben-eng-<version>.exe with NSIS.
# Upload that file to a GitHub Release. The app itself never downloads it.

param(
    [string]$Version = "1.0.0",
    [string]$DataFrom = ""
)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$pack = Join-Path $root "ocr-pack"
$data = Join-Path $pack "data"
New-Item -ItemType Directory -Force $data, (Join-Path $pack "dist") | Out-Null

$base = "https://raw.githubusercontent.com/tesseract-ocr/tessdata_best/main"
foreach ($lang in "ben", "eng") {
    $dest = Join-Path $data "$lang.traineddata"
    if ($DataFrom -and (Test-Path (Join-Path $DataFrom "$lang.traineddata"))) {
        Copy-Item (Join-Path $DataFrom "$lang.traineddata") $dest -Force
    } elseif (-not (Test-Path $dest)) {
        Write-Host "Downloading $lang.traineddata from tessdata_best..."
        Invoke-WebRequest "$base/$lang.traineddata" -OutFile $dest
    }
}
$license = Join-Path $data "LICENSE-tessdata.txt"
if ($DataFrom -and (Test-Path (Join-Path (Split-Path $DataFrom) "LICENSE-tessdata.txt"))) {
    Copy-Item (Join-Path (Split-Path $DataFrom) "LICENSE-tessdata.txt") $license -Force
} elseif (-not (Test-Path $license)) {
    Invoke-WebRequest "$base/LICENSE" -OutFile $license
}

# NSIS: the copy Tauri installs, or one on PATH.
$makensis = @(
    (Join-Path $env:LOCALAPPDATA "tauri\NSIS\makensis.exe"),
    "C:\Program Files (x86)\NSIS\makensis.exe",
    "C:\Program Files\NSIS\makensis.exe"
) | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $makensis) { $makensis = (Get-Command makensis.exe -ErrorAction Stop).Source }

Push-Location $pack
try {
    & $makensis "/DPACK_VERSION=$Version" "akkhor-ocr-pack.nsi"
    if ($LASTEXITCODE -ne 0) { throw "makensis failed ($LASTEXITCODE)" }
} finally {
    Pop-Location
}
$out = Join-Path $pack "dist\Akkhor-OCR-Pack-ben-eng-$Version.exe"
$hash = (Get-FileHash $out -Algorithm SHA256).Hash
Write-Host "Built $out"
Write-Host "SHA-256: $hash"
