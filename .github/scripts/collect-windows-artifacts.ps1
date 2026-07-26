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

$PortableExecutable = Join-Path "src-tauri/target/$Target/release" "geotagger.exe"
if (-not (Test-Path -LiteralPath $PortableExecutable -PathType Leaf)) {
    throw "Portable Windows executable is missing: $PortableExecutable"
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
$OutputName = "GeoTagger-$Version-windows-x64.exe"
$OutputPath = Join-Path $OutputFull $OutputName
Copy-Item -LiteralPath $PortableExecutable -Destination $OutputPath

$Hash = (Get-FileHash -LiteralPath $OutputPath -Algorithm SHA256).Hash.ToLowerInvariant()
"$Hash  $OutputName" | Set-Content -LiteralPath (Join-Path $OutputFull "SHA256SUMS") -Encoding ascii

Write-Host "Collected unsigned Windows artifacts in $OutputFull"
