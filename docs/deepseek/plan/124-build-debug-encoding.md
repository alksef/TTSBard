# Plan 124: Восстановить запуск debug-сборки Windows

**Дата:** 2026-07-12  
**Проблема:** `scripts/build-debug.bat` падает на разборе `scripts/build.ps1` в Windows PowerShell до запуска Tauri.

## Диагностика

`scripts/build.ps1` содержит UTF-8-байты с кириллицей и длинным тире, но не имеет UTF-8 BOM. При запуске через `powershell -File` (Windows PowerShell 5.1) файл читается как ANSI, из-за чего строковые литералы и комментарии превращаются в mojibake, а парсер сообщает `The string is missing the terminator`.

## Решение

Сохранить существующее содержимое `scripts/build.ps1` без функциональных изменений, добавив UTF-8 BOM (`EF BB BF`) в начало файла. Не менять `.bat`, команды сборки, параметры Tauri или пользовательские незакоммиченные файлы.

## Критерии приемки

1. Первые три байта `scripts/build.ps1` — `EF BB BF`, остальное содержимое не меняется.
2. `cmd /c scripts\\build-debug.bat` проходит PowerShell-разбор и доходит до проверки toolchain/запуска сборки; parser error отсутствует.
3. `cargo check --manifest-path src-tauri/Cargo.toml` и `npx vue-tsc --noEmit` завершаются успешно либо отдельный сбой явно фиксируется как внешний к кодировке скрипта.
4. Изменения ограничены исправлением кодировки `scripts/build.ps1` и workflow-артефактами этой задачи.
