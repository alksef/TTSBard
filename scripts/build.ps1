# build.ps1 — сборка TTSBard (Tauri) под Windows.
#
# Использование:
#   .\scripts\build.ps1                  # релиз по умолчанию
#   .\scripts\build.ps1 -Mode debug      # debug-сборка (без инсталляторов)
#   .\scripts\build.ps1 -Mode release    # полная релиз-сборка (exe + nsis/msi)
#   .\scripts\build.ps1 -Clean           # очистить target/ и dist/ перед сборкой
#
# Обёртки для двойного клика: build-debug.bat, build-release.bat.
#
# Артефакты:
#   exe:      src-tauri\target\<debug|release>\ttsbard.exe
#   bundles:  src-tauri\target\release\bundle\{nsis,msi}\  (только release)

[CmdletBinding()]
param(
    [ValidateSet('debug', 'release')]
    [string]$Mode = 'release',

    [switch]$Clean
)

$ErrorActionPreference = 'Stop'

# --- Цветной вывод -----------------------------------------------------------
function Write-Step($msg) { Write-Host "==> $msg" -ForegroundColor Cyan }
function Write-Ok($msg)   { Write-Host "    $msg" -ForegroundColor Green }
function Write-WarnLine($msg) { Write-Host "    ! $msg" -ForegroundColor Yellow }
function Write-Err($msg)  { Write-Host "    X $msg" -ForegroundColor Red }

$repoRoot = Split-Path -Parent $PSScriptRoot
Set-Location $repoRoot

$modeLabel = $Mode
if ($Clean) { $modeLabel = "$Mode (+clean)" }
Write-Step "TTSBard build — mode: $modeLabel"
Write-Step "Repo: $repoRoot"

# --- Проверка окружения ------------------------------------------------------
Write-Step "Checking toolchain..."

foreach ($cmd in @('node', 'npm', 'cargo', 'cmake')) {
    if (-not (Get-Command $cmd -ErrorAction SilentlyContinue)) {
        Write-Err "$cmd not found in PATH. Установите требуемый инструмент и повторите сборку."
        exit 1
    }
}
try {
    $nodeVer = (node -v)
    $npmVer  = (npm -v)
    $rustcVer = (rustc --version)
    $cmakeVer = (cmake --version | Select-Object -First 1)
} catch {
    Write-Err "Не удалось определить версии toolchain: $_"
    exit 1
}
Write-Ok "node $nodeVer, npm $npmVer"
Write-Ok $rustcVer
Write-Ok $cmakeVer

# espeak-rs-sys hardcodes Release library paths. Keep its C library on the
# default dynamic CRT; src-tauri/build.rs suppresses its extra msvcrtd input for
# debug builds so all native code shares the same Windows heap.
if ($Mode -eq 'debug') {
    $env:ESPEAK_LIB_PROFILE = 'Release'
    Remove-Item Env:ESPEAK_STATIC_CRT -ErrorAction SilentlyContinue
    Write-Ok 'espeak-ng CMake profile: Release + dynamic CRT (shared heap)'
}

# --- Проверка libclang (нужен для espeak-rs-sys / bindgen) --------------------
Write-Step "Checking libclang for bindgen..."

# 1. Если LIBCLANG_PATH уже задан пользователем — проверить наличие libclang.dll.
# Устаревшее значение не должно мешать автопоиску в D:\LLVM и типовых путях.
$libclangDir = $null
if ($env:LIBCLANG_PATH) {
    $envLibclangPath = Join-Path $env:LIBCLANG_PATH 'libclang.dll'
    if (Test-Path $envLibclangPath -PathType Leaf) {
        $libclangDir = $env:LIBCLANG_PATH
        Write-Ok "LIBCLANG_PATH = $($env:LIBCLANG_PATH) (libclang.dll найден в окружении)"
    } else {
        Write-WarnLine "LIBCLANG_PATH задан ($($env:LIBCLANG_PATH)), но libclang.dll не найден в этом каталоге."
    }
}

