[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$errors = [System.Collections.Generic.List[string]]::new()

function Add-Error([string]$Message) {
    $errors.Add($Message)
}

function Get-RepoMarkdownFiles {
    $paths = & git -C $repoRoot ls-files --cached -- '*.md'
    if ($LASTEXITCODE -ne 0) {
        throw 'git ls-files failed'
    }

    $files = @()
    foreach ($path in $paths) {
        $fullPath = Join-Path $repoRoot $path
        if (Test-Path -LiteralPath $fullPath) {
            $files += $fullPath
        }
    }

    $files += Get-ChildItem -LiteralPath (Join-Path $repoRoot 'docs') -Recurse -Filter '*.md' -File |
        Select-Object -ExpandProperty FullName
    $files | Sort-Object -Unique
}

function Test-MarkdownLinks([string[]]$Files) {
    $linkPattern = '\[[^\]]*\]\((?<target>[^)]+)\)'

    foreach ($file in $Files) {
        $content = Get-Content -LiteralPath $file -Raw -Encoding UTF8
        foreach ($match in [regex]::Matches($content, $linkPattern)) {
            $target = $match.Groups['target'].Value.Trim()
            if ($target -match '^(?:https?://|mailto:|#)') {
                continue
            }

            if ($target.StartsWith('<') -and $target.EndsWith('>')) {
                $target = $target.Substring(1, $target.Length - 2)
            }

            $target = ($target -split '#', 2)[0]
            if ([string]::IsNullOrWhiteSpace($target)) {
                continue
            }

            $target = [System.Uri]::UnescapeDataString($target)
            $resolved = Join-Path (Split-Path $file) $target
            if (-not (Test-Path -LiteralPath $resolved)) {
                $relativeFile = $file.Substring($repoRoot.Length + 1)
                Add-Error "$relativeFile -> $target"
            }
        }
    }
}

function Test-StatusFiles(
    [string]$RelativeDirectory,
    [string[]]$AllowedStatuses
) {
    $directory = Join-Path $repoRoot $RelativeDirectory
    if (-not (Test-Path -LiteralPath $directory)) {
        Add-Error "Missing required directory: $RelativeDirectory"
        return
    }

    $pattern = '(?m)^\*\*[^*\r\n]+:\*\*\s+\x60(?<status>[^\x60]+)\x60'

    foreach ($file in Get-ChildItem -LiteralPath $directory -Filter '*.md' -File) {
        if ($file.Name -eq 'README.md') {
            continue
        }

        $content = Get-Content -LiteralPath $file.FullName -Raw -Encoding UTF8
        $match = [regex]::Match($content, $pattern)
        if (-not $match.Success -or $AllowedStatuses -notcontains $match.Groups['status'].Value) {
            Add-Error "$RelativeDirectory/$($file.Name): missing canonical status ($($AllowedStatuses -join ', '))"
        }
    }
}

function Test-TrackedArtifacts {
    $paths = & git -C $repoRoot ls-files
    if ($LASTEXITCODE -ne 0) {
        throw 'git ls-files failed'
    }

    $forbidden = '(?i)(^|/)(?:\.work|docs/(?:bugs|deepseek|stage|plans|reviews|ideas|works|depth-analysis))/|\.(?:log|err)$|(^|/)(?:stderr|stdout)\.txt$'
    foreach ($path in $paths) {
        $fullPath = Join-Path $repoRoot $path
        if (-not (Test-Path -LiteralPath $fullPath)) {
            continue
        }

        if ($path -match $forbidden) {
            Add-Error "Tracked local/scratch artifact: $path"
        }

        if ($path.StartsWith('docs/') -and (Get-Item -LiteralPath $fullPath).Length -eq 0) {
            Add-Error "Empty documentation file: $path"
        }
    }
}

$markdownFiles = @(Get-RepoMarkdownFiles)
Test-MarkdownLinks $markdownFiles
Test-StatusFiles 'docs/roadmap/active' @('exploring', 'planned', 'in_progress', 'deferred')
Test-StatusFiles 'docs/roadmap/completed' @('completed')
Test-StatusFiles 'docs/roadmap/rejected' @('rejected')
Test-StatusFiles 'docs/tasks' @('planned', 'in_progress', 'deferred', 'blocked')
Test-StatusFiles 'docs/decisions' @('accepted', 'superseded', 'deprecated')
Test-TrackedArtifacts

if ($errors.Count -gt 0) {
    Write-Host "Documentation validation failed ($($errors.Count)):" -ForegroundColor Red
    foreach ($failure in $errors) {
        Write-Host " - $failure" -ForegroundColor Red
    }
    exit 1
}

Write-Host "Documentation validation passed: $($markdownFiles.Count) Markdown files checked."
