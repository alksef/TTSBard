[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$scriptDir = Split-Path $PSCommandPath
$validator = (Resolve-Path (Join-Path $scriptDir '..\check-docs.ps1')).Path
$fixtureParent = [IO.Path]::GetFullPath((Join-Path $scriptDir '..\..\.work\ai\anchor-tests'))
$testRoot = [IO.Path]::GetFullPath((Join-Path $fixtureParent ([guid]::NewGuid().ToString('N'))))
if ([IO.Path]::GetDirectoryName($testRoot) -ne $fixtureParent) {
    throw 'Fixture path escaped its workspace'
}
$passed = 0
$total = 0

function Reset-Fixture {
    if (Test-Path -LiteralPath $testRoot) {
        Remove-Item -LiteralPath $testRoot -Recurse -Force
    }
    New-Item -ItemType Directory -Path $testRoot -Force | Out-Null
}

function Write-Md([string]$Filename, [string]$Content) {
    [IO.File]::WriteAllText(
        (Join-Path $testRoot $Filename),
        $Content,
        [Text.UTF8Encoding]::new($true)
    )
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
        -MarkdownAnchorFixtureRoot $testRoot 2>&1)
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
    Assert-Case 'colliding generated heading suffix' $true '' {
        Write-Md 'collision.md' "# Foo`n## Foo`n## Foo-1`n[link](#foo-1-1)"
    }
    Assert-Case 'long fence cannot close with shorter fence' $false '#fake' {
        Write-Md 'long-fence.md' @'
# Doc
````
```
## Fake
````
[link](#fake)
'@
    }
    Assert-Case 'plain attribute text is not HTML anchor' $false '#fake' {
        Write-Md 'attribute.md' '# Doc
Plain text id="fake".
[link](#fake)'
    }
    Assert-Case 'spaces retain separate hyphens' $true '' {
        Write-Md 'spaces.md' "# Foo  Bar`n[link](#foo--bar)"
    }
    Assert-Case 'encoded file path' $true '' {
        Write-Md 'with space.md' '# Target'
        Write-Md 'source.md' '[link](./with%20space.md#target)'
    }
    Assert-Case 'valid Cyrillic heading anchor' $true '' {
        Write-Md 'cyrillic.md' @'
# Документ

## Привет мир

[Ссылка](#привет-мир).
'@
    }

    Assert-Case 'duplicate headers get -1 suffix' $true '' {
        Write-Md 'dupes.md' @'
# Doc

## Foo

## Foo

[первый](#foo) и [второй](#foo-1).
'@
    }

    Assert-Case 'same-file anchor' $true '' {
        Write-Md 'same.md' @'
# Doc

## Раздел

[ссылка](#раздел).
'@
    }

    Assert-Case 'percent-encoded fragment' $true '' {
        Write-Md 'encoded.md' @'
# Doc

## Привет мир

[ссылка](#%D0%BF%D1%80%D0%B8%D0%B2%D0%B5%D1%82-%D0%BC%D0%B8%D1%80).
'@
    }

    Assert-Case 'inline formatting in heading' $true '' {
        Write-Md 'format.md' @'
# Doc

## **Жирный** и [ссылка](https://example.com) и `код`

[якорь](#жирный-и-ссылка-и-код).
'@
    }

    Assert-Case 'cross-file anchor' $true '' {
        Write-Md 'target.md' @'
# Цель

## Глава первая
'@
        Write-Md 'source.md' @'
# Исходник

[переход](./target.md#глава-первая).
'@
    }

    Assert-Case 'invalid anchor fails' $false '#missing' {
        Write-Md 'broken.md' @'
# Doc

[битая](#missing).
'@
    }

    Assert-Case 'fenced heading not counted' $false '#fake' {
        Write-Md 'fence-heading.md' @'
# Doc

```
# Fake heading
```

[битая](#fake-heading).
'@
    }

    Assert-Case 'link inside fence ignored' $true '' {
        Write-Md 'fence-link.md' @'
# Doc

```
[not a link](#nope)
```
'@
    }

    Assert-Case 'explicit HTML anchor' $true '' {
        Write-Md 'html.md' @'
# Doc

<a id="custom-anchor"></a>

[ссылка](#custom-anchor).
'@
    }

    Assert-Case 'explicit HTML name anchor' $true '' {
        Write-Md 'html-name.md' @'
# Doc

<a name="named"></a>

[ссылка](#named).
'@
    }

    Assert-Case 'external link ignored' $true '' {
        Write-Md 'external.md' @'
# Doc

[внешняя](https://example.com/#section).
'@
    }

    Assert-Case 'non-markdown fragment target skipped' $true '' {
        Write-Md 'skip-non-md.md' @'
# Doc

[картинка](image.png#whatever).
'@
    }
}
finally {
    if (Test-Path -LiteralPath $testRoot) {
        Remove-Item -LiteralPath $testRoot -Recurse -Force
    }
}

Write-Host "Anchor tests passed: $passed/$total"
exit 0
