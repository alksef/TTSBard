---
id: ROADMAP-069
status: completed
created: 2026-08-13
updated: 2026-08-13
related_tasks: []
---

# ROADMAP-069 — Устранение дефектов полного review 0.21.0

## Контекст

Полное review кода и архитектуры на baseline `6a8040c` (`v0.21.0`) выявило
несколько независимых классов риска: потерю и откат пользовательских данных,
расхождение runtime state с persisted state, обход shutdown lifecycle,
блокирующую работу на async executor, медленную работу внутри low-level hook и
stale frontend results.

Локальный исходный отчёт хранится в `.work/ai/` и не является implementation
prompt. Этот roadmap фиксирует долговечный продуктовый результат, а связанные
TASK-120–TASK-129 задают отдельные границы реализации и review.

Известный дефект фактического runtime-состояния WebView уже принадлежит
[ROADMAP-063](./063-webview-runtime-server-status.md). Он не дублируется здесь:
TASK-128 является исполнимым срезом того roadmap, а завершение ROADMAP-069
требует закрытия его блокирующего runtime-status контракта.

## Цель

Устранить все доказанные findings review так, чтобы операции с пользовательскими
данными были атомарными и наблюдаемыми, async/lifecycle boundaries — явными, а
frontend никогда не публиковал поздний результат как актуальное состояние.

## Инварианты

1. Успешный IPC persistence означает, что durable state действительно записан.
2. Ошибка записи не удаляет предыдущий корректный файл и не публикует candidate
   как сохранённый runtime state.
3. Поздний async response не меняет другую вкладку, новую редакцию текста или
   состояние более свежего запроса.
4. Все пользовательские способы выхода проходят один идемпотентный shutdown
   coordinator.
5. Файловые, CPU-heavy и native audio операции не выполняются на общем async
   executor без явной blocking isolation.
6. Low-level keyboard hook ограничен быстрым capture/enqueue и не выполняет UI,
   playback или файловую работу.
7. Desired configuration и фактический runtime status внешнего сервиса не
   подменяют друг друга.
8. Исправления сохраняют существующие IPC names и persisted schema, если
   отдельная task явно не вводит совместимую typed замену.

## Этапы и задачи

### P0 — Data durability

1. TASK-120 —
   единый Windows-safe replace primitive, propagation ошибок tabs/history и
   переносимые contract-fixture tests.
2. TASK-124 —
   transactional update/remove SoundPanel binding без раннего удаления audio.
3. TASK-127 — одна атомарная
   операция сохранения Fish Audio settings и честный UI in-flight state.

### P1 — Ordering и защита пользовательского ввода

1. TASK-121 —
   conflict-safe AI actions и сериализованное/coalesced сохранение вкладок.
2. TASK-126 —
   generation-aware history/search/speech queue и keyboard-accessible model list.

### P2 — Runtime lifecycle и latency boundaries

1. TASK-122 — единый shutdown
   coordinator для sidebar, tray и OS exit.
2. TASK-123 — blocking
   isolation cache/decode/effects/cache-write этапов speech pipeline.
3. TASK-125 — bounded
   dispatch действий Intercept за пределами low-level hook callback.

### P3 — Runtime truth и документация

1. TASK-128 — исполнимый backend/UI
   срез ROADMAP-063 с readiness, supervisor ownership и truthful UI.
2. TASK-129 — после
   реализации синхронизировать ownership/lifecycle/CORS документацию и закрыть
   review findings без переноса временного отчёта в tracked docs.
3. После независимого review всех задач обновить architecture/decisions только
   по фактической реализации и перенести roadmap в `completed/` с Outcome.

## Порядок выполнения

- TASK-120 выполняется первой: TASK-121 использует её Result/persistence contract,
  а TASK-124 и TASK-127 должны переиспользовать общий atomic primitive там, где
  формат хранения совместим.
- TASK-122 выполняется до TASK-123 и TASK-128 либо координируется с ними через
  заранее определённый shutdown API; параллельные задачи не должны создавать
  собственные exit paths.
- TASK-124 предшествует TASK-125, чтобы storage и hook latency не смешивались в
  одном diff.
- TASK-129 выполняется последней после принятых TASK-120–TASK-128; она не должна
  заранее описывать ещё не реализованную архитектуру.
- TASK-121, TASK-123, TASK-126 и TASK-127 могут выполняться параллельно после
  стабилизации их зависимостей.
- Каждая task проходит отдельный цикл task → DeepSeek/OpenCode → независимый
  task-scoped review. Галочки исполнителя не являются подтверждением готовности.

## Критерии завершения

- crash/failure до replace не уничтожает предыдущий settings/history/tabs файл;
- tabs/history IPC возвращает ошибку при persistence failure и не расходится с
  runtime state;
- AI actions не перезаписывают другую вкладку или новую редакцию;
- два завершившихся в обратном порядке save/request не публикуют старый state;
- update/remove SoundPanel binding сохраняет старый audio при неуспешном commit;
- Fish settings сохраняются целиком либо не меняются;
- tray/sidebar/OS exit используют один идемпотентный shutdown lifecycle;
- speech worker остаётся responsive во время cache/DSP работы, shutdown имеет
  определённую cancellation policy;
- low-level hook callback не вызывает Tauri/window/playback handlers;
- WebView UI показывает подтверждённый runtime status, а не только `enabled`;
- Rust contract fixtures проходят на Windows независимо от CRLF/LF;
- проходят `npm test`, `npm run build`, contract/parity checks,
  `./scripts/cargo.ps1 check`, `test`, `clippy` и `./scripts/check-docs.ps1`;
- ручная приёмка покрывает exit under load, persistence failure, SoundPanel
  replacement, AI tab switch, reversed async completion и WebView port-in-use.

## Не входит

- смена JSON storage на SQLite;
- редизайн speech queue или аудиоалгоритмов;
- новый frontend state-management framework;
- изменение протоколов внешних интеграций;
- общий рефакторинг AppState из TASK-117;
- автоматический retry внешних сервисов без явно зафиксированной policy.

## Связанные материалы

- [ROADMAP-063 — runtime-состояние WebView](./063-webview-runtime-server-status.md)
- [ROADMAP-068 — editor-scoped hotkeys и submit flow](../completed/068-editor-scoped-hotkeys-and-submit-flow.md)
- [DECISION-008 — JSON persistence](../../decisions/008-json-persistence.md)
- [DECISION-011 — lifecycle интеграций](../../decisions/011-integration-lifecycle.md)
- [DECISION-014 — ошибки и validation](../../decisions/014-errors-and-validation.md)

## Outcome

Завершены Windows-safe atomic persistence, stale-result guards редактора и UI,
единый shutdown coordinator, blocking isolation speech pipeline, transactional
SoundPanel/Fish settings и truthful WebView runtime status. Проверки включили
Rust focused suites, `cargo check`/`clippy`, frontend tests/build, IPC/settings
parity и documentation validation. Временные implementation tasks удалены из
`docs/tasks`; долговечные контракты отражены в architecture и WebView docs.
