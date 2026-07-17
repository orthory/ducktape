# Stage CEF's sandbox-owning bootstrap, the iced client DLL, node sidecar, and
# the exact Cargo.lock-pinned CEF runtime. The result is a
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
$clientSource = Join-Path $targetDir "ducktape_iced.dll"
$nodeSource = Join-Path $targetDir "ducktape-node.exe"
$manifestSource = Join-Path $repo "app\src-iced\assets\windows\Ducktape.exe.manifest"
$iconSource = Join-Path $repo "app\src-iced\assets\icons\icon.ico"

$lock = Get-Content -LiteralPath (Join-Path $repo "Cargo.lock") -Raw
$cefLockMatch = [regex]::Match(
    $lock,
    '(?ms)^\[\[package\]\]\r?\nname = "cef"\r?\nversion = "([^"]+)"'
)
if (-not $cefLockMatch.Success) {
    throw "[windows-app] could not resolve the cef package from Cargo.lock"
}
$cefPackageVersion = $cefLockMatch.Groups[1].Value
$cefDistributions = @{
    # Keep this identity in lockstep with cef-dll-sys' generated CEF_VERSION.
    "148.0.0+147.0.10" = "147.0.10+gd58e84d+chromium-147.0.7727.118"
}
$cefDistribution = $cefDistributions[$cefPackageVersion]
if ([string]::IsNullOrWhiteSpace($cefDistribution)) {
    throw "[windows-app] Cargo.lock pins unsupported cef $cefPackageVersion; audit and allowlist its exact distribution"
}
$cefVersion = ($cefPackageVersion -split '\+', 2)[1]

$nativeArchitecture = if (-not [string]::IsNullOrWhiteSpace($env:PROCESSOR_ARCHITEW6432)) {
    $env:PROCESSOR_ARCHITEW6432
} else {
    $env:PROCESSOR_ARCHITECTURE
}
switch ($nativeArchitecture.ToUpperInvariant()) {
    "AMD64" {
        $cefArchitecture = "x86_64"
        $archivePlatform = "windows64"
        $archiveSha1 = "af0bd26423b06c5f3f172c66bfef466f035ea3e1"
    }
    "ARM64" {
        $cefArchitecture = "aarch64"
        $archivePlatform = "windowsarm64"
        $archiveSha1 = "497c116d8347729cb0499abf941b7178fc254023"
    }
    "X86" {
        $cefArchitecture = "x86"
        $archivePlatform = "windows32"
        $archiveSha1 = "0efda7a995f22c90147bf3ed0ad0ab0968c0e721"
    }
    default { throw "[windows-app] unsupported native architecture $nativeArchitecture" }
}

$cefPath = if ([string]::IsNullOrWhiteSpace($env:CEF_PATH)) {
    Join-Path $HOME ".local\share\cef"
} else {
    $env:CEF_PATH
}
$cefSource = Join-Path (Join-Path $cefPath $cefVersion) "cef_windows_$cefArchitecture"
$archiveMetadata = Join-Path $cefSource "archive.json"
if (-not (Test-Path -LiteralPath $archiveMetadata -PathType Leaf)) {
    throw "[windows-app] exact CEF distribution metadata is missing: $archiveMetadata"
}
$archive = Get-Content -LiteralPath $archiveMetadata -Raw | ConvertFrom-Json
$expectedArchive = "cef_binary_${cefDistribution}_${archivePlatform}_minimal.tar.bz2"
if ($archive.type -ne "minimal" -or $archive.name -ne $expectedArchive -or $archive.sha1 -ne $archiveSha1) {
    throw "[windows-app] $cefSource is not the exact Cargo.lock-pinned CEF distribution ($expectedArchive)"
}

$bootstrapSource = Join-Path $cefSource "bootstrap.exe"
$cefRuntimeFiles = @(
    "chrome_elf.dll",
    "d3dcompiler_47.dll",
    "libcef.dll",
    "libEGL.dll",
    "libGLESv2.dll",
    "v8_context_snapshot.bin",
    "vk_swiftshader.dll",
    "vk_swiftshader_icd.json",
    "vulkan-1.dll"
)
if ($cefArchitecture -ne "x86") {
    $cefRuntimeFiles += @("dxil.dll", "dxcompiler.dll")
}
$cefResourceFiles = @(
    "chrome_100_percent.pak",
    "chrome_200_percent.pak",
    "resources.pak",
    "icudtl.dat"
)
$localesSource = Join-Path $cefSource "locales"
$cefLocaleBases = @(
    "af", "am", "ar", "bg", "bn", "ca", "cs", "da", "de", "el",
    "en-GB", "en-US", "es-419", "es", "et", "fa", "fi", "fil", "fr", "gu",
    "he", "hi", "hr", "hu", "id", "it", "ja", "kn", "ko", "lt", "lv",
    "ml", "mr", "ms", "nb", "nl", "pl", "pt-BR", "pt-PT", "ro", "ru",
    "sk", "sl", "sr", "sv", "sw", "ta", "te", "th", "tr", "uk", "ur",
    "vi", "zh-CN", "zh-TW"
)
$cefLocaleFiles = @(
    foreach ($base in $cefLocaleBases) {
        "$base.pak"
        "${base}_FEMININE.pak"
        "${base}_MASCULINE.pak"
        "${base}_NEUTER.pak"
    }
)
$cefLocaleFiles = @($cefLocaleFiles | Sort-Object -Unique)

if ($Install -and $Configuration -ne "release") {
    throw "[windows-app] refusing to install a non-release build"
}

