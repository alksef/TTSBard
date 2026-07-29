[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$scriptDir = Split-Path $PSCommandPath
$validator = (Resolve-Path (Join-Path $scriptDir '..\check-docs.ps1')).Path
$testRoot = Join-Path $scriptDir 'test-task-lifecycle'
$passed = 0
$total = 0

function Reset-Fixture {
    if (Test-Path -LiteralPath $testRoot) {
        Remove-Item -LiteralPath $testRoot -Recurse -Force
    }
    New-Item -ItemType Directory -Path $testRoot | Out-Null
}

function Write-Utf8Bom([string]$Path, [string]$Content) {
    [IO.File]::WriteAllText($Path, $Content, [Text.UTF8Encoding]::new($true))
}

function Write-Task([string]$Filename, [string]$Status) {
    $content = "# Test task`r`n`r`n**Статус:** ``$Status```r`n"
    Write-Utf8Bom (Join-Path $testRoot $Filename) $content
}

function Write-Index([string]$Entries) {
    $content = "# Tasks`r`n`r`n## Текущие задачи`r`n`r`n$Entries"
    Write-Utf8Bom (Join-Path $testRoot 'README.md') $content
}

function Assert-Case(
    [string]$Name,
    [bool]$ShouldPass,
    [string]$ExpectedFragment,
    [scriptblock]$Arrange
) {
    $script:total++
    Reset-Fixture
    & $Arrange

    $output = @(& powershell.exe -NoProfile -ExecutionPolicy Bypass -File $validator `
        -TaskLifecycleFixtureRoot $testRoot 2>&1)
    $exitCode = $LASTEXITCODE
    $text = $output -join "`n"
    $ok = if ($ShouldPass) {
        $exitCode -eq 0
    }
    else {
        $exitCode -ne 0 -and $text.Contains($ExpectedFragment)
    }

    if (-not $ok) {
        throw "FAIL: $Name (exit $exitCode): $text"
    }
    $script:passed++
    Write-Host "PASS: $Name"
}

try {
    Assert-Case 'matching file and status' $true '' {
        Write-Task '100-test.md' 'in_progress'
        Write-Index '- [TASK-100](./100-test.md) — `in_progress`, test.'
    }
    Assert-Case 'missing indexed file' $false 'indexed file not found: 200-gone.md' {
        Write-Index '- [TASK-200](./200-gone.md) — `planned`, test.'
    }
    Assert-Case 'unindexed active file' $false 'unindexed task file' {
        Write-Task '300-orphan.md' 'planned'
        Write-Index ''
    }
    Assert-Case 'status mismatch' $false 'status mismatch' {
        Write-Task '400-mismatch.md' 'in_progress'
        Write-Index '- [TASK-400](./400-mismatch.md) — `planned`, test.'
    }
    Assert-Case 'duplicate index entry' $false 'duplicate index entry' {
        Write-Task '500-duplicate.md' 'planned'
        Write-Index "- [TASK-500](./500-duplicate.md) — ``planned``, first.`r`n- [TASK-500](./500-duplicate.md) — ``planned``, second."
    }
    Assert-Case 'parent traversal' $false 'resolves outside docs/tasks' {
        Write-Task '600-traversal.md' 'planned'
        Write-Index '- [TASK-600](../600-traversal.md) — `planned`, test.'
    }
}
finally {
    if (Test-Path -LiteralPath $testRoot) {
        Remove-Item -LiteralPath $testRoot -Recurse -Force
    }
}

Write-Host "Task lifecycle tests passed: $passed/$total"
exit 0
