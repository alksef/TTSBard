[CmdletBinding()]
param(
    [string]$TaskLifecycleFixtureRoot,
    [string]$MarkdownAnchorFixtureRoot
)

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
        $content = Remove-FencedCode (Get-Content -LiteralPath $file -Raw -Encoding UTF8)
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

function Remove-FencedCode([string]$Content) {
    $sb = [System.Text.StringBuilder]::new()
    $inFence = $false
    $fenceChar = [string]::Empty
    $fenceLength = 0
    foreach ($line in ($Content -split '\r?\n')) {
        if (-not $inFence) {
            if ($line -match '^\s{0,3}(?<f>`{3,}|~{3,})') {
                $inFence = $true
                $fenceChar = $matches['f'].Substring(0, 1)
                $fenceLength = $matches['f'].Length
                $null = $sb.AppendLine()
                continue
            }
            $null = $sb.AppendLine($line)
        }
        else {
            $closingPattern = '^\s{0,3}' + [regex]::Escape($fenceChar) + '{' + $fenceLength + ',}\s*$'
            if ($line -match $closingPattern) {
                $inFence = $false
                $fenceChar = [string]::Empty
                $null = $sb.AppendLine()
                continue
            }
            $null = $sb.AppendLine()
        }
    }
    $sb.ToString()
}

function ConvertFrom-PercentEncoded([string]$Value) {
    try {
        return [System.Uri]::UnescapeDataString($Value)
    }
    catch {
        return $Value
    }
}

function Get-GitHubSlug([string]$Text) {
    $s = $Text
    $s = [regex]::Replace($s, '!\[([^\]]*)\]\([^)]*\)', '$1')
    $s = [regex]::Replace($s, '\[([^\]]*)\]\([^)]*\)', '$1')
    $s = [regex]::Replace($s, '!\[([^\]]*)\]\[[^\]]*\]', '$1')
    $s = [regex]::Replace($s, '\[([^\]]*)\]\[[^\]]*\]', '$1')
    $s = [regex]::Replace($s, '<[^>]+>', '')
    $s = $s.ToLowerInvariant()
    $s = [regex]::Replace($s, '[^\p{L}\p{N}\p{M}_ -]', '')
    return $s.Replace(' ', '-')
}

function Get-DocumentAnchors([string]$Content) {
    $anchors = @{}
    $clean = Remove-FencedCode $Content

    foreach ($m in [regex]::Matches($clean, '(?i)<[a-z][^>]*?\s(?:id|name)\s*=\s*"([^"]+)"[^>]*>')) {
        $anchors[$m.Groups[1].Value] = $true
    }
    foreach ($m in [regex]::Matches($clean, "(?i)<[a-z][^>]*?\s(?:id|name)\s*=\s*'([^']+)'[^>]*>")) {
        $anchors[$m.Groups[1].Value] = $true
    }

    $counts = @{}
    foreach ($line in ($clean -split '\r?\n')) {
        $headingMatch = [regex]::Match($line, '^\s{0,3}#{1,6}\s+(?<text>.+)$')
        if (-not $headingMatch.Success) {
            continue
        }

        $headingText = $headingMatch.Groups['text'].Value.Trim()
        $headingText = [regex]::Replace($headingText, '#+\s*$', '')
        $headingText = $headingText.Trim()
        if ([string]::IsNullOrEmpty($headingText)) {
            continue
        }

        $slug = Get-GitHubSlug $headingText
        if ([string]::IsNullOrEmpty($slug)) {
            continue
        }

        $baseSlug = $slug
        $suffix = 0
        while ($counts.ContainsKey($slug)) {
            $suffix++
            $slug = "$baseSlug-$suffix"
        }
        $counts[$slug] = $true
        $anchors[$slug] = $true
    }

    return $anchors
}

