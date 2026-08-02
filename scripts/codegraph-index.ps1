# Rebuild or update the CodeGraphContext index for this repository.
# Additional arguments are forwarded to `cgc index` (for example `--force`).

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$wrapper = Join-Path $PSScriptRoot 'codegraph.ps1'

& $wrapper index $repoRoot --summarize @args
exit $LASTEXITCODE
