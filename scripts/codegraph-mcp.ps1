# Start the CodeGraphContext stdio MCP server for Cline and other MCP clients.
# Do not print anything here: stdout is reserved for the MCP protocol.

$ErrorActionPreference = 'Stop'

$wrapper = Join-Path $PSScriptRoot 'codegraph.ps1'

& $wrapper mcp start
exit $LASTEXITCODE