function Test-MarkdownAnchors([string[]]$Files, [string]$BaseDirectory) {
    $base = [IO.Path]::GetFullPath($BaseDirectory)
    $anchorMaps = @{}
    $mdSet = @{}
    foreach ($file in $Files) {
        $full = [IO.Path]::GetFullPath($file)
        $content = Get-Content -LiteralPath $full -Raw -Encoding UTF8
        $anchorMaps[$full] = Get-DocumentAnchors $content
        $mdSet[$full] = $true
    }

    $linkPattern = '\[[^\]]*\]\((?<target>[^)]+)\)'
    foreach ($file in $Files) {
        $full = [IO.Path]::GetFullPath($file)
        $content = Get-Content -LiteralPath $full -Raw -Encoding UTF8
        $body = Remove-FencedCode $content
        foreach ($match in [regex]::Matches($body, $linkPattern)) {
            $target = $match.Groups['target'].Value.Trim()
            if ($target -match '^(?:https?://|mailto:)') {
                continue
            }

            if ($target.StartsWith('<') -and $target.EndsWith('>')) {
                $target = $target.Substring(1, $target.Length - 2)
            }

            $hashIndex = $target.IndexOf('#')
            if ($hashIndex -lt 0) {
                continue
            }

            $pathPart = $target.Substring(0, $hashIndex)
            $fragment = ConvertFrom-PercentEncoded ($target.Substring($hashIndex + 1))
            if ([string]::IsNullOrWhiteSpace($fragment)) {
                continue
            }

            if ([string]::IsNullOrWhiteSpace($pathPart)) {
                $targetFile = $full
            }
            else {
                $decoded = ConvertFrom-PercentEncoded $pathPart
                $targetFile = [IO.Path]::GetFullPath((Join-Path (Split-Path $full) $decoded))
            }

            if (-not $mdSet.ContainsKey($targetFile)) {
                continue
            }

            if (-not $anchorMaps[$targetFile].ContainsKey($fragment)) {
                $relativeFile = $full.Substring($base.Length + 1)
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

function Test-RoadmapFiles(
    [string]$RelativeDirectory,
    [string[]]$AllowedStatuses
) {
    $directory = Join-Path $repoRoot $RelativeDirectory
    if (-not (Test-Path -LiteralPath $directory)) {
        Add-Error "Missing required directory: $RelativeDirectory"
        return
    }

    $frontMatterPattern = [regex]::new(
        '\A---\r?\n' +
        'id:\s*(?<id>ROADMAP-\d{3})\r?\n' +
        'status:\s*(?<status>[a-z_]+)\r?\n' +
        'created:\s*(?<created>\d{4}-\d{2}-\d{2})\r?\n' +
        'updated:\s*(?<updated>\d{4}-\d{2}-\d{2})\r?\n' +
        'related_tasks:\s*(?<tasks>\[[^\r\n]*\])\r?\n' +
        '---(?:\r?\n|$)'
    )
    $dateFormat = 'yyyy-MM-dd'

    foreach ($file in Get-ChildItem -LiteralPath $directory -Filter '*.md' -File) {
        if ($file.Name -eq 'README.md') {
            continue
        }

        $content = Get-Content -LiteralPath $file.FullName -Raw -Encoding UTF8
        $match = $frontMatterPattern.Match($content)
        $relativePath = "$RelativeDirectory/$($file.Name)"
        if (-not $match.Success) {
            Add-Error "$relativePath`: missing canonical roadmap front matter"
            continue
        }

        $filenameMatch = [regex]::Match($file.Name, '^(?<number>\d{3})-')
        if (-not $filenameMatch.Success) {
            Add-Error "$relativePath`: filename must start with a three-digit roadmap number"
        }
        elseif ($match.Groups['id'].Value -ne "ROADMAP-$($filenameMatch.Groups['number'].Value)") {
            Add-Error "$relativePath`: id does not match filename"
        }

        $status = $match.Groups['status'].Value
        if ($AllowedStatuses -notcontains $status) {
            Add-Error "$relativePath`: invalid status '$status' ($($AllowedStatuses -join ', '))"
        }

        $created = [datetime]::MinValue
        $updated = [datetime]::MinValue
        $createdValid = [datetime]::TryParseExact(
            $match.Groups['created'].Value,
            $dateFormat,
            [Globalization.CultureInfo]::InvariantCulture,
            [Globalization.DateTimeStyles]::None,
            [ref]$created
        )
        $updatedValid = [datetime]::TryParseExact(
            $match.Groups['updated'].Value,
            $dateFormat,
            [Globalization.CultureInfo]::InvariantCulture,
            [Globalization.DateTimeStyles]::None,
            [ref]$updated
        )
        if (-not $createdValid -or -not $updatedValid) {
            Add-Error "$relativePath`: created and updated must be valid ISO dates"
        }
        elseif ($updated -lt $created) {
            Add-Error "$relativePath`: updated date precedes created date"
        }

        $relatedTasks = $match.Groups['tasks'].Value
        if ($relatedTasks -notmatch '^\[(?:\s*TASK-\d{3}(?:\s*,\s*TASK-\d{3})*)?\]$') {
            Add-Error "$relativePath`: related_tasks must be an inline list of TASK-NNN ids"
        }

        if ($status -eq 'superseded') {
            $hasReplacementText = $content -match '(?i)(supersed|замен)'
            $hasMarkdownLink = $content -match '\[[^\]]+\]\([^)]+\)'
            if (-not $hasReplacementText -or -not $hasMarkdownLink) {
                Add-Error "$relativePath`: superseded item must link to its replacement"
            }
        }
        elseif ($status -eq 'completed') {
            $outcome = [regex]::Match(
                $content,
                '(?ms)^## Outcome\s*\r?\n\s*(?<body>.+?)(?=^## |\z)'
            )
            if (-not $outcome.Success -or [string]::IsNullOrWhiteSpace($outcome.Groups['body'].Value)) {
                Add-Error "$relativePath`: completed item must contain a non-empty Outcome section"
            }
        }
        elseif ($status -eq 'rejected' -and $content -notmatch '(?im)^## Reconsider when\s*$') {
            Add-Error "$relativePath`: rejected item must contain a Reconsider when section"
        }
    }
}

function Test-TaskLifecycle([string]$TasksDirectory) {
    $tasksDir = [IO.Path]::GetFullPath($TasksDirectory)
    $readmePath = Join-Path $tasksDir 'README.md'
    if (-not (Test-Path -LiteralPath $readmePath)) {
        Add-Error "Missing docs/tasks/README.md"
        return
    }

    $readmeContent = Get-Content -LiteralPath $readmePath -Raw -Encoding UTF8
    $sectionMatch = [regex]::Match(
        $readmeContent,
        '## Текущие задачи\s*\r?\n(?<body>.*?)(?=\r?\n## |\z)',
        [Text.RegularExpressions.RegexOptions]::Singleline
    )
    if (-not $sectionMatch.Success) {
        Add-Error "docs/tasks/README.md: missing '## Текущие задачи' section"
        return
    }

    $sectionBody = $sectionMatch.Groups['body'].Value

    # Parse indexed entries: link target + backtick-quoted status
    $indexedFiles = @{}
    $indexedStatuses = @{}
    $entryPattern = '(?ms)^\s*-\s+\[[^\]\r\n]+\]\((?<link>[^)\r\n]+)\)(?<tail>.*?)(?=^\s*-\s+\[|\z)'
    foreach ($match in [regex]::Matches($sectionBody, $entryPattern)) {
        $linkTarget = $match.Groups['link'].Value.Trim()
        $statusMatch = [regex]::Match($match.Groups['tail'].Value, '`(?<status>[^`]+)`')
        if (-not $statusMatch.Success) {
            Add-Error "docs/tasks/README.md: index entry '$linkTarget' has no status"
            continue
        }
        $status = $statusMatch.Groups['status'].Value.Trim()

        try {
            $resolved = [IO.Path]::GetFullPath((Join-Path $tasksDir $linkTarget))
        }
        catch {
            Add-Error "docs/tasks/README.md: invalid index link: $linkTarget"
            continue
        }
        if ([IO.Path]::GetDirectoryName($resolved) -ne $tasksDir) {
            Add-Error "docs/tasks/README.md: index link resolves outside docs/tasks: $linkTarget"
            continue
        }
        $filename = [IO.Path]::GetFileName($resolved)
        if ([string]::IsNullOrEmpty($filename) -or $filename -eq 'README.md') {
            Add-Error "docs/tasks/README.md: invalid task link: $linkTarget"
            continue
        }

        if ($indexedFiles.ContainsKey($filename)) {
            Add-Error "docs/tasks/README.md: duplicate index entry for $filename"
            continue
        }

        $indexedFiles[$filename] = $true
        $indexedStatuses[$filename] = $status
    }

    $taskFiles = @{}
    foreach ($file in Get-ChildItem -LiteralPath $tasksDir -Filter '*.md' -File) {
        if ($file.Name -eq 'README.md') { continue }
        $taskFiles[$file.Name] = $file.FullName
    }

    foreach ($file in $taskFiles.Keys) {
        if (-not $indexedFiles.ContainsKey($file)) {
            Add-Error "docs/tasks/$($file): unindexed task file; add to README current-task list"
        }
    }

    foreach ($filename in $indexedFiles.Keys) {
        if (-not $taskFiles.ContainsKey($filename)) {
            Add-Error "docs/tasks/README.md: indexed file not found: $filename"
        }
    }

    $canonicalPattern = [regex]::new("(?m)^\*\*[^*\r\n]+:\*\*\s+\x60(?<status>[^\x60]+)\x60")
    foreach ($filename in $taskFiles.Keys) {
        if (-not $indexedFiles.ContainsKey($filename)) { continue }

        $content = Get-Content -LiteralPath $taskFiles[$filename] -Raw -Encoding UTF8
        $canonMatch = $canonicalPattern.Match($content)
        if (-not $canonMatch.Success) { continue }

        $fileStatus = $canonMatch.Groups['status'].Value
        $indexedStatus = $indexedStatuses[$filename]
        if ($fileStatus -ne $indexedStatus) {
            Add-Error "docs/tasks/$($filename): status mismatch — file has '$fileStatus', README index has '$indexedStatus'"
        }
    }
}

function Test-DocsStructure {
    $allowedEntries = @(
        'README.md',
        'faq.md',
        'product-overview.md',
        'decisions',
        'development',
        'integrations',
        'research',
        'roadmap',
        'tasks',
        'user'
    )
    $paths = & git -C $repoRoot ls-files -- 'docs/*'
    if ($LASTEXITCODE -ne 0) {
        throw 'git ls-files for docs failed'
    }

    foreach ($path in $paths) {
        if (-not (Test-Path -LiteralPath (Join-Path $repoRoot $path))) {
            continue
        }
        $entry = ($path -split '/')[1]
        if ($allowedEntries -notcontains $entry) {
            Add-Error "Unexpected tracked docs root entry: docs/$entry"
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

if (-not [string]::IsNullOrWhiteSpace($TaskLifecycleFixtureRoot)) {
    Test-TaskLifecycle $TaskLifecycleFixtureRoot
    if ($errors.Count -gt 0) {
        foreach ($failure in $errors) {
            Write-Output "ERROR: $failure"
        }
        exit 1
    }
    Write-Output 'Task lifecycle validation passed.'
    exit 0
}

if (-not [string]::IsNullOrWhiteSpace($MarkdownAnchorFixtureRoot)) {
    $fixtureFiles = @(
        Get-ChildItem -LiteralPath $MarkdownAnchorFixtureRoot -Recurse -Filter '*.md' -File |
            Select-Object -ExpandProperty FullName
    )
    Test-MarkdownAnchors $fixtureFiles $MarkdownAnchorFixtureRoot
    if ($errors.Count -gt 0) {
        foreach ($failure in $errors) {
            Write-Output "ERROR: $failure"
        }
        exit 1
    }
    Write-Output 'Markdown anchor validation passed.'
    exit 0
}

$markdownFiles = @(Get-RepoMarkdownFiles)
Test-MarkdownLinks $markdownFiles
Test-MarkdownAnchors $markdownFiles $repoRoot
Test-RoadmapFiles 'docs/roadmap/active' @('exploring', 'planned', 'in_progress', 'deferred')
Test-RoadmapFiles 'docs/roadmap/completed' @('completed', 'superseded')
Test-RoadmapFiles 'docs/roadmap/rejected' @('rejected', 'superseded')
Test-StatusFiles 'docs/tasks' @('planned', 'in_progress', 'deferred', 'blocked')
Test-StatusFiles 'docs/decisions' @('accepted', 'superseded', 'deprecated')
Test-TaskLifecycle (Join-Path $repoRoot 'docs/tasks')
Test-DocsStructure
Test-TrackedArtifacts

if ($errors.Count -gt 0) {
    Write-Host "Documentation validation failed ($($errors.Count)):" -ForegroundColor Red
    foreach ($failure in $errors) {
        Write-Host " - $failure" -ForegroundColor Red
    }
    exit 1
}

Write-Host "Documentation validation passed: $($markdownFiles.Count) Markdown files checked."