if (-not $libclangDir) {
    # 2. Автопоиск libclang.dll в типовых каталогах LLVM
    $candidatePaths = @(
        'D:\LLVM\bin',
        'C:\Program Files\LLVM\bin',
        "$env:ProgramFiles\LLVM\bin",
        "$env:LOCALAPPDATA\Programs\LLVM\bin"
    )

    foreach ($dir in $candidatePaths) {
        $dllPath = Join-Path $dir 'libclang.dll'
        if (Test-Path $dllPath -PathType Leaf) {
            $libclangDir = $dir
            Write-Ok "libclang.dll найден: $dllPath"
            break
        }
    }

    if ($libclangDir) {
        $env:LIBCLANG_PATH = $libclangDir
        Write-Ok "LIBCLANG_PATH установлен в $libclangDir"
    } else {
        Write-Err 'libclang.dll не найден.'
        Write-Err 'Он требуется для сборки espeak-rs-sys через bindgen.'
        Write-Err ''
        Write-Err 'Установите LLVM одним из способов:'
        Write-Err '  1. Скачайте установщик с https://github.com/llvm/llvm-project/releases'
        Write-Err '     и установите LLVM в каталог по умолчанию.'
        Write-Err '  2. Или задайте переменную окружения LIBCLANG_PATH,'
        Write-Err '     указывающую на каталог с libclang.dll.'
        Write-Err '     Например: $env:LIBCLANG_PATH = ''D:\LLVM\bin'''
        exit 1
    }
}

# --- Опциональная очистка ----------------------------------------------------
if ($Clean) {
    Write-Step "Cleaning build artifacts..."
    $targetDir = Join-Path $repoRoot 'src-tauri\target'
    $distDir   = Join-Path $repoRoot 'dist'
    foreach ($d in @($targetDir, $distDir)) {
        if (Test-Path $d) {
            Remove-Item -Recurse -Force $d
            Write-Ok "removed $d"
        }
    }
}

# --- Установка npm-зависимостей (если нужно) ---------------------------------
Write-Step "Checking npm dependencies..."
$nodeModules = Join-Path $repoRoot 'node_modules'
if (-not (Test-Path $nodeModules)) {
    Write-Step "Installing npm dependencies..."
    npm install
    if ($LASTEXITCODE -ne 0) { Write-Err "npm install failed"; exit 1 }
    Write-Ok "npm install done"
} else {
    Write-Ok "node_modules exists, skipping install"
}

# --- Сборка ------------------------------------------------------------------
$buildStart = Get-Date

if ($Mode -eq 'debug') {
    Write-Step "Building (tauri build --debug --no-bundle)..."
    # --debug: бэкенд в debug-профайле, фронтенд-бандл, готовый exe, БЕЗ инсталляторов.
    npm run tauri -- build --debug --no-bundle
} else {
    Write-Step "Building (tauri build, release)..."
    npm run tauri -- build
}

if ($LASTEXITCODE -ne 0) {
    Write-Err "tauri build failed (exit $LASTEXITCODE)"
    exit $LASTEXITCODE
}

$elapsed = (Get-Date) - $buildStart
Write-Ok ("build done in {0:mm\:ss}" -f $elapsed)

# --- Отчёт об артефактах -----------------------------------------------------
Write-Step "Artifacts:"

$targetProfile = if ($Mode -eq 'debug') { 'debug' } else { 'release' }
$exePath = Join-Path $repoRoot "src-tauri\target\$targetProfile\ttsbard.exe"
if (Test-Path $exePath) {
    $sizeMb = [math]::Round((Get-Item $exePath).Length / 1MB, 1)
    Write-Ok "EXE  : $exePath ($sizeMb MB)"
} else {
    Write-WarnLine "EXE not found at expected path: $exePath"
}

if ($Mode -eq 'release') {
    $bundleDir = Join-Path $repoRoot 'src-tauri\target\release\bundle'
    if (Test-Path $bundleDir) {
        $installers = Get-ChildItem -Recurse -Path $bundleDir -Include '*.exe','*.msi' -ErrorAction SilentlyContinue
        if ($installers) {
            foreach ($inst in $installers) {
                $sizeMb = [math]::Round($inst.Length / 1MB, 1)
                Write-Ok ("BUNDLE: {0} ({1} MB)" -f $inst.FullName, $sizeMb)
            }
        } else {
            Write-WarnLine "Bundle dir exists but no .exe/.msi installers found"
        }
    } else {
        Write-WarnLine "No bundle directory (installers) — check tauri.conf.json bundle config"
    }
}

Write-Host ""
Write-Host "BUILD SUCCEEDED" -ForegroundColor Green
