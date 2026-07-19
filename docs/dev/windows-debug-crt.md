# Смешанная CRT в debug-сборке Windows

## Симптом

При запуске debug-сборки `ttsbard.exe` возникает аварийное завершение с ошибкой
`_CrtIsValidHeapPointer` из debug-реализации UCRT (`ucrtbased.dll`).
Диагностика по PE-импортам и скрипту сборки `espeak-rs-sys` показала, что
линковщику передан запрос на отладочную библиотеку импорта CRT (`msvcrtd.lib`).

## Причина

`espeak-rs-sys v0.2.0` в методе `main` скрипта `build.rs` содержит
безусловный блок для Windows debug:

```rust
if cfg!(all(debug_assertions, windows)) {
    println!("cargo:rustc-link-lib=dylib=msvcrtd");
}
```

Директива `dylib=msvcrtd` — это запрос линковщику на подключение отладочной
библиотеки импорта CRT (`msvcrtd.lib`). Она тянет за собой отладочные DLL
UCRT (`ucrtbased.dll`) и отладочные функции аллокации (`_malloc_dbg`,
`_free_dbg` и т.д.), тогда как весь остальной процесс собран с
release-профилем и использует обычную `ucrtbase.dll`.

Одновременно CMake-сборка `espeak-ng` выполняется с профилем **Release**
(значение по умолчанию `ESPEAK_LIB_PROFILE` или явная установка в
`scripts/build.ps1`). Release-сборка CMake использует динамическую CRT
(MSVC default).

В одном процессе оказываются **несовместимые аллокаторы**: release-код
выделяет память через обычный CRT (`malloc`/`free` из `ucrtbase.dll`),
а отладочный код — через `_malloc_dbg`/`_free_dbg` из `ucrtbased.dll`.
Проблема не в количестве низкоуровневых Windows heaps, а в несовместимом
учёте блоков: debug-аллокатор добавляет метаданные
(заголовки с информацией о файле/строке, защитные байты), которые
release-`free` не умеет интерпретировать. Проверка `_CrtIsValidHeapPointer`
ловит именно это несоответствие и завершает процесс.

## Почему установка VC++ Redist или Debug CRT DLL не помогает

Скачивание отладочных библиотек CRT (Debug CRT DLL) или установка Visual C++
Redistributable не устраняет несовместимость release/debug allocator metadata.
Проблема не в отсутствии DLL, а в смешивании вариантов CRT в одном процессе.
Корректное решение — использовать согласованный вариант CRT для всех
компонентов.

## Локальный патч `espeak-rs-sys` (developer-local, не коммитить)

Каждый Windows-разработчик, выполняющий debug-сборку, должен самостоятельно
подготовить локальный патч. Директория `src-tauri/patches/espeak-rs-sys/`
и конфигурация `[patch.crates-io]` являются **developer-local** и **не должны
коммититься** в репозиторий. Релизная сборка и committed `Cargo.lock`
в чистом checkout продолжают использовать `espeak-rs-sys` с crates.io.

### Пошаговая подготовка (PowerShell)

Все команды ниже выполняются из корня репозитория.

1. **Загрузить и найти исходный код `espeak-rs-sys 0.2.0` в Cargo registry:**

   ```powershell
   cargo fetch --manifest-path .\src-tauri\Cargo.toml
   $cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE '.cargo' }
   $srcDir = Get-ChildItem -Path "$cargoHome\registry\src\*\espeak-rs-sys-0.2.0" -Directory |
       Select-Object -First 1
   if (-not $srcDir) { throw 'espeak-rs-sys-0.2.0 not found in Cargo registry' }
   Write-Host "Source: $($srcDir.FullName)"
   ```

2. **Скопировать исходный код в `src-tauri/patches/espeak-rs-sys`:**

   ```powershell
   $patchDir = Join-Path (Get-Location) 'src-tauri\patches\espeak-rs-sys'
   if (Test-Path $patchDir) {
       throw "Patch directory already exists: $patchDir. Inspect it before replacing."
   }
   Copy-Item -Recurse -Force $srcDir.FullName $patchDir
   ```

3. **Открыть скопированный `build.rs` и удалить ровно пять строк
   msVCRT debug-блока:**

   ```rust
   // Windows debug
   if cfg!(all(debug_assertions, windows)) {
       println!("cargo:rustc-link-lib=dylib=msvcrtd");
   }
   ```

4. **Добавить в локальный `src-tauri/.cargo/config.toml`, сохранив уже
   существующие секции вроде `[env]`:**

   ```toml
   [patch.crates-io]
   espeak-rs-sys = { path = "patches/espeak-rs-sys" }
   ```

   Cargo разрешает этот путь относительно корня `src-tauri`, поэтому здесь
   намеренно нет `../`.

