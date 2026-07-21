# Документация разработки

Инструкции по сборке, тестированию, отладке, выпуску версий и процессу
разработки. Этот раздел — нормативный источник инженерного workflow; входные
файлы агентов должны ссылаться сюда, а не дублировать команды.

## Локальная рабочая область

Временные задания AI-агентам, промежуточные ревью, логи и скриншоты хранятся в
`.work/ai/<work-id>/`. Каталог `.work/` игнорируется Git и может отсутствовать в
новом клоне. Перед первой AI-задачей его нужно создать вручную:

```powershell
New-Item -ItemType Directory -Force .work/ai | Out-Null
```

Постоянные планы и решения в `.work/` не хранятся.

## Быстрые проверки

```powershell
npm test
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
```

Проверки выбираются по риску изменения. `npm run build` уже выполняет
`vue-tsc --noEmit`; Rust-тесты по возможности сначала запускаются точечно.

Структура документации проверяется отдельно:

```powershell
./scripts/check-docs.ps1
```

Проверка валидирует локальные Markdown-ссылки, lifecycle-статусы и отсутствие
tracked scratch-артефактов. Она также запускается в GitHub Actions.

## Сборка приложения на Windows

```powershell
./scripts/build.ps1 -Mode debug
./scripts/build.ps1 -Mode release
```

Debug-режим создаёт runnable `src-tauri/target/debug/ttsbard.exe` без
инсталляторов. Release-режим собирает приложение и bundles в
`src-tauri/target/release/bundle/`. Флаг `-Clean` удаляет build outputs перед
сборкой; применять его следует только при доказанной проблеме кэша.

Для запуска двойным кликом доступны `scripts/build-debug.bat` и
`scripts/build-release.bat`. `scripts/build.ps1` сохранён как UTF-8 с BOM для
совместимости кириллицы с Windows PowerShell 5.1 — при редактировании BOM нужно
сохранить.

## Документы

- [AI-assisted development workflow](./ai-workflow.md) — постановка локальных
  задач DeepSeek, запуск OpenCode и независимая проверка результата.
- [Архитектура](./architecture.md) — устройство приложения и основные
  инженерные паттерны.
- [Сборка и релиз Windows в GitHub Actions](./github-actions-build.md) —
  устройство CI, теги и нативные зависимости.
- [Локальный ONNX Runtime для Piper](./debug-piper-onnx-runtime.md) — подготовка
  debug-сборки.
- [Смешанная CRT в debug-сборке Windows](./windows-debug-crt.md) — диагностика
  и проверка Windows runtime.
