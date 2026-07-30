[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$scriptDir = Split-Path $PSCommandPath
$helper = (Resolve-Path (Join-Path $scriptDir '..\libclang-bootstrap.ps1')).Path

$passed = 0
$total  = 0

$fixtureRoot = Join-Path ([IO.Path]::GetTempPath()) "tts-test-libclang-$([Guid]::NewGuid())"
New-Item -ItemType Directory -Path $fixtureRoot -Force | Out-Null

$savedLibClangPath = $env:LIBCLANG_PATH

function Reset-Env {
    Remove-Item Env:LIBCLANG_PATH -ErrorAction SilentlyContinue
}

function New-FixtureDir([string]$Name) {
    $d = Join-Path $fixtureRoot $Name
    New-Item -ItemType Directory -Path $d -Force | Out-Null
    return $d
}

function New-FakeLibClang([string]$Dir) {
    $dll = Join-Path $Dir 'libclang.dll'
    '' | Out-File -FilePath $dll -Encoding ascii
}

function Assert-Equal([string]$Name, $Expected, $Actual) {
    $script:total++
    if ($Expected -eq $Actual) {
        $script:passed++
        Write-Host "PASS: $Name" -ForegroundColor Green
    }
    else {
        throw "FAIL: $Name — expected '$Expected', got '$Actual'"
    }
}

function Assert-Null([string]$Name, $Actual) {
    $script:total++
    if ($null -eq $Actual) {
        $script:passed++
        Write-Host "PASS: $Name" -ForegroundColor Green
    }
    else {
        throw "FAIL: $Name — expected `$null, got '$Actual'"
    }
}

function Assert-NotNull([string]$Name, $Actual) {
    $script:total++
    if ($null -ne $Actual) {
        $script:passed++
        Write-Host "PASS: $Name" -ForegroundColor Green
    }
    else {
        throw "FAIL: $Name — expected non-null value, got `$null"
    }
}

function Assert-Contains([string]$Name, [string]$Haystack, [string]$Needle) {
    $script:total++
    if ($Haystack.Contains($Needle)) {
        $script:passed++
        Write-Host "PASS: $Name" -ForegroundColor Green
    }
    else {
        throw "FAIL: $Name — string did not contain '$Needle'.`nGot: $Haystack"
    }
}

try {
    . $helper

    $candidateDir = New-FixtureDir 'candidates'
    New-FakeLibClang $candidateDir

    # --- Case 1: Valid env LIBCLANG_PATH wins ---
    Write-Host "--- Case 1: Valid env wins ---"
    Reset-Env
    $envDir = New-FixtureDir 'env-valid'
    New-FakeLibClang $envDir
    $env:LIBCLANG_PATH = $envDir
    $result = Find-LibClangPath -CandidatePaths @()
    Assert-Equal 'valid env returns env dir' $envDir $result

    # --- Case 2: Invalid env + auto-discovery succeeds ---
    Write-Host "--- Case 2: Invalid env, fallback to discovery ---"
    Reset-Env
    $env:LIBCLANG_PATH = Join-Path $fixtureRoot 'nonexistent'
    $result = Find-LibClangPath -CandidatePaths @($candidateDir)
    Assert-Equal 'invalid env falls back to discovery' $candidateDir $result

    # --- Case 3: Auto-discovery with no env ---
    Write-Host "--- Case 3: No env, auto-discovery succeeds ---"
    Reset-Env
    $result = Find-LibClangPath -CandidatePaths @($candidateDir)
    Assert-Equal 'auto-discovery finds libclang' $candidateDir $result

    # --- Case 4: Total absence — error diagnostics ---
    Write-Host "--- Case 4: Total absence, error diagnostics ---"
    Reset-Env
    $emptyDir = New-FixtureDir 'empty'
    $result = Find-LibClangPath -CandidatePaths @($emptyDir)
    Assert-Null 'total absence returns null' $result

    $initResult = Initialize-LibClangPath -CandidatePaths @($emptyDir) -FailOnMissing:$false
    Assert-Null 'init total absence returns null' $initResult

    # --- Case 5: Initialize-LibClangPath sets env correctly ---
    Write-Host "--- Case 5: Initialize-LibClangPath sets env ---"
    Reset-Env
    $envDir2 = New-FixtureDir 'env-init'
    New-FakeLibClang $envDir2
    $result = Initialize-LibClangPath -CandidatePaths @($envDir2) -FailOnMissing:$false
    Assert-Equal 'init sets LIBCLANG_PATH env' $envDir2 $env:LIBCLANG_PATH
    Assert-Equal 'init returns directory' $envDir2 $result

    # --- Case 6: Initialize-LibClangPath throws on total absence ---
    Write-Host "--- Case 6: Initialize-LibClangPath throws on total absence ---"
    Reset-Env
    $emptyDir2 = New-FixtureDir 'empty-throw'
    $threw = $false
    try {
        Initialize-LibClangPath -CandidatePaths @($emptyDir2)
    }
    catch {
        $threw = $true
        $errMsg = $_.Exception.Message
        Assert-Contains 'throw mentions libclang.dll' $errMsg 'libclang.dll'
        Assert-Contains 'throw mentions espeak-rs-sys' $errMsg 'espeak-rs-sys'
        Assert-Contains 'throw mentions bindgen' $errMsg 'bindgen'
        Assert-Contains 'throw mentions LIBCLANG_PATH' $errMsg 'LIBCLANG_PATH'
    }
    Assert-Equal 'Initialize-LibClangPath throws' $true $threw

    Write-Host ""
    Write-Host "libclang bootstrap tests passed: $passed/$total" -ForegroundColor Green
}
finally {
    Remove-Item Env:LIBCLANG_PATH -ErrorAction SilentlyContinue
    if ($savedLibClangPath) {
        $env:LIBCLANG_PATH = $savedLibClangPath
    }
    if (Test-Path -LiteralPath $fixtureRoot) {
        Remove-Item -LiteralPath $fixtureRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}

exit 0