5. **Чтобы локальные файлы нельзя было случайно добавить через `git add .`,
   добавить их в `.git/info/exclude`:**

   ```gitignore
   /src-tauri/.cargo/
   /src-tauri/patches/espeak-rs-sys/
   ```

   `.git/info/exclude` действует только в текущем clone и не коммитится.

6. **Запустить debug-сборку:**

   ```powershell
   .\scripts\build.ps1 -Mode debug
   ```

   Cargo локально уберёт registry `source`/`checksum` у `espeak-rs-sys` в
   `Cargo.lock`. **Не коммитьте** это изменение — committed lockfile должен
   оставаться привязанным к crates.io. Не запускайте ради этого
   `cargo generate-lockfile`: он может без необходимости перерезолвить другие
   зависимости.

### Почему `/NODEFAULTLIB:msvcrtd.lib` был неэффективен

Предыдущий подход в `src-tauri/build.rs` пытался подавить `msvcrtd.lib`
через `/NODEFAULTLIB`. Однако директива `cargo:rustc-link-lib=dylib=msvcrtd`
в `espeak-rs-sys/build.rs` добавляет DLL-ссылку как *явную зависимость*
(не default library), поэтому `/NODEFAULTLIB` на неё не действует.
Правильное решение — удалить саму директиву в источнике.

### Release-сборка

Release-сборка (`--release`) не затрагивается локальным патчем и никогда не
имела этой проблемы: условный блок `debug_assertions` не активируется в
release, и eSpeak всегда собирается с профилем Release + динамическая CRT.
Committed `Cargo.toml` и `Cargo.lock` не содержат патча, поэтому CI и чистые
checkout используют crates.io-версию. На машине разработчика локальная
конфигурация Cargo применяется к обоим профилям, но единственное отличие
`build.rs` находится под `debug_assertions` и в release не выполняется.

## Сборка debug

### Предварительные требования

- **node** и **npm** (фронтенд Tauri/Vite)
- **Rust toolchain MSVC** (stable, `x86_64-pc-windows-msvc`)
- **CMake** (для сборки eSpeak)
- **LLVM/libclang** (для bindgen, генерирующего Rust-биндинги eSpeak)

### Каноническая команда

```powershell
.\scripts\build.ps1 -Mode debug
```

Скрипт автоматически проверяет toolchain, LLVM/libclang, готовит
espeak-ng-data и выполняет `tauri build --debug --no-bundle`. Артефакт:
`src-tauri\target\debug\ttsbard.exe`.

## Проверка отсутствия Debug CRT

После debug-сборки убедиться, что к исполняемому файлу не прилинкованы
отладочные CRT DLL:

```powershell
dumpbin /dependents src-tauri\target\debug\ttsbard.exe
dumpbin /dependents src-tauri\target\debug\ttsbard_lib.dll
```

### Запрещённые импорты

В выводе `dumpbin /dependents` или `dumpbin /imports` **не должно быть**
ни одной из следующих DLL или символов:

| Запрещённая DLL        | Запрещённые символы     |
|------------------------|-------------------------|
| `ucrtbased.dll`        | `_malloc_dbg`           |
| `vcruntime*d.dll`      | `_free_dbg`             |
| `msvcp*d.dll`          |                         |
| `msvcrtd.dll`          |                         |

В debug-сборке после патча должны присутствовать только обычные (не отладочные)
CRT DLL: `ucrtbase.dll`, `vcruntime140.dll` (или аналогичные без суффикса `d`).

## Дымовой тест (3–5 секунд)

```powershell
$proc = Start-Process -FilePath "src-tauri\target\debug\ttsbard.exe" -PassThru
Start-Sleep -Seconds 4
if (-not $proc.HasExited) {
    Stop-Process -Id $proc.Id -Force
    Write-Host "PASS: process did not crash within 4 seconds"
} else {
    Write-Host "FAIL: process exited with code $($proc.ExitCode)"
}
```

Тест не оставляет фонового процесса: через 4 секунды процесс принудительно
завершается.

## Обслуживание патча при обновлении `espeak-rs`/`espeak-rs-sys`

1. При изменении версии `espeak-rs-sys` в `Cargo.toml` проверить upstream
   крейт на наличие блока `msvcrtd`. Если в новой версии блок удалён
   разработчиками — локальный патч можно убрать.
2. Если блок всё ещё присутствует — обновить vendored копию в
   `src-tauri/patches/espeak-rs-sys` из нового источника Cargo registry и
   повторно применить удаление блока.
3. После любого изменения выполнить полную debug-сборку и проверить
   зависимости через `dumpbin /dependents`.

## См. также

[`debug-piper-onnx-runtime.md`](./debug-piper-onnx-runtime.md) — отдельная тема
про локальный кэш ONNX Runtime для debug-сборки Piper (не связана с CRT).
