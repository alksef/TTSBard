# libclang-bootstrap.ps1 — reusable libclang.dll discovery and validation.
#
# Usage (dot-source):  . "$PSScriptRoot\libclang-bootstrap.ps1"
#                      Initialize-LibClangPath
#
# Usage (direct):      .\scripts\libclang-bootstrap.ps1
#
# Exported functions:
#   Get-DefaultLibClangCandidatePaths   returns default autodiscovery dirs
#   Find-LibClangPath [-CandidatePaths] discovers libclang.dll, returns dir or $null
#   Initialize-LibClangPath [-CandidatePaths] [-FailOnMissing]
#        sets $env:LIBCLANG_PATH for current process; throws if not found
#
# Behavior (matching build.ps1 precedence):
#   1. Valid existing $env:LIBCLANG_PATH wins.
#   2. Invalid env value emits a warning and falls back to discovery.
#   3. Discovery includes D:\LLVM\bin, Program Files LLVM, local app data LLVM.
#   4. Absence fails with actionable diagnostics (unless -FailOnMissing:$false).

function Get-DefaultLibClangCandidatePaths {
    return @(
        'D:\LLVM\bin',
        'C:\Program Files\LLVM\bin',
        "$env:ProgramFiles\LLVM\bin",
        "$env:LOCALAPPDATA\Programs\LLVM\bin"
    )
}

function Find-LibClangPath {
    [CmdletBinding()]
    param(
        [string[]]$CandidatePaths
    )

    if (-not $CandidatePaths) {
        $CandidatePaths = Get-DefaultLibClangCandidatePaths
    }

    if ($env:LIBCLANG_PATH) {
        $dllPath = Join-Path $env:LIBCLANG_PATH 'libclang.dll'
        if (Test-Path $dllPath -PathType Leaf) {
            Write-Host "    LIBCLANG_PATH = $($env:LIBCLANG_PATH) (libclang.dll found in environment)" -ForegroundColor Green
            return $env:LIBCLANG_PATH
        }
        else {
            Write-Host "    ! LIBCLANG_PATH is set ($($env:LIBCLANG_PATH)), but libclang.dll not found in that directory." -ForegroundColor Yellow
        }
    }

    foreach ($dir in $CandidatePaths) {
        $dllPath = Join-Path $dir 'libclang.dll'
        if (Test-Path $dllPath -PathType Leaf) {
            Write-Host "    libclang.dll found: $dllPath" -ForegroundColor Green
            return $dir
        }
    }

    return $null
}

function Initialize-LibClangPath {
    [CmdletBinding()]
    param(
        [string[]]$CandidatePaths,
        [switch]$FailOnMissing = $true
    )

    $libclangDir = Find-LibClangPath -CandidatePaths $CandidatePaths

    if ($libclangDir) {
        $env:LIBCLANG_PATH = $libclangDir
        Write-Host "    LIBCLANG_PATH set to $libclangDir" -ForegroundColor Green
    }
    elseif ($FailOnMissing) {
        Write-Host "    X libclang.dll not found." -ForegroundColor Red
        Write-Host "    X It is required for building espeak-rs-sys via bindgen." -ForegroundColor Red
        Write-Host "    X" -ForegroundColor Red
        Write-Host "    X Install LLVM one of these ways:" -ForegroundColor Red
        Write-Host "    X   1. Download the installer from https://github.com/llvm/llvm-project/releases" -ForegroundColor Red
        Write-Host "    X      and install LLVM to the default directory." -ForegroundColor Red
        Write-Host "    X   2. Or set the LIBCLANG_PATH environment variable" -ForegroundColor Red
        Write-Host "    X      pointing to the directory containing libclang.dll." -ForegroundColor Red
        Write-Host "    X      e.g.: `$env:LIBCLANG_PATH = 'D:\LLVM\bin'" -ForegroundColor Red
        throw "libclang.dll not found. It is required for building espeak-rs-sys via bindgen. Install LLVM (https://github.com/llvm/llvm-project/releases) or set the LIBCLANG_PATH environment variable pointing to the directory containing libclang.dll."
    }

    return $libclangDir
}

if ($MyInvocation.InvocationName -ne '.') {
    Initialize-LibClangPath
}
