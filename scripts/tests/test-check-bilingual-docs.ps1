[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$validator = Join-Path $repoRoot 'scripts/check-bilingual-docs.ps1'
$shellExecutable = (Get-Process -Id $PID).Path
$fixtureParent = Join-Path $repoRoot '.work/ai/bilingual-doc-tests'
$fixture = Join-Path $fixtureParent ([guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path (Join-Path $fixture 'docs') -Force | Out-Null
foreach ($locale in @('en', 'ru')) {
    Copy-Item -LiteralPath (Join-Path $repoRoot "docs/$locale") -Destination (Join-Path $fixture 'docs') -Recurse
}
$target = Join-Path $fixture 'docs/en/user/tts.md'
$original = Get-Content -LiteralPath $target -Raw -Encoding UTF8

function Check-Case([string]$Name, [bool]$ShouldPass, [string]$Expected) {
    $result = @(& $shellExecutable -NoProfile -ExecutionPolicy Bypass -File $validator -Root $fixture 2>&1)
    $code = $LASTEXITCODE
    if (($ShouldPass -and $code -ne 0) -or
        (-not $ShouldPass -and ($code -eq 0 -or -not (($result -join "`n").Contains($Expected))))) {
        throw "FAIL: $Name (exit $code): $result"
    }
    Write-Output "PASS: $Name"
}

Check-Case 'matching real pages' $true ''
Move-Item -LiteralPath $target -Destination "$target.saved"
Check-Case 'missing translation' $false 'Missing language pair'
Move-Item -LiteralPath "$target.saved" -Destination $target
Set-Content -LiteralPath $target -Encoding UTF8 -Value 'English translation coming soon.'
Check-Case 'placeholder instead of translation' $false 'Content is too short'
Set-Content -LiteralPath $target -Encoding UTF8 -Value ($original -replace '../../ru/user/tts.md', '../../ru/user/guide.md')
Check-Case 'switch to wrong subject' $false 'Missing language switch'
Set-Content -LiteralPath $target -Encoding UTF8 -Value ($original + "`n## " + [char]0x0422 + " test`n")
Check-Case 'untranslated heading' $false 'Untranslated English heading'
Set-Content -LiteralPath $target -Encoding UTF8 -Value $original
$newPage = Join-Path $fixture 'docs/en/user/additional.md'
Set-Content -LiteralPath $newPage -Encoding UTF8 -Value $original
Check-Case 'new page without a counterpart' $false 'Missing language pair: docs/ru/user/additional.md'

# Expected validator failures leave LASTEXITCODE set; report the suite result to CI.
exit 0
