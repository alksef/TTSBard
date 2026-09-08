[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$errors = [System.Collections.Generic.List[string]]::new()

function Add-Error([string]$Message) {
    $errors.Add($Message)
}

$expectedRuAff = '38CE7D4AF78E211E9BAFE4BF7E3D6A2C420591136CB738EC6648F8FDF6524CD7'
$expectedRuDic = 'F6047416A0204ADBECF3A451B874EC8A97EE37E2CBC714466EF04D8DBCC0D6FC'

$dictionaryRevision = '32b006a2c22a4ac7e8ed3f03346f7b3d85a970a4'
$piperRsRevision = '346f10f6e8520b2ee21e4d0117c1ca0e38e5cd0f'
$espeakNgRevision = '724808c5a83f9ef95fdd0db886ba7ba537ff224a'

$dictDir = Join-Path $repoRoot 'src-tauri/resources/dict'
$ruAff = Join-Path $dictDir 'ru.aff'
$ruDic = Join-Path $dictDir 'ru.dic'
$dictLicense = Join-Path $dictDir 'LICENSE.txt'
$vendorStretchLicense = Join-Path $repoRoot 'src-tauri/vendor/signalsmith-stretch/LICENSE.txt'
$vendorLinearLicense = Join-Path $repoRoot 'src-tauri/vendor/signalsmith-linear/LICENSE.txt'
$rootLicense = Join-Path $repoRoot 'LICENSE'
$noticesFile = Join-Path $repoRoot 'THIRD_PARTY_NOTICES.md'
$cargoToml = Join-Path $repoRoot 'src-tauri/Cargo.toml'
$cargoLock = Join-Path $repoRoot 'src-tauri/Cargo.lock'
$tauriConf = Join-Path $repoRoot 'src-tauri/tauri.conf.json'

function Get-Relative([string]$Path) {
    if ($Path.StartsWith($repoRoot, [StringComparison]::OrdinalIgnoreCase)) {
        return $Path.Substring($repoRoot.Length + 1)
    }
    return $Path
}

function Test-DictionaryHashes {
    $cases = @(
        @{ Path = $ruAff; Expected = $expectedRuAff; Label = 'src-tauri/resources/dict/ru.aff' },
        @{ Path = $ruDic; Expected = $expectedRuDic; Label = 'src-tauri/resources/dict/ru.dic' }
    )
    foreach ($case in $cases) {
        if (-not (Test-Path -LiteralPath $case.Path -PathType Leaf)) {
            Add-Error "Missing dictionary file: $($case.Label)"
            continue
        }
        $actual = (Get-FileHash -LiteralPath $case.Path -Algorithm SHA256).Hash
        if ($actual -ne $case.Expected) {
            Add-Error "$($case.Label): SHA-256 mismatch — expected $($case.Expected), got $actual"
        }
    }
}

function Test-LicenseFiles {
    $files = @(
        @{ Path = $dictLicense; Label = 'dictionary' },
        @{ Path = $vendorStretchLicense; Label = 'Signalsmith Stretch' },
        @{ Path = $vendorLinearLicense; Label = 'Signalsmith Linear' },
        @{ Path = $rootLicense; Label = 'root GPL' }
    )
    foreach ($file in $files) {
        if (-not (Test-Path -LiteralPath $file.Path -PathType Leaf)) {
            Add-Error "Missing $($file.Label) license file: $(Get-Relative $file.Path)"
        }
    }
}

function Test-DictionaryNotice {
    if (-not (Test-Path -LiteralPath $dictLicense -PathType Leaf)) {
        return
    }
    $content = Get-Content -LiteralPath $dictLicense -Raw -Encoding UTF8
    foreach ($marker in @(
        'Alexander I. Lebedev',
        'Redistribution and use in source and binary forms',
        'POSSIBILITY OF SUCH DAMAGE'
    )) {
        if (-not $content.Contains($marker)) {
            Add-Error "src-tauri/resources/dict/LICENSE.txt is missing required text: $marker"
        }
    }
}

function Test-NoticesMarkers {
    if (-not (Test-Path -LiteralPath $noticesFile -PathType Leaf)) {
        Add-Error 'Missing THIRD_PARTY_NOTICES.md'
        return
    }
    $content = Get-Content -LiteralPath $noticesFile -Raw -Encoding UTF8
    foreach ($marker in @(
        $dictionaryRevision,
        $piperRsRevision,
        $espeakNgRevision,
        'Alexander I. Lebedev',
        'signalsmith-stretch',
        'signalsmith-linear'
    )) {
        if (-not $content.Contains($marker)) {
            Add-Error "THIRD_PARTY_NOTICES.md is missing required marker: $marker"
        }
    }
}

function Get-CargoLockGitRevision([string]$Lock, [string]$PackageName) {
    $escaped = [regex]::Escape($PackageName)
    $pattern = '(?ms)^\[\[package\]\]\s*\r?\nname\s*=\s*"' + $escaped + '"(?<body>.*?)(?=^\[\[package\]\]|\z)'
    $match = [regex]::Match($Lock, $pattern)
    if (-not $match.Success) {
        return $null
    }
    $sourceMatch = [regex]::Match($match.Groups['body'].Value, '(?m)^source\s*=\s*"([^"]+)"')
    if (-not $sourceMatch.Success) {
        return $null
    }
    $revMatch = [regex]::Match($sourceMatch.Groups[1].Value, '\?rev=([0-9a-f]+)#')
    if (-not $revMatch.Success) {
        return $null
    }
    return $revMatch.Groups[1].Value
}

function Test-PiperRevision {
    if (-not (Test-Path -LiteralPath $cargoToml -PathType Leaf)) {
        Add-Error 'Missing src-tauri/Cargo.toml'
    }
    else {
        $toml = Get-Content -LiteralPath $cargoToml -Raw -Encoding UTF8
        $depPattern = '(?m)^espeak-rs\s*=\s*\{[^}]*rev\s*=\s*"(?<rev>[0-9a-f]+)"'
        $depMatch = [regex]::Match($toml, $depPattern)
        if (-not $depMatch.Success) {
            Add-Error 'src-tauri/Cargo.toml: espeak-rs dependency does not pin a git rev'
        }
        elseif ($depMatch.Groups['rev'].Value -ne $piperRsRevision) {
            Add-Error "src-tauri/Cargo.toml: espeak-rs dependency pins rev $($depMatch.Groups['rev'].Value), expected $piperRsRevision"
        }
    }

    if (-not (Test-Path -LiteralPath $cargoLock -PathType Leaf)) {
        Add-Error 'Missing src-tauri/Cargo.lock'
    }
    else {
        $lock = Get-Content -LiteralPath $cargoLock -Raw -Encoding UTF8
        foreach ($package in @('espeak-rs', 'espeak-rs-sys')) {
            $rev = Get-CargoLockGitRevision $lock $package
            if ($null -eq $rev) {
                Add-Error "src-tauri/Cargo.lock: '$package' git source record does not pin a rev"
            }
            elseif ($rev -ne $piperRsRevision) {
                Add-Error "src-tauri/Cargo.lock: '$package' pins rev $rev, expected $piperRsRevision"
            }
        }
    }
}

function Get-ResourceValue([object]$Resources, [string]$Source) {
    $prop = $Resources.PSObject.Properties |
        Where-Object { $_.Name -eq $Source } |
        Select-Object -First 1
    if ($null -eq $prop) {
        return $null
    }
    return $prop.Value
}

function Test-TauriConfig {
    if (-not (Test-Path -LiteralPath $tauriConf -PathType Leaf)) {
        Add-Error 'Missing src-tauri/tauri.conf.json'
        return
    }

    try {
        $config = Get-Content -LiteralPath $tauriConf -Raw -Encoding UTF8 | ConvertFrom-Json
    }
    catch {
        Add-Error "src-tauri/tauri.conf.json is not valid JSON: $($_.Exception.Message)"
        return
    }

    $licenseFile = $config.bundle.licenseFile
    if ($licenseFile -ne '../LICENSE') {
        Add-Error "bundle.licenseFile must remain '../LICENSE', got '$licenseFile'"
    }

    $resources = $config.bundle.resources
    if ($null -eq $resources) {
        Add-Error 'bundle.resources is missing'
        return
    }

    $requiredMappings = @(
        @{ Source = '../THIRD_PARTY_NOTICES.md'; Destination = 'THIRD_PARTY_NOTICES.md' },
        @{ Source = 'vendor/signalsmith-stretch/LICENSE.txt'; Destination = 'third-party/licenses/signalsmith-stretch-LICENSE.txt' },
        @{ Source = 'vendor/signalsmith-linear/LICENSE.txt'; Destination = 'third-party/licenses/signalsmith-linear-LICENSE.txt' }
    )

    foreach ($mapping in $requiredMappings) {
        $actual = Get-ResourceValue $resources $mapping.Source
        if ($null -eq $actual) {
            Add-Error "bundle.resources is missing mapping '$($mapping.Source)' -> '$($mapping.Destination)'"
        }
        elseif ($actual -ne $mapping.Destination) {
            Add-Error "bundle.resources['$($mapping.Source)'] expected '$($mapping.Destination)', got '$actual'"
        }
    }

    $dictDestination = Get-ResourceValue $resources 'resources/dict'
    if ($dictDestination -ne 'resources/dict') {
        Add-Error "bundle.resources['resources/dict'] must map to 'resources/dict' so dict/LICENSE.txt is bundled"
    }
}

Test-DictionaryHashes
Test-LicenseFiles
Test-DictionaryNotice
Test-NoticesMarkers
Test-PiperRevision
Test-TauriConfig

if ($errors.Count -gt 0) {
    Write-Host "Third-party notices validation failed ($($errors.Count)):" -ForegroundColor Red
    foreach ($failure in $errors) {
        Write-Host " - $failure" -ForegroundColor Red
    }
    exit 1
}

Write-Host 'Third-party notices validation passed.'
