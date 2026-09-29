# DECISION-023 — Ограниченное хранение terminal jobs очереди речи

**Статус:** `accepted` (ROADMAP-112)
**Связано:** [ROADMAP-112](../roadmap/completed/112-speech-queue-retention-and-history-io.md)

> **TL;DR.** `SpeechQueue` хранит не более `MAX_TERMINAL_RETENTION = 50` самых
> новых terminal jobs (`Completed`/`Cancelled`) и вытесняет самые старые из них
> на каждом переходе в terminal-статус. Active-задания и `Failed` не вытесняются
> никогда: `Failed` уже ограничен active capacity и требует явного действия
> пользователя. Итоговый размер очереди — не более 100 jobs, поэтому
> `state()` и событие `speech-queue-changed` ограничены.

## Контекст

`speech_queue.rs` добавлял задание в `VecDeque` и никогда ничего не удалял:
`active_count()` исключает `Completed`/`Cancelled`, поэтому лимит
`MAX_ACTIVE_CAPACITY = 50` ограничивал только незавершённые задания. Каждое
завершённое задание сохраняло `original_text` и полный `Snapshot` (настройки,
provider, preprocessor, RUAccent runtime slot), а `state()` сериализовал все
задания в каждый `speech-queue-changed`. В длинной сессии память, обход под
локом и размер события росли линейно (full code review 2026-09-29, MAJOR).

## Решение

- Terminal-статусы — `Completed` и `Cancelled`. Хранятся последние
  `MAX_TERMINAL_RETENTION = 50` terminal jobs в порядке постановки в очередь;
  при переполнении удаляется самый старый terminal job.
- Вытеснение выполняется владельцем очереди сразу после успешного перехода в
  terminal-статус (`mark_completed`, `cancel_job`, `cancel_ready_job`,
  `cancel_generating_job`, `skip_job`). Ошибочные переходы состояние не меняют.
- `Queued`, `Generating`, `Ready`, `Playing` и `Failed` не вытесняются.
  `Failed` считается в `active_count()`, то есть ограничен
  `MAX_ACTIVE_CAPACITY`, и остаётся доступным для retry/skip.
- DTO и IPC-контракт не меняются: вытесняется только присутствие старого
  задания в списке, не поля `JobDto`.

## Последствия

- Размер очереди ограничен `MAX_ACTIVE_CAPACITY + MAX_TERMINAL_RETENTION = 100`;
  `state()` и payload события перестают расти с длиной сессии.
- 50 ≥ `AUDIO_CACHE_SIZE = 20`, поэтому все записи, воспроизводимые из
  playback-кэша, остаются в DTO с `job_id`.
- Вытеснённый ID становится неизвестным для очереди: `has_job`/`get_status`
  возвращают `false`/`None`, поздние события playback для него — no-op
  (существующий гейт `has_job` в `event_loop.rs`), `restore_cancelled_job` —
  `JobNotFound`. UI не показывает действие, которое больше не может выполниться.
- Completed-задание, вытесненное из очереди, но оставшееся в playback-кэше,
  продолжает отображаться в объединённом списке активности через ветку
  cache-записей `project_playback_activity` (`job_id: null`, replay доступен).
- Через 50 terminal jobs становится недоступным restore старого
  user-cancelled задания из UI. Это принятая цена bound: восстановление
  опирается на cached audio (`replay_from_cache`) либо на повторную генерацию,
  и обе операции не обязаны храниться бессрочно.
- Значение лимита — константа `MAX_TERMINAL_RETENTION`. Пересмотр требует нового
  решения, если появится сценарий, которому нужен более длинный список terminal
  jobs (например, отдельная persisted history очереди).

## Условия пересмотра

- Появится требование хранить terminal jobs дольше сессии (persisted очередь или
  отдельный журнал задач) — тогда retention заменяется на persisted history.
- Пользовательский сценарий потребует restore/retry заданий старше 50 terminal
  jobs; тогда лимит повышается или разделяется по статусам.
