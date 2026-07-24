[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$OutputDirectory,
    [Parameter(Mandatory = $true)]
    [string]$Target,
    [Parameter(Mandatory = $true)]
    [string]$Version
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

$BundleRoot = Join-Path "src-tauri/target/$Target/release/bundle" "nsis"
if (-not (Test-Path -LiteralPath $BundleRoot -PathType Container)) {
    throw "NSIS bundle directory is missing: $BundleRoot"
}

$Installer = Get-ChildItem -LiteralPath $BundleRoot -File -Filter "*.exe" |
    Where-Object { $_.Name -like "*-setup.exe" } |
    Select-Object -First 1
if (-not $Installer) {
    throw "No NSIS setup executable was produced"
}

$SevenZip = (Get-Command "7z.exe" -ErrorAction Stop).Source
$InstallerListing = (& $SevenZip l -slt $Installer.FullName | Out-String)
if ($LASTEXITCODE -ne 0) {
    throw "7-Zip could not inspect the generated NSIS installer"
}
if ($InstallerListing -notmatch "exiftool\.exe") {
    throw "The generated NSIS installer does not contain exiftool.exe"
}
if ($InstallerListing -notmatch "exiftool_files") {
    throw "The generated NSIS installer does not contain exiftool_files"
}

$BundledExifTool = Join-Path "src-tauri/resources/exiftool" "exiftool.exe"
$BundledExifToolFiles = Join-Path "src-tauri/resources/exiftool" "exiftool_files"
if (-not (Test-Path -LiteralPath $BundledExifTool -PathType Leaf)) {
    throw "Prepared ExifTool executable is missing"
}
if (-not (Test-Path -LiteralPath $BundledExifToolFiles -PathType Container)) {
    throw "Prepared exiftool_files directory is missing"
}

$ActualVersion = (& $BundledExifTool -ver).Trim()
if ($LASTEXITCODE -ne 0 -or $ActualVersion -ne "13.59") {
    throw "Prepared ExifTool version mismatch: $ActualVersion"
}

New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$OutputFull = [System.IO.Path]::GetFullPath($OutputDirectory)
$OutputName = "GeoTagger-$Version-windows-x64-setup-UNSIGNED.exe"
$OutputPath = Join-Path $OutputFull $OutputName
Copy-Item -LiteralPath $Installer.FullName -Destination $OutputPath

$Hash = (Get-FileHash -LiteralPath $OutputPath -Algorithm SHA256).Hash.ToLowerInvariant()
"$Hash  $OutputName" | Set-Content -LiteralPath (Join-Path $OutputFull "SHA256SUMS") -Encoding ascii

Write-Host "Collected unsigned Windows artifacts in $OutputFull"
