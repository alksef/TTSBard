# CodeGraphContext wrapper for the project-local Python environment.
#
# Usage:
#   .\scripts\codegraph.ps1 doctor
#   .\scripts\codegraph.ps1 stats
#   .\scripts\codegraph.ps1 find name speech_queue
#   .\scripts\codegraph.ps1 analyze callers some_function
#
# All arguments are forwarded to cgc. KuzuDB is selected explicitly because
# FalkorDB Lite requires Python 3.12+, while this project's .venv uses 3.11.

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$cgc = Join-Path $repoRoot '.venv\Scripts\cgc.exe'

if (-not (Test-Path -LiteralPath $cgc -PathType Leaf)) {
    throw "CodeGraphContext is not installed in $repoRoot\.venv. Run: .\.venv\Scripts\python.exe -m pip install codegraphcontext"
}

# Rich/Click output contains Unicode symbols that fail under the default
# Windows cp1251 console used by non-interactive MCP clients.
$env:PYTHONUTF8 = '1'
$env:PYTHONIOENCODING = 'utf-8'
[Console]::InputEncoding = [Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)

Push-Location $repoRoot
try {
    & $cgc --database kuzudb @args
    exit $LASTEXITCODE
}
finally {
    Pop-Location
}
