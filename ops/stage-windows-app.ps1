# Stage CEF's sandbox-owning bootstrap, the iced client DLL, node sidecar, and
# the exact CEF runtime copied beside them by cef-dll-sys. The result is a
# relocatable directory plus a portable zip; -Install also installs it for the
# current user and refreshes the Start-menu shortcut.
[CmdletBinding()]
param(
    [ValidateSet("debug", "release")]
    [string]$Configuration = "release",
    [switch]$Install,
    [switch]$NoArchive
)

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot
Set-Location $repo

$targetDir = Join-Path $repo "target\$Configuration"
$bundleRoot = Join-Path $targetDir "bundle\windows"
$stage = Join-Path $bundleRoot "Ducktape"
$bootstrapSource = Join-Path $targetDir "bootstrap.exe"
$clientSource = Join-Path $targetDir "ducktape_iced.dll"
$nodeSource = Join-Path $targetDir "ducktape-node.exe"
$manifestSource = Join-Path $repo "app\src-iced\assets\windows\Ducktape.exe.manifest"
$iconSource = Join-Path $repo "app\src-iced\assets\icons\icon.ico"

if ($Install -and $Configuration -ne "release") {
    throw "[windows-app] refusing to install a non-release build"
}

foreach ($required in @($bootstrapSource, $clientSource, $nodeSource, $manifestSource, $iconSource)) {
    if (-not (Test-Path -LiteralPath $required -PathType Leaf)) {
        throw "[windows-app] missing $required; build the iced --lib and node targets first"
    }
    if ((Get-Item -LiteralPath $required).Length -eq 0) {
        throw "[windows-app] refusing empty artifact $required"
    }
}

$manifest = Get-Content -LiteralPath $manifestSource -Raw
if ($manifest -notmatch 'requestedExecutionLevel\s+level="asInvoker"' -or
    $manifest -match 'requireAdministrator|highestAvailable') {
    throw "[windows-app] manifest must remain rootless (asInvoker)"
}
$mt = Get-Command "mt.exe" -ErrorAction SilentlyContinue
if ($null -eq $mt) {
    throw "[windows-app] mt.exe is required to embed and verify the rootless PE manifest"
}

if (Test-Path -LiteralPath $stage) {
    Remove-Item -LiteralPath $stage -Recurse -Force
}
New-Item -ItemType Directory -Path $stage -Force | Out-Null
Copy-Item -LiteralPath $bootstrapSource -Destination (Join-Path $stage "Ducktape.exe")
Copy-Item -LiteralPath $clientSource -Destination (Join-Path $stage "Ducktape.dll")
Copy-Item -LiteralPath $nodeSource -Destination (Join-Path $stage "ducktape-node.exe")
Copy-Item -LiteralPath $manifestSource -Destination (Join-Path $stage "Ducktape.exe.manifest")
Copy-Item -LiteralPath $iconSource -Destination (Join-Path $stage "Ducktape.ico")

$stagedExe = Join-Path $stage "Ducktape.exe"
& $mt.Source "-nologo" "-manifest" $manifestSource "-outputresource:$stagedExe;#1"
if ($LASTEXITCODE -ne 0) {
    throw "[windows-app] failed to embed the rootless manifest in Ducktape.exe"
}
$effectiveManifest = Join-Path $bundleRoot "Ducktape.effective.manifest"
& $mt.Source "-nologo" "-inputresource:$stagedExe;#1" "-out:$effectiveManifest"
if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $effectiveManifest -PathType Leaf)) {
    throw "[windows-app] failed to read Ducktape.exe's effective manifest"
}
$effective = Get-Content -LiteralPath $effectiveManifest -Raw
Remove-Item -LiteralPath $effectiveManifest -Force
if ($effective -notmatch 'requestedExecutionLevel\s+level="asInvoker"' -or
    $effective -match 'requireAdministrator|highestAvailable') {
    throw "[windows-app] Ducktape.exe's effective manifest is not rootless"
}

# cef-dll-sys copies the target-specific distribution to the Cargo target. Copy
# runtime-shaped files rather than a version-specific list: CEF occasionally
# adds a graphics DLL, while build metadata and SDK sources must never ship.
$runtimePatterns = @("*.dll", "*.bin", "*.dat", "*.pak")
foreach ($pattern in $runtimePatterns) {
    Get-ChildItem -LiteralPath $targetDir -Filter $pattern -File | ForEach-Object {
        # The Rust cdylib must only appear under the bootstrap's matching name.
        if ($_.Name -ne "ducktape_iced.dll") {
            Copy-Item -LiteralPath $_.FullName -Destination $stage
        }
    }
}
foreach ($runtimeJson in @("vk_swiftshader_icd.json")) {
    $source = Join-Path $targetDir $runtimeJson
    if (Test-Path -LiteralPath $source -PathType Leaf) {
        Copy-Item -LiteralPath $source -Destination $stage
    }
}