foreach ($required in @($bootstrapSource, $clientSource, $nodeSource, $manifestSource, $iconSource) +
    ($cefRuntimeFiles | ForEach-Object { Join-Path $cefSource $_ }) +
    ($cefResourceFiles | ForEach-Object { Join-Path $cefSource $_ })) {
    if (-not (Test-Path -LiteralPath $required -PathType Leaf)) {
        throw "[windows-app] missing $required; build the iced --lib and node targets first"
    }
    if ((Get-Item -LiteralPath $required).Length -eq 0) {
        throw "[windows-app] refusing empty artifact $required"
    }
}
if (-not (Test-Path -LiteralPath $localesSource -PathType Container) -or
    -not (Test-Path -LiteralPath (Join-Path $localesSource "en-US.pak") -PathType Leaf)) {
    throw "[windows-app] exact CEF locale set is missing from $localesSource"
}
$sourceLocaleDirectories = @(Get-ChildItem -LiteralPath $localesSource -Directory)
$sourceLocales = @(Get-ChildItem -LiteralPath $localesSource -File)
$sourceLocaleFiles = @($sourceLocales | ForEach-Object { $_.Name } | Sort-Object -Unique)
$localeDifferences = @(Compare-Object $cefLocaleFiles $sourceLocaleFiles)
if ($sourceLocaleDirectories.Count -ne 0 -or $localeDifferences.Count -ne 0) {
    throw "[windows-app] CEF locales differ from the exact M147 allowlist: $($localeDifferences | Out-String)"
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
foreach ($runtime in $cefRuntimeFiles + $cefResourceFiles) {
    Copy-Item -LiteralPath (Join-Path $cefSource $runtime) -Destination $stage
}
Copy-Item -LiteralPath $localesSource -Destination $stage -Recurse

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

$appFiles = @(
    "Ducktape.exe",
    "Ducktape.dll",
    "ducktape-node.exe",
    "Ducktape.exe.manifest",
    "Ducktape.ico"
)
$expectedTopLevelFiles = @($appFiles + $cefRuntimeFiles + $cefResourceFiles)
$expectedTopLevelFiles = @($expectedTopLevelFiles | Sort-Object -Unique)
foreach ($required in $expectedTopLevelFiles) {
    if (-not (Test-Path -LiteralPath (Join-Path $stage $required) -PathType Leaf)) {
        throw "[windows-app] required staged file is missing: $required"
    }
}
$actualTopLevelFiles = @(Get-ChildItem -LiteralPath $stage -File | ForEach-Object { $_.Name } | Sort-Object -Unique)
$unexpectedFiles = @(Compare-Object $expectedTopLevelFiles $actualTopLevelFiles)
if ($unexpectedFiles.Count -ne 0) {
    throw "[windows-app] staged top-level files differ from the M147 allowlist: $($unexpectedFiles | Out-String)"
}
$actualTopLevelDirectories = @(Get-ChildItem -LiteralPath $stage -Directory | ForEach-Object { $_.Name })
if ($actualTopLevelDirectories.Count -ne 1 -or $actualTopLevelDirectories[0] -ne "locales") {
    throw "[windows-app] staged top-level directories differ from the allowlist (locales only)"
}

foreach ($pair in @(
    [PSCustomObject]@{ Source = $clientSource; Staged = (Join-Path $stage "Ducktape.dll") },
    [PSCustomObject]@{ Source = $nodeSource; Staged = (Join-Path $stage "ducktape-node.exe") }
)) {
    if ((Get-FileHash -Algorithm SHA256 -LiteralPath $pair.Source).Hash -ne
        (Get-FileHash -Algorithm SHA256 -LiteralPath $pair.Staged).Hash) {
        throw "[windows-app] staged app artifact identity check failed"
    }
}
foreach ($runtime in $cefRuntimeFiles + $cefResourceFiles) {
    $source = Join-Path $cefSource $runtime
    $staged = Join-Path $stage $runtime
    if ((Get-FileHash -Algorithm SHA256 -LiteralPath $source).Hash -ne
        (Get-FileHash -Algorithm SHA256 -LiteralPath $staged).Hash) {
        throw "[windows-app] staged CEF identity check failed: $runtime"
    }
}
$stagedLocalesRoot = Join-Path $stage "locales"
$stagedLocales = @(Get-ChildItem -LiteralPath $stagedLocalesRoot -File -Recurse)
if ($sourceLocales.Count -ne $stagedLocales.Count) {
    throw "[windows-app] staged locale count differs from the exact CEF distribution"
}
foreach ($sourceLocale in $sourceLocales) {
    $relative = $sourceLocale.FullName.Substring($localesSource.Length).TrimStart([char[]]'\/')
    $stagedLocale = Join-Path $stagedLocalesRoot $relative
    if (-not (Test-Path -LiteralPath $stagedLocale -PathType Leaf) -or
        (Get-FileHash -Algorithm SHA256 -LiteralPath $sourceLocale.FullName).Hash -ne
        (Get-FileHash -Algorithm SHA256 -LiteralPath $stagedLocale).Hash) {
        throw "[windows-app] staged CEF locale identity check failed: $relative"
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
$archiveBase = "Ducktape-windows-$nativeArchitecture"
foreach ($staleBase in @($archiveBase, "Ducktape-windows-$env:PROCESSOR_ARCHITECTURE") | Sort-Object -Unique) {
    foreach ($staleZip in @(
        (Join-Path $bundleRoot "$staleBase.zip"),
        (Join-Path $bundleRoot "$staleBase-unsigned.zip")
    )) {
        if (Test-Path -LiteralPath $staleZip) {
            Remove-Item -LiteralPath $staleZip -Force
        }
    }
}
if (-not $NoArchive) {
    $suffix = if ($signed) { "" } else { "-unsigned" }
    $zip = Join-Path $bundleRoot "$archiveBase$suffix.zip"
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
