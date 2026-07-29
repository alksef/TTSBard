---
id: ROADMAP-056
status: in_progress
created: 2026-07-29
updated: 2026-07-29
related_tasks: [TASK-117]
---

# ROADMAP-056 — IPC-контракты и AI-ready границы

## Контекст

ROADMAP-055 закрыл выявленные дефекты безопасности, конкурентности, async
lifecycle, privacy logging и воспроизводимости Rust quality gates. Следующий
этап не является продолжением defect sweep: он должен уменьшить стоимость и
риск последующих изменений на границах Vue, Tauri commands, внутренних событий,
settings и крупных backend-модулей.

Основной путь озвучивания уже использует `submit_speech`, immutable
`SpeechSnapshot` и `SpeechQueue`. Предлагавшийся дополнительный
`SpeechApplicationService` не нужен: у legacy `AppEvent::TextReady` нет активного
producer, а IPC-команда `speak_text` не вызывается frontend. Текущий
`InterceptPanel` перехватывает отдельные клавиши для действий и не является
глобальным перехватом текста.

## Утверждённый scope

1. Ввести явный проверяемый IPC contract layer для command names, event names,
   DTO и структурированных ошибок.
2. Разделить внутренние application/domain events и сериализуемые frontend
   notifications.
3. Улучшить settings consistency только там, где один сценарий меняет
   persistence и runtime state. Общий transaction manager, revisions и большая
   frontend migration не требуются.
4. Разделять крупные backend-модули по ответственности и причинам изменения,
   сохраняя поведение и небольшие task-scoped diffs.
5. Удалить подтверждённый legacy-код текстового перехвата вместо создания нового
   speech orchestration layer.

## Целевой поток данных

```text
Vue feature
  → typed command contract
  → thin Tauri command adapter
  → application/domain API
  → persistence или runtime owner
  → internal event
  → frontend event adapter
  → typed frontend notification
```

Для основного TTS flow остаётся действующей текущая модель:

```text
submit_speech → SpeechSnapshot → SpeechQueue → speech_worker → playback/routing
```

Export сохраняет отдельную output policy без playback. Он может переиспользовать
prepare/synthesize helpers, но не должен искусственно проходить через очередь,
если ему не нужны queue ordering, cancellation и activity state.

## Инварианты

1. Literal `invoke`/`listen` без зарегистрированного backend contract ломает
   автоматическую проверку.
2. Frontend не принимает решения по свободному тексту Rust error; ветвление
   выполняется по стабильному error code и признаку `retryable`.
3. Внутренний event не сериализуется во frontend неявно. Публичный event имеет
   отдельное имя и DTO на contract boundary.
4. Settings-команда либо успешно выполняет `validate → persist → runtime → emit`,
   либо возвращает ошибку без молчаливого частичного результата.
5. Command adapter не содержит domain workflow, а публичный API модуля не
   раскрывает внутренние mutex и mutable state.
6. Каждый этап мигрирует один seam и сохраняет совместимость до перевода всех
   его consumers.

## Этапы реализации

### P0 — Инвентаризация контрактов и legacy cleanup — завершён 2026-07-29

1. Собрать machine-readable inventory зарегистрированных Tauri commands,
   frontend `invoke`, backend/frontend event names и literal `listen`.
2. Добавить parity gate, который сообщает отсутствующий command/event и место
   consumer; динамические имена оформить явным allowlist с причиной.
3. Подтвердить отсутствующих consumers и удалить legacy `TextReady`,
   `speak_text`, `speak_text_internal`, старый text-interception state/commands и
   устаревший комментарий про F1. Key-action intercept не менять.
4. До удаления каждого public name проверить tests, frontend и возможные
   вспомогательные окна; не смешивать cleanup с изменением speech pipeline.

Результат: source-derived inventory и parity gate добавлены в локальные проверки
и CI; динамический event adapter оформлен allowlist с причиной. Legacy
`TextReady`/`speak_text`, дублирующие interception flags и старые команды удалены,
а действующий key-action intercept сохранён.

### P1 — IPC contract layer

1. Ввести backend/frontend contract modules для command и event names. Начать с
   одного небольшого vertical slice, затем переводить domains по очереди.
