[CmdletBinding()]
param(
    [string]$ExifToolDirectory = "",
    [string]$FixturePath = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

$ExpectedVersion = "13.59"
$ExpectedLatitude = 31.2304
$ExpectedLongitude = 121.4737
$ExpectedAltitude = 12.3
$Workspace = if ($env:GITHUB_WORKSPACE) { $env:GITHUB_WORKSPACE } else { (Get-Location).Path }
$RunnerTemp = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { [System.IO.Path]::GetTempPath() }
$InvariantCulture = [System.Globalization.CultureInfo]::InvariantCulture

if (-not $ExifToolDirectory) {
    $ExifToolDirectory = Join-Path $Workspace "src-tauri/resources/exiftool"
}
if (-not $FixturePath) {
    $FixturePath = Join-Path $RunnerTemp "geotagger-exiftool-fixture.jpg"
}

$ExifToolExe = Join-Path $ExifToolDirectory "exiftool.exe"
$ExifToolFiles = Join-Path $ExifToolDirectory "exiftool_files"
if (-not (Test-Path -LiteralPath $ExifToolExe -PathType Leaf)) {
    throw "ExifTool executable is missing: $ExifToolExe"
}
if (-not (Test-Path -LiteralPath $ExifToolFiles -PathType Container)) {
    throw "ExifTool companion directory is missing: $ExifToolFiles"
}
if (-not (Test-Path -LiteralPath $FixturePath -PathType Leaf)) {
    throw "ExifTool fixture is missing: $FixturePath"
}

$ActualVersion = (& $ExifToolExe -ver).Trim()
if ($LASTEXITCODE -ne 0 -or $ActualVersion -ne $ExpectedVersion) {
    throw "Expected ExifTool $ExpectedVersion, got $ActualVersion"
}

$WorkDir = Join-Path $RunnerTemp "geotagger-exiftool-smoke-$([guid]::NewGuid().ToString('N'))"
New-Item -ItemType Directory -Path $WorkDir | Out-Null

try {
    $SourcePhoto = Join-Path $WorkDir "源 照片.jpg"
    $OutputPhoto = Join-Path $WorkDir "输出 照片.jpg"
    Copy-Item -LiteralPath $FixturePath -Destination $SourcePhoto
    Copy-Item -LiteralPath $SourcePhoto -Destination $OutputPhoto

    $SourceHashBefore = (Get-FileHash -LiteralPath $SourcePhoto -Algorithm SHA256).Hash

    & $ExifToolExe `
        -charset filename=UTF8 `
        -overwrite_original `
        -n `
        "-GPSLatitude=$ExpectedLatitude" `
        -GPSLatitudeRef=N `
        "-GPSLongitude=$ExpectedLongitude" `
        -GPSLongitudeRef=E `
        "-GPSAltitude=$ExpectedAltitude" `
        $OutputPhoto
    if ($LASTEXITCODE -ne 0) {
        throw "ExifTool failed to write the smoke-test photo"
    }

    [string[]]$Values = & $ExifToolExe `
        -charset filename=UTF8 `
        -n `
        -s3 `
        -GPSLatitude `
        -GPSLongitude `
        -GPSAltitude `
        $OutputPhoto
    if ($LASTEXITCODE -ne 0 -or $Values.Count -lt 3) {
        throw "ExifTool failed to read back all GPS values"
    }

    $Latitude = [double]::Parse($Values[0], $InvariantCulture)
    $Longitude = [double]::Parse($Values[1], $InvariantCulture)
    $Altitude = [double]::Parse($Values[2], $InvariantCulture)

    if ([Math]::Abs($Latitude - $ExpectedLatitude) -gt 0.000001) {
        throw "Latitude verification failed: $Latitude"
    }
    if ([Math]::Abs($Longitude - $ExpectedLongitude) -gt 0.000001) {
        throw "Longitude verification failed: $Longitude"
    }
    if ([Math]::Abs($Altitude - $ExpectedAltitude) -gt 0.01) {
        throw "Altitude verification failed: $Altitude"
    }

    $SourceHashAfter = (Get-FileHash -LiteralPath $SourcePhoto -Algorithm SHA256).Hash
    if ($SourceHashBefore -ne $SourceHashAfter) {
        throw "ExifTool smoke test modified the source fixture"
    }

    Write-Host "ExifTool $ActualVersion write/read smoke test passed"
}
finally {
    if (Test-Path -LiteralPath $WorkDir) {
        Remove-Item -LiteralPath $WorkDir -Recurse -Force
    }
}
