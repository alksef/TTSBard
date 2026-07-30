---
id: ROADMAP-058
status: completed
created: 2026-07-30
updated: 2026-07-30
related_tasks: []
---

# ROADMAP-058 — Исполнимые контракты и lifecycle-проверки

## Контекст

Текущая CI-матрица запускает frontend tests и Rust tests на Windows, а
source-derived IPC gate проверяет имена команд и событий во всех трёх frontend
entrypoints. Остаются три локальных пробела доказательства:

- auxiliary окна регистрируют несколько Tauri listeners последовательными
  `await` и теряют уже полученные unlisten handles при частичной ошибке;
- parity gate не сравнивает сериализуемую форму Rust/TypeScript DTO, event
  payloads и наборы структурированных error codes;
- прямые локальные Cargo-команды на Windows требуют неочевидного
  `LIBCLANG_PATH`, а discovery реализован только внутри полного build script.

Этот roadmap продолжает невыполненные contract/lifecycle части ROADMAP-055 и
ROADMAP-056. Он не меняет пользовательское поведение и не вводит big-bang code
generation.

## Цель

Сделать наиболее рискованные межъязыковые и lifecycle-инварианты исполнимыми:
ошибка частичной регистрации не оставляет listener, wire drift ломает focused
test до runtime, а локальная Rust-проверка использует документированный native
bootstrap.

## Инварианты

1. Каждый успешно зарегистрированный listener освобождается при partial failure
   и unmount, включая auxiliary entrypoints.
2. Contract proof выводится из production declarations, точной сериализации или
   generated artifact, но не из второго вручную поддерживаемого manifest.
3. DTO/error мигрируются по одному vertical slice с сохранением текущего wire
   shape.
4. Native bootstrap одинаково разрешает `libclang` для локальных проверок и
   полного Windows build.
5. Наличие compile-time TypeScript типа без runtime/parity proof не считается
   подтверждением Rust serialization contract.

## Этапы

### P0 — Rollback-safe auxiliary listeners

1. Мигрировать Playback Control на существующий `createAsyncCleanupScope` или
   эквивалентный общий production helper.
2. Мигрировать SoundPanel component и entrypoint, сохраняя все unlisten handles.
3. Добавить focused tests для partial-registration failure, unmount до
   завершения регистрации, обычного unmount и повторного mount.
4. Не менять имена событий, payloads и UI-поведение окон.

### P1 — Reusable Windows native bootstrap

1. Выделить discovery/validation `libclang.dll` из `scripts/build.ps1` в
   переиспользуемый PowerShell helper или wrapper.
2. Использовать один bootstrap в полном build path и документированной команде
   локальных `cargo test/check/clippy`.
3. Проверить сценарии: уже заданный корректный путь, некорректный env,
   auto-discovery и отсутствие dependency с понятной диагностикой.
4. Обновить нормативные quick checks в `docs/development/README.md`; подсказки в
   agent skills не считать единственным source of truth.

### P2 — Первый payload parity slice

1. Зафиксировать точную JSON-форму `AppSettingsDto` через serialization fixture,
   schema/generation либо другой source-derived test без нового unchecked
   manifest.
2. Проверять поля, optional/null semantics и действующие serde rename/default
   правила против frontend contract.
3. Сделать drift локализованным: ошибка должна указывать конкретное поле и
   сторону границы.
4. Сохранить существующий runtime settings refresh flow.

### P3 — Error codes и high-value events

1. Расширить speech contract так, чтобы Rust и TypeScript проверяли один
   исчерпывающий набор error codes, сохраняя `code/message/retryable`.
2. Выбрать один high-value event slice — settings либо playback — и добавить
   payload compatibility proof.
3. Следующие domains мигрировать отдельными task-файлами только после оценки
   полезности первого slice.
4. Не требовать runtime validation для каждого внутреннего Rust event.

## Порядок выполнения

P0 и P1 независимы и реализуются отдельными task-файлами. P2 задаёт выбранный
механизм payload parity. P3 переиспользует его, не создавая альтернативный
contract path.

## Критерии завершения

- auxiliary listener tests доказывают rollback и cleanup при всех заявленных
  failure/unmount сценариях;
- документированные локальные Rust checks не требуют ручного поиска
  `libclang.dll` после выполнения указанного bootstrap;
- изменение Rust или TypeScript формы `AppSettingsDto` в одну сторону ломает
  focused contract test;
- speech error code drift обнаруживается автоматически;
- минимум один публичный event payload имеет executable compatibility proof;
- проходят `npm test`, `npm run build`, `npm run check:ipc`, `npm run test:ipc`,
  релевантные Rust tests и `./scripts/check-docs.ps1`.

## Outcome

- Playback Control и SoundPanel используют rollback-safe scopes: partial
  registration failure, позднее завершение регистрации, обычный unmount и
  повторный mount покрыты focused tests без изменения event names и payloads.
- `scripts/libclang-bootstrap.ps1` стал единым discovery/validation path для
  полного Windows build и локального `scripts/cargo.ps1`; сценарии valid env,
  invalid env, auto-discovery и отсутствующей зависимости проверяются отдельно.
- Точная serde-форма `AppSettingsDto` закреплена двумя Rust-generated fixtures.
  Общий TypeScript compiler-backed checker сравнивает поля, optional/null
  semantics, arrays, records, primitive categories и literal unions; normal
  Rust tests fixtures не переписывают.
- Production-таблица `submit_speech` задаёт исчерпывающий набор error codes и
  retryability. Rust-generated fixture и TypeScript contract проверяют точное
  равенство набора и envelope `code/message/retryable`.
- Публичный `speech-queue-changed` получил executable payload proof для реальных
  Rust `SpeechQueueStateDto` и TypeScript `SpeechQueueStateDto` через тот же
  parity-механизм.
- Проходят frontend suite, build, IPC gates, focused Rust contract tests и
  documentation validation.

## Не входит

- генерация всех DTO и IPC wrappers одним изменением;
- новый frontend state manager;
- изменение публичных event names или UI semantics;
- публикация coverage percentage gate;
- замена Tauri IPC transport;
- общий schema registry, не используемый production/tests.
