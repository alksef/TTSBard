---
id: ROADMAP-112
status: completed
created: 2026-09-29
updated: 2026-09-29
related_tasks: []
---

# ROADMAP-112 — Ограниченная очередь речи и неблокирующая запись истории

## Контекст

Полное ревью на `c873ff9` обнаружило два независимых риска в speech pipeline:
`SpeechQueue` сохраняет все завершённые задания вместе с текстом и snapshot, а
async worker синхронно записывает историю на диск. Локальное evidence —
`.work/ai/full-review/2026-09-29-full-code-review/reviews/review-001-2026-09-29.md`.

## Цель

Длительная сессия имеет ограниченный объём terminal jobs и предсказуемый размер
`speech-queue-changed`; медленный диск не блокирует общий async runtime при записи
истории. Активные задания, replay и restore сохраняют ожидаемое поведение.

## Границы работы

1. Определить политику хранения terminal jobs и правила для ID, используемых
   replay, restore и текущим playback. Выбор лимита и семантики удаления требует
   отдельного архитектурного решения до реализации.
2. В owner `SpeechQueue` ограничить terminal retention и DTO, не меняя active
   capacity как побочный эффект.
3. Изолировать синхронную запись phrase history от async executor в speech
   worker и export path; сохранить порядок публикации истории, обработку ошибок
   и persist-before-publish инвариант.

## Затрагиваемые места

- `src-tauri/src/speech_queue.rs` — очередь, transitions, state DTO и тесты.
- `src-tauri/src/commands/speech_queue.rs`, `src-tauri/src/event_loop.rs` —
  consumers ID и публикация состояния; менять только если политика retention
  требует.
- `src-tauri/src/setup.rs`, `src-tauri/src/commands/tts_pipeline.rs`,
  `src-tauri/src/history.rs` — вызовы записи истории и blocking isolation.
- Frontend queue list/replay consumers — проверить влияние удаления старых ID;
  UI менять лишь при подтверждённом контрактном изменении.

## Критерии результата

- После числа terminal jobs выше выбранного лимита память/DTO остаются bounded;
  active jobs и доступные replay/restore операции не теряются неожиданно.
- Тест с задержанной persistence показывает, что другие задачи Tokio продолжают
  исполняться; ошибка записи сохраняет текущую rollback/diagnostic семантику.
- Проходят focused Rust tests, затем полный `scripts/cargo.ps1 test`, check,
  строгий clippy и релевантные frontend/contract gates.

## Outcome

Политика хранения зафиксирована в [DECISION-023](../../decisions/023-speech-queue-terminal-retention.md):
`SpeechQueue` хранит последние `MAX_TERMINAL_RETENTION = 50` terminal jobs
(`Completed`/`Cancelled`) и вытесняет самый старый terminal job на каждом переходе
в terminal-статус. Non-terminal и `Failed` не вытесняются: `Failed` уже ограничен
`MAX_ACTIVE_CAPACITY`. Размер очереди ограничен 100 заданиями, `state()` и payload
`speech-queue-changed` перестали расти с длиной сессии; DTO и IPC-контракт не
менялись, frontend не правился.

Запись phrase history вынесена с async worker на blocking pool через owned handle
(`EditorService::history_handle()` + `history::write_phrase_blocking`). Оба call
site (`setup.rs` speech worker и `synthesize_and_export`) ожидают запись до
публикации события, поэтому persist-before-publish сохранён; ошибка персистенции
и отказ blocking-задачи сообщаются раздельно (`HistoryWriteError::Persist` /
`::Join`). Rollback-семантика `history.rs` не менялась.

Доказательства: 6 новых тестов retention в `speech_queue.rs` (границы completed и
cancelled, вытеснение самого старого terminal независимо от вида, сохранение
active и `Failed`, `JobNotFound` для вытеснённого ID и restore retained ID);
5 новых тестов в `history.rs` (запись исполняется вне async worker thread,
задержанная persistence не останавливает другие задачи Tokio, раздельные
`Persist`/`Join`, persist-before-publish); 1 тест в `playback.rs`
(`project_evicted_completed_cache_row_stays_replayable`) на путь проекции после
вытеснения.

Проверки: focused `--lib speech_queue` — 165 passed, 1 ignored; focused
`--lib history` — 21 passed из 27 (6 падений среды, см. ниже); полный
`scripts/cargo.ps1 test` — 1955 passed / 58 failed / 6 ignored при baseline
1943 / 58 / 6 (набор падений идентичен, +12 новых тестов); строгий
`clippy --all-targets --all-features -- -D warnings` и `fmt --check` — clean;
`npm test` 1187 passed, `npm run build`, `check/test:ipc` и
`check/test:speech-contract` — зелёные; `scripts/check-docs.ps1` — passed.

Ограничения: все 58 падений полного Rust-прогона — это отказы доступа
(`Access denied`) при atomic replace временных файлов в `%TEMP%` и `%APPDATA%`,
которые в этой среде блокирует файловый sandbox; тот же набор воспроизводится на
baseline без правок. Ручной UI-смоук окна playback (cancel → restore → replay,
длинная сессия) не проводился: живая сессия приложения в этой среде недоступна,
поведение списка подтверждено юнит-тестами DTO-проекции и существующей веткой
cache-записей `project_playback_activity` (видимый список окна строится именно
из `get_playback_activity`, `speechQueue.jobs` используется только для
empty-hint).