$locales = Join-Path $targetDir "locales"
if (-not (Test-Path -LiteralPath $locales -PathType Container)) {
    throw "[windows-app] CEF locales are missing from $targetDir"
}
Copy-Item -LiteralPath $locales -Destination $stage -Recurse

foreach ($required in @(
    "Ducktape.exe",
    "Ducktape.dll",
    "Ducktape.exe.manifest",
    "Ducktape.ico",
    "libcef.dll",
    "chrome_elf.dll",
    "icudtl.dat",
    "resources.pak",
    "v8_context_snapshot.bin",
    "locales\en-US.pak"
)) {
    if (-not (Test-Path -LiteralPath (Join-Path $stage $required) -PathType Leaf)) {
        throw "[windows-app] required CEF runtime file is missing: $required"
    }
}

if (Test-Path -LiteralPath (Join-Path $stage "ducktape_iced.dll") -PathType Leaf) {
    throw "[windows-app] unrenamed Rust client DLL escaped into the package"
}

foreach ($pair in @(
    [PSCustomObject]@{ Source = $clientSource; Staged = (Join-Path $stage "Ducktape.dll") }
)) {
    if ((Get-FileHash -Algorithm SHA256 -LiteralPath $pair.Source).Hash -ne
        (Get-FileHash -Algorithm SHA256 -LiteralPath $pair.Staged).Hash) {
        throw "[windows-app] staged bootstrap/client identity check failed"
    }
}

$signThumbprint = $env:DUCKTAPE_WINDOWS_SIGN_SHA1
$timestampUrl = $env:DUCKTAPE_WINDOWS_TIMESTAMP_URL
$signed = -not [string]::IsNullOrWhiteSpace($signThumbprint)
if ($signed) {
    if ([string]::IsNullOrWhiteSpace($timestampUrl)) {
        throw "[windows-app] DUCKTAPE_WINDOWS_TIMESTAMP_URL is required for a signed package"
    }
    $signtool = Get-Command "signtool.exe" -ErrorAction SilentlyContinue
    if ($null -eq $signtool) {
        throw "[windows-app] signtool.exe is required for a signed package"
    }
    foreach ($file in @(
        (Join-Path $stage "Ducktape.exe"),
        (Join-Path $stage "Ducktape.dll"),
        (Join-Path $stage "ducktape-node.exe")
    )) {
        & $signtool.Source sign /sha1 $signThumbprint /fd SHA256 /tr $timestampUrl /td SHA256 $file
        if ($LASTEXITCODE -ne 0) {
            throw "[windows-app] Authenticode signing failed for $file"
        }
        & $signtool.Source verify /pa /all $file
        if ($LASTEXITCODE -ne 0) {
            throw "[windows-app] Authenticode verification failed for $file"
        }
    }
    Write-Host "[windows-app] signed and verified app-owned PE files"
} elseif (-not [string]::IsNullOrWhiteSpace($timestampUrl)) {
    throw "[windows-app] DUCKTAPE_WINDOWS_SIGN_SHA1 is required when a timestamp URL is set"
} else {
    Write-Warning "[windows-app] local package is unsigned and is not a distribution artifact"
}

Write-Host "[windows-app] staged sandbox bootstrap + client DLL at $stage"
if (-not $NoArchive) {
    $suffix = if ($signed) { "" } else { "-unsigned" }
    $zip = Join-Path $bundleRoot "Ducktape-windows-$env:PROCESSOR_ARCHITECTURE$suffix.zip"
    if (Test-Path -LiteralPath $zip) {
        Remove-Item -LiteralPath $zip -Force
    }
    Compress-Archive -Path $stage -DestinationPath $zip -CompressionLevel Optimal
    Write-Host "[windows-app] packed $zip"
}

if ($Install) {
    $installRoot = Join-Path ([Environment]::GetFolderPath("LocalApplicationData")) "Ducktape\app"
    $installParent = Split-Path -Parent $installRoot
    New-Item -ItemType Directory -Path $installParent -Force | Out-Null
    if (Test-Path -LiteralPath $installRoot) {
        Remove-Item -LiteralPath $installRoot -Recurse -Force
    }
    Copy-Item -LiteralPath $stage -Destination $installRoot -Recurse

    $programs = [Environment]::GetFolderPath("Programs")
    $shortcut = Join-Path $programs "Ducktape.lnk"
    $shell = New-Object -ComObject WScript.Shell
    $link = $shell.CreateShortcut($shortcut)
    $link.TargetPath = Join-Path $installRoot "Ducktape.exe"
    $link.WorkingDirectory = $installRoot
    $link.IconLocation = Join-Path $installRoot "Ducktape.ico"
    $link.Description = "Ducktape"
    $link.Save()
    Write-Host "[windows-app] installed $installRoot"
    Write-Host "[windows-app] installed $shortcut"
}