2. Добавить базовый `CommandError { code, message, retryable }` и adapters между
   domain errors и IPC. Внутренний diagnostic context оставлять в tracing.
3. Переводить frontend consumers без big-bang: новый typed wrapper, его tests,
   миграция callers, затем удаление прежних string literals.
4. Code generation вводить только если ручные shared constants и parity tests
   перестанут обеспечивать достаточную проверяемость.

Первый vertical slice реализован для `submit_speech`: backend возвращает
`CommandError { code, message, retryable }`, frontend вызывает команду через
typed wrapper и преобразует rejection в `IpcCommandError`. Следующие slices
должны переиспользовать этот envelope и мигрироваться по одному domain.

### P2 — Internal и frontend events

1. Отделить внутреннюю event-шину от публичных Tauri notifications; не требовать
   `Serialize` от domain events только ради frontend.
2. Ввести единый frontend event adapter с именами и DTO из contract layer.
3. Первым seam выбрать speech/playback routing либо settings notifications — тот,
   где можно сохранить wire shape и добавить focused compatibility tests.
4. Добавлять correlation ID только сценариям с конкурентными операциями или
   stale result risk, а не каждому событию формально.

### P3 — Локальная консистентность settings

1. Для WebView заменить последовательность независимых field writes одним
   атомарным section-level persist, чтобы ошибка не оставляла частичный config.
2. Для SoundPanel/Intercept перестать игнорировать ошибки `intercept.json`:
   сохранять до публикации runtime state либо выполнять явный rollback.
3. Проверить остальные domains и менять только подтверждённые multi-owner paths.
   Уже корректный порядок `persist → runtime → emit` не переписывать.
4. Добавить failure-injection tests для write error и проверку отсутствия emit и
   runtime mutation после неуспешного persist.

SoundPanel/Intercept часть завершена 2026-07-29: все три mutation-команды
возвращают ошибку записи, runtime обновляется только после успешного persist,
а `InterceptionChanged` не публикуется при failure. Атомарный WebView section
persist остаётся следующим отдельным P3 slice.

### P4 — Module boundaries

Разделять модули по причинам изменения, а не по целевому числу строк:

- Telegram: transport/auth, request correlation, parsing и download policy;
- VTube Studio: connection/auth, request correlation, typing action и item sync;
- settings: schema/defaults, validation, persistence и DTO mapping;
- commands: parse/validate/dispatch без domain workflow.

Для каждого extraction этапа обязательны один owner API, module-level invariants,
focused tests и отсутствие нового публичного доступа к locks. Extraction не
совмещается с изменением wire DTO, UI redesign или новым пользовательским
поведением.

## Порядок выполнения

1. P0 создаёт карту фактических контрактов и удаляет заведомо мёртвый seam.
2. P1 задаёт contract primitives; после первого принятого vertical slice можно
   параллельно выполнять узкие P3 fixes.
3. P2 использует contract layer и мигрирует события по одному domain.
4. P4 начинается только для модулей, где contract/owner boundary уже понятен;
   Telegram и VTube Studio не декомпозируются одновременно.

## Проверки

```powershell
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --locked --manifest-path src-tauri/Cargo.toml
cargo check --locked --manifest-path src-tauri/Cargo.toml
./scripts/check-docs.ps1
```

На каждом этапе дополнительно обязательны task-scoped diff, focused contract
tests и сохранение существующего wire shape, если его изменение не заявлено как
отдельная migration.

## Критерии завершения

- command/event parity автоматически проверяется в локальной и CI-матрице;
- frontend использует contract names и структурированные ошибки на
  мигрированных boundaries;
- internal events отделены от frontend notifications;
- legacy text-interception path удалён, key-action intercept сохранён;
- WebView и Intercept settings не допускают молчаливый partial commit;
- выбранные крупные модули имеют owner API и focused tests, а commands остаются
  тонкими adapters;
- `docs/development/architecture.md` обновлён после фактической реализации, а не
  заранее.

## Не входит

- новый общий `SpeechApplicationService`;
- обязательный прогон export через `SpeechQueue`;
- глобальный settings transaction manager или frontend revision protocol;
- генерация всех Rust/TypeScript DTO одним big-bang этапом;
- одновременная декомпозиция Telegram, VTube Studio, settings и commands;
- UI redesign и новые TTS/AI providers.
