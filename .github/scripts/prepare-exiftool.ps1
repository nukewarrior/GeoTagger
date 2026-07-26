[CmdletBinding()]
param(
    [string]$Destination = "",
    [string]$FixturePath = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

$ExifToolVersion = "13.59"
$ExifToolPayloadSha256 = "8ca3846bed2cf8dc9bcd90ebb118f987f015ddfbe76f46c973b315a71e03ecdb"
$ExifToolUrl = "https://sourceforge.net/projects/exiftool/files/exiftool-$($ExifToolVersion)_64.zip/download"
$Workspace = if ($env:GITHUB_WORKSPACE) { $env:GITHUB_WORKSPACE } else { (Get-Location).Path }
$RunnerTemp = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { [System.IO.Path]::GetTempPath() }

if (-not $Destination) {
    $Destination = Join-Path $Workspace "src-tauri/resources/exiftool"
}
if (-not $FixturePath) {
    $FixturePath = Join-Path $RunnerTemp "geotagger-exiftool-fixture.jpg"
}

$DestinationFull = [System.IO.Path]::GetFullPath($Destination)
$ExpectedDestination = [System.IO.Path]::GetFullPath(
    (Join-Path $Workspace "src-tauri/resources/exiftool")
)
if ($DestinationFull -ne $ExpectedDestination) {
    throw "Refusing to replace unexpected ExifTool destination: $DestinationFull"
}

$WorkDir = Join-Path $RunnerTemp "geotagger-exiftool-$([guid]::NewGuid().ToString('N'))"
New-Item -ItemType Directory -Path $WorkDir | Out-Null

try {
    $Archive = Join-Path $WorkDir "exiftool-$($ExifToolVersion)_64.zip"
    Invoke-WebRequest -Uri $ExifToolUrl -OutFile $Archive -MaximumRedirection 10

    $Extracted = Join-Path $WorkDir "extracted"
    Expand-Archive -LiteralPath $Archive -DestinationPath $Extracted

    $SourceExe = Get-ChildItem -LiteralPath $Extracted -Recurse -File |
        Where-Object { $_.Name -eq "exiftool(-k).exe" } |
        Select-Object -First 1
    if (-not $SourceExe) {
        throw "The official ExifTool executable was not found in the archive"
    }

    $SourceFiles = Join-Path $SourceExe.Directory.FullName "exiftool_files"
    if (-not (Test-Path -LiteralPath $SourceFiles -PathType Container)) {
        throw "The required exiftool_files directory was not found"
    }

    $PayloadRoot = $SourceExe.Directory.FullName
    $PayloadManifest = Get-ChildItem -LiteralPath $PayloadRoot -Recurse -File |
        Sort-Object FullName |
        ForEach-Object {
            $RelativePath = $_.FullName.Substring($PayloadRoot.Length + 1).Replace("\", "/")
            "$RelativePath $((Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant())"
        }
    $PayloadManifestPath = Join-Path $WorkDir "payload.sha256"
    $PayloadText = (($PayloadManifest -join "`n") + "`n")
    [System.IO.File]::WriteAllText(
        $PayloadManifestPath,
        $PayloadText,
        [System.Text.UTF8Encoding]::new($false)
    )
    $ActualPayloadHash = (Get-FileHash -LiteralPath $PayloadManifestPath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($ActualPayloadHash -ne $ExifToolPayloadSha256) {
        throw "ExifTool payload checksum mismatch. Expected $ExifToolPayloadSha256, got $ActualPayloadHash"
    }

    if (Test-Path -LiteralPath $DestinationFull) {
        Remove-Item -LiteralPath $DestinationFull -Recurse -Force
    }
    New-Item -ItemType Directory -Path $DestinationFull | Out-Null
    Copy-Item -LiteralPath $SourceExe.FullName -Destination (Join-Path $DestinationFull "exiftool.exe")
    Copy-Item -LiteralPath $SourceFiles -Destination (Join-Path $DestinationFull "exiftool_files") -Recurse

    $FixtureDirectory = Split-Path -Parent $FixturePath
    New-Item -ItemType Directory -Path $FixtureDirectory -Force | Out-Null
    $FixtureBase64 = "/9j/4AAQSkZJRgABAQAAAQABAAD/2wBDAP//////////////////////////////////////////////////////////////////////////////////////2wBDAf//////////////////////////////////////////////////////////////////////////////////////wAARCAABAAEDASIAAhEBAxEB/8QAFQABAQAAAAAAAAAAAAAAAAAAAAf/xAAUEAEAAAAAAAAAAAAAAAAAAAAA/9oADAMBAAIQAxAAAAF//8QAFBABAAAAAAAAAAAAAAAAAAAAAP/aAAgBAQABBQJ//8QAFBEBAAAAAAAAAAAAAAAAAAAAAP/aAAgBAwEBPwF//8QAFBEBAAAAAAAAAAAAAAAAAAAAAP/aAAgBAgEBPwF//8QAFBABAAAAAAAAAAAAAAAAAAAAAP/aAAgBAQAGPwJ//8QAFBABAAAAAAAAAAAAAAAAAAAAAP/aAAgBAQABPyF//9oADAMBAAIAAwAAABAf/8QAFBEBAAAAAAAAAAAAAAAAAAAAAP/aAAgBAwEBPxB//8QAFBEBAAAAAAAAAAAAAAAAAAAAAP/aAAgBAgEBPxB//8QAFBABAAAAAAAAAAAAAAAAAAAAAP/aAAgBAQABPxB//9k="
    [System.IO.File]::WriteAllBytes($FixturePath, [Convert]::FromBase64String($FixtureBase64))

    $ExifToolExe = Join-Path $DestinationFull "exiftool.exe"
    $ActualVersion = (& $ExifToolExe -ver).Trim()
    if ($LASTEXITCODE -ne 0 -or $ActualVersion -ne $ExifToolVersion) {
        throw "Expected ExifTool $ExifToolVersion, got $ActualVersion"
    }

    Write-Host "Prepared ExifTool $ActualVersion at $DestinationFull"
}
finally {
    if (Test-Path -LiteralPath $WorkDir) {
        Remove-Item -LiteralPath $WorkDir -Recurse -Force
    }
}
