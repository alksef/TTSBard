# cargo.ps1 — convenience wrapper that sets LIBCLANG_PATH before invoking Cargo.
#
# Usage:  .\scripts\cargo.ps1 test --manifest-path src-tauri/Cargo.toml
#         .\scripts\cargo.ps1 check --manifest-path src-tauri/Cargo.toml
#         .\scripts\cargo.ps1 clippy --manifest-path src-tauri/Cargo.toml
#         .\scripts\cargo.ps1 --version
#
# All arguments are forwarded to cargo; exit code is preserved.

$ErrorActionPreference = 'Stop'

. "$PSScriptRoot\libclang-bootstrap.ps1"

$null = Initialize-LibClangPath

& cargo @args
exit $LASTEXITCODE
