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

## Постоянное исправление через Cargo profile

В `src-tauri/Cargo.toml` задан узкий profile override:

```toml
[profile.dev.package.espeak-rs-sys]
debug-assertions = false
```

Он отключает `cfg(debug_assertions)` только при компиляции upstream package
`espeak-rs-sys`, включая его build script. Поэтому ошибочная ветка не печатает
`cargo:rustc-link-lib=dylib=msvcrtd`, а assertions приложения, тестов и остальных
dependencies остаются включёнными. Профиль `test` наследует настройки `dev`,
поэтому одинаковое правило действует для `cargo build`, `cargo test` и debug
Tauri build.

Это решение воспроизводится в чистом checkout, не изменяет Cargo registry, не
требует vendored копии eSpeak и сохраняет registry `source`/`checksum` в
`Cargo.lock`.

### Почему глобальный `build-override` не используется

`[profile.dev.build-override] debug-assertions = false` затрагивает все build
scripts и proc macros. Для текущего dependency graph это меняет конфигурацию
Tauri codegen и приводит к несовместимому generated code. Package-scoped
override ограничивает workaround ровно дефектным upstream crate.

### Почему `/NODEFAULTLIB:msvcrtd.lib` неэффективен

Подавление `msvcrtd.lib` через `/NODEFAULTLIB` не решает проблему. Директива
`cargo:rustc-link-lib=dylib=msvcrtd`
в `espeak-rs-sys/build.rs` добавляет DLL-ссылку как *явную зависимость*
(не default library), поэтому `/NODEFAULTLIB` на неё не действует.
Нужно не допустить генерацию самой директивы.

### Release-сборка

Release-сборка (`--release`) не затрагивается override для `profile.dev` и
никогда не имела этой проблемы: `debug_assertions` в release выключены, а
eSpeak собирается с профилем Release + динамическая CRT.

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

## Обслуживание при обновлении `espeak-rs`/`espeak-rs-sys`

1. При изменении версии проверить upstream `build.rs` на блок `msvcrtd`.
2. Если блок удалён разработчиками, удалить package override из `Cargo.toml`.
3. Если условие или package name изменились, не расширять workaround глобально:
   сначала повторно определить минимальный package-scoped profile.
4. После изменения выполнить полную debug-сборку и проверить зависимости через
   `dumpbin /dependents`.

## См. также

[`debug-piper-onnx-runtime.md`](./debug-piper-onnx-runtime.md) — отдельная тема
про локальный кэш ONNX Runtime для debug-сборки Piper (не связана с CRT).
