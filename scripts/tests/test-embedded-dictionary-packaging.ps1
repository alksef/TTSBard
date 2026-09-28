[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$scriptDir = Split-Path $PSCommandPath
$realRepoRoot = (Resolve-Path (Join-Path $scriptDir '..\..')).Path

$buildScript = Join-Path $realRepoRoot 'scripts\build.ps1'
$noticesScript = Join-Path $realRepoRoot 'scripts\check-third-party-notices.ps1'

$passed = 0
$total = 0

function Get-FunctionDefinitions([string]$ScriptPath) {
    $tokens = $null
    $errors = $null
    $ast = [System.Management.Automation.Language.Parser]::ParseFile($ScriptPath, [ref]$tokens, [ref]$errors)
    if ($errors -and $errors.Count -gt 0) {
        throw "Failed to parse '$ScriptPath': $($errors[0].Message)"
    }
    $result = @{}
    foreach ($fn in $ast.FindAll({ param($a) $a -is [System.Management.Automation.Language.FunctionDefinitionAst] }, $true)) {
        $result[$fn.Name] = $fn.Extent.Text
    }
    return $result
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

function Assert-True([string]$Name, [bool]$Value) {
    $script:total++
    if ($Value) {
        $script:passed++
        Write-Host "PASS: $Name" -ForegroundColor Green
    }
    else {
        throw "FAIL: $Name — expected true"
    }
}

function Assert-False([string]$Name, [bool]$Value) {
    $script:total++
    if (-not $Value) {
        $script:passed++
        Write-Host "PASS: $Name" -ForegroundColor Green
    }
    else {
        throw "FAIL: $Name — expected false"
    }
}

function Assert-Throws([string]$Name, [scriptblock]$Action) {
    $script:total++
    $threw = $false
    try {
        & $Action
    }
    catch {
        $threw = $true
    }
    if ($threw) {
        $script:passed++
        Write-Host "PASS: $Name" -ForegroundColor Green
    }
    else {
        throw "FAIL: $Name — expected an exception"
    }
}

function Assert-PathExists([string]$Name, [string]$Path, [bool]$ExpectedExists) {
    $script:total++
    $actual = Test-Path -LiteralPath $Path
    if ($actual -eq $ExpectedExists) {
        $script:passed++
        Write-Host "PASS: $Name" -ForegroundColor Green
    }
    else {
        throw "FAIL: $Name — expected existence $ExpectedExists for '$Path'"
    }
}

function New-FixtureFile([string]$Path) {
    $parent = Split-Path -Parent $Path
    if ($parent -and -not (Test-Path -LiteralPath $parent)) {
        New-Item -ItemType Directory -Path $parent -Force | Out-Null
    }
    '' | Out-File -FilePath $Path -Encoding ascii
}

function New-FixtureDir([string]$Path) {
    New-Item -ItemType Directory -Path $Path -Force | Out-Null
}

# --- Load the actual helper functions via PowerShell AST (no full build) -----

$buildFunctions = Get-FunctionDefinitions $buildScript
. ([scriptblock]::Create($buildFunctions['Test-IsAncestorOf']))
. ([scriptblock]::Create($buildFunctions['Remove-StaleExternalDict']))

$noticesFunctions = Get-FunctionDefinitions $noticesScript
. ([scriptblock]::Create($noticesFunctions['Test-CoversDictionary']))
. ([scriptblock]::Create($noticesFunctions['Test-SourceBundlesDict']))

# Remove-StaleExternalDict emits progress via Write-Ok; suppress for test output.
function Write-Ok([string]$msg) { }

$fixtureName = 'test-embedded-dictionary-packaging-' + [Guid]::NewGuid().ToString('N')
$fixtureRoot = Join-Path (Join-Path $realRepoRoot '.work\ai\tests\embedded-dictionary-packaging') $fixtureName
New-FixtureDir $fixtureRoot

try {
    # ======================================================================
    # Remove-StaleExternalDict
    # ======================================================================

    # --- Case 1: removes exactly the two stale files, keeps license/other ---
    Write-Host "--- Case 1: stale files removed, license and other retained ---"
    $case1Target = Join-Path $fixtureRoot 'case-remove\target'
    $case1Res = Join-Path $case1Target 'release\resources'
    $case1Dict = Join-Path $case1Res 'dict'
    New-FixtureFile (Join-Path $case1Dict 'ru.aff')
    New-FixtureFile (Join-Path $case1Dict 'ru.dic')
    New-FixtureFile (Join-Path $case1Dict 'LICENSE.txt')
    New-FixtureFile (Join-Path $case1Res 'other.txt')

    Remove-StaleExternalDict -ProfileResourcesDir $case1Res -BuildTargetDir $case1Target

    Assert-PathExists 'stale ru.aff removed' (Join-Path $case1Dict 'ru.aff') $false
    Assert-PathExists 'stale ru.dic removed' (Join-Path $case1Dict 'ru.dic') $false
    Assert-PathExists 'dict license retained' (Join-Path $case1Dict 'LICENSE.txt') $true
    Assert-PathExists 'unrelated file retained' (Join-Path $case1Res 'other.txt') $true

    # --- Case 2: profile resources dir outside build target is rejected ---
    Write-Host "--- Case 2: out-of-root profile resources dir rejected ---"
    $case2Target = Join-Path $fixtureRoot 'case-oor\target'
    $case2Res = Join-Path $fixtureRoot 'case-oor\outside\resources'
    New-FixtureDir $case2Target
    New-FixtureFile (Join-Path $case2Res 'dict\ru.aff')

    Assert-Throws 'out-of-root profile resources dir throws' {
        Remove-StaleExternalDict -ProfileResourcesDir $case2Res -BuildTargetDir $case2Target
    }
    Assert-PathExists 'out-of-root file not deleted' (Join-Path $case2Res 'dict\ru.aff') $true

    # --- Case 3: reparse point in the path is rejected, file not deleted ---
    Write-Host "--- Case 3: reparse point rejected ---"
    $case3Real = Join-Path $fixtureRoot 'case-reparse\real'
    $case3Link = Join-Path $fixtureRoot 'case-reparse\target'
    New-FixtureFile (Join-Path $case3Real 'release\resources\dict\ru.aff')
    New-Item -ItemType Junction -Path $case3Link -Target $case3Real | Out-Null

    Assert-Throws 'reparse point throws' {
        Remove-StaleExternalDict -ProfileResourcesDir (Join-Path $case3Link 'release\resources') -BuildTargetDir $case3Link
    }
    Assert-PathExists 'file behind reparse point retained' (Join-Path $case3Real 'release\resources\dict\ru.aff') $true

    # ======================================================================
    # Test-SourceBundlesDict (validator against a fixture source tree)
    # ======================================================================

    Write-Host "--- Validator: rejecting/accepting bundle.resources mappings ---"
    $validatorFixture = Join-Path $fixtureRoot 'validator'
    New-FixtureFile (Join-Path $validatorFixture 'src-tauri\resources\dict\ru.aff')
    New-FixtureFile (Join-Path $validatorFixture 'src-tauri\resources\dict\ru.dic')
    New-FixtureFile (Join-Path $validatorFixture 'src-tauri\resources\dict\LICENSE.txt')

    $repoRoot = $validatorFixture

    $cases = @(
        @{ Source = 'resources/dict';                          Expected = $true  },
        @{ Source = 'resources/dict/ru.aff';                   Expected = $true  },
        @{ Source = 'resources/dict/ru.dic';                   Expected = $true  },
        @{ Source = './resources/dict';                        Expected = $true  },
        @{ Source = 'resources/dict/ru.*';                     Expected = $true  },
        @{ Source = 'resources/dict/r*';                       Expected = $true  },
        @{ Source = 'resources';                               Expected = $true  },
        @{ Source = '.';                                       Expected = $true  },
        @{ Source = 'resources/dict/LICENSE.txt';              Expected = $false },
        @{ Source = 'resources/espeak-ng-data';                Expected = $false },
        @{ Source = 'locales';                                 Expected = $false },
        @{ Source = 'vendor/signalsmith-stretch/LICENSE.txt';  Expected = $false },
        @{ Source = 'vendor/signalsmith-linear/LICENSE.txt';   Expected = $false },
        @{ Source = '../THIRD_PARTY_NOTICES.md';               Expected = $false }
    )

    foreach ($case in $cases) {
        $actual = Test-SourceBundlesDict $case.Source
        $verb = if ($case.Expected) { 'reject' } else { 'accept' }
        Assert-Equal "validator $verb '$($case.Source)'" $case.Expected $actual
    }

    Write-Host ""
    Write-Host "embedded dictionary packaging tests passed: $passed/$total" -ForegroundColor Green
}
finally {
    if (Test-Path -LiteralPath $fixtureRoot) {
        Remove-Item -LiteralPath $fixtureRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}

exit 0
