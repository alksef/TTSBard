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
./scripts/cargo.ps1 test --manifest-path src-tauri/Cargo.toml
./scripts/cargo.ps1 check --manifest-path src-tauri/Cargo.toml
./scripts/cargo.ps1 clippy --manifest-path src-tauri/Cargo.toml
```

`./scripts/cargo.ps1` автоматически находит `libclang.dll` и передаёт все аргументы
в `cargo`, сохраняя код возврата. При необходимости discovery можно вызвать
напрямую: `./scripts/libclang-bootstrap.ps1`.

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

Debug-режим по умолчанию создаёт runnable `src-tauri/target/debug/ttsbard.exe`
без инсталляторов, release-режим — приложение и bundles в
`src-tauri/target/release/bundle/`. Это расположение по умолчанию; при
переопределении Cargo target directory (см. «Локальная конфигурация» ниже)
артефакты окажутся в выбранном каталоге. Флаг `-Clean` удаляет build outputs
перед сборкой; применять его следует только при доказанной проблеме кэша.

Для запуска двойным кликом доступны `scripts/build-debug.bat` и
`scripts/build-release.bat`. `scripts/build.ps1` сохранён как UTF-8 с BOM для
совместимости кириллицы с Windows PowerShell 5.1 — при редактировании BOM нужно
сохранить.

### Локальная конфигурация

Разработчик может переопределить Cargo target directory и путь к Rust toolchain
через персональный конфигурационный файл, переменные окружения или параметры
командной строки. Это позволяет разным разработчикам на разных машинах
использовать `build.ps1` без правок.

Приоритет (от высшего к низшему):
1. Явный параметр командной строки (`-CargoTargetDir`, `-RustBinDir`);
2. Переменная окружения (`CARGO_TARGET_DIR`, `TTSBARD_RUST_BIN_DIR`);
3. Файл `scripts/build.local.psd1`;
4. Умолчания (`src-tauri\target`, без переопределения PATH).

Пример конфигурационного файла: `scripts/build.local.example.psd1`. Скопируйте
его в `scripts/build.local.psd1` (игнорируется Git) и отредактируйте:

```powershell
Copy-Item scripts/build.local.example.psd1 scripts/build.local.psd1
# Отредактируйте scripts/build.local.psd1
```

Затем запускайте сборку как обычно:

```powershell
./scripts/build.ps1 -Mode debug
# или с явным параметром:
./scripts/build.ps1 -Mode debug -CargoTargetDir D:\my-target
```

Разрешённые ключи в `.psd1`: `CargoTargetDir` и `RustBinDir` (оба строковые или
`$null`). Ссылки `%NAME%` в путях раскрываются в значения переменных окружения.
Относительные пути разрешаются относительно корня репозитория.

При использовании внешней Cargo target directory (не `src-tauri\target`) перед
первым `-Clean` необходима одна не-clean сборка, которая создаст маркерный файл
`.ttsbard-build-target` внутри target-каталога. Этот маркер защищает от
случайного удаления нецелевой директории. После разрешённой очистки каталог и
маркер создаются заново.

Фактическое расположение артефактов зависит от настроек целевой директории и
выводится скриптом в начале и в конце сборки.

## Документы

- [AI-assisted development workflow](./ai-workflow.md) — постановка локальных
  задач DeepSeek, запуск OpenCode и независимая проверка результата.
- [AI-ready: принципы](./ai-ready-principles.md) — норматив наблюдаемой стоимости
  безопасного малого изменения для агента без памяти о прошлых сессиях.
- [Шаблон code review](./templates/code-review.md) и
  [шаблон AI-ready review](./templates/ai-ready-review.md) — проверка конкретной
  реализации и ручная оценка удобства следующего безопасного изменения.
- [Ручные тест-кейсы](./test-cases.md) и
  [шаблон тест-кейсов](./templates/test-cases.md) — норматив формирования
  ручной проверки функционала, затронутого roadmap-этапом, поверх зелёных
  автотестов.
- [Архитектура](./architecture.md) — устройство приложения и основные
  инженерные паттерны.
- [Сборка и релиз Windows в GitHub Actions](./github-actions-build.md) —
  устройство CI, теги и нативные зависимости.
- [Локальный ONNX Runtime для Piper](./debug-piper-onnx-runtime.md) — подготовка
  debug-сборки.
- [Смешанная CRT в debug-сборке Windows](./windows-debug-crt.md) — диагностика
  и проверка Windows runtime.
