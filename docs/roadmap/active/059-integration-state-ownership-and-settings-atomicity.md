---
id: ROADMAP-059
status: planned
created: 2026-07-30
updated: 2026-07-30
related_tasks: [TASK-117]
---

# ROADMAP-059 — Владение integration state и атомарность settings

## Контекст

Архитектурные решения требуют изменять mutable state через API владельца, но
несколько integration boundaries всё ещё допускают прямую работу с locks:

- `TelegramState` объявлен в command adapter, публично раскрывает client и
  изменяется из Telegram, proxy, AI и setup flows;
- Twitch, WebView и VTube Studio services публично раскрывают settings/status;
- сохранение WebView section вызывает несколько независимо пишущих field
  setters, поэтому поздняя ошибка способна оставить частично сохранённый config
  при неизменённом runtime;
- TASK-117 описывает преимущественно поля `AppState` и не охватывает все
  подтверждённые integration seams.

TTS provider selection уже получил отдельный сериализованный owner flow в
review-021 remediation и не входит в этот roadmap.

## Цель

Восстанавливать owner boundaries по одному integration seam: сначала устранить
подтверждённый partial commit WebView, затем выделить Telegram auth/client owner,
после чего закрывать публичные locks существующих сервисов только вместе с
конкретным локальным сценарием и regression tests.

## Инварианты

1. Настройка с общим пользовательским действием применяется как
   `validate → persist → runtime → emit` либо завершается без частичного durable
   и runtime результата.
2. Command adapter валидирует IPC input и делегирует workflow owner API; он не
   владеет auth/client state.
3. Lock guard не удерживается через network request, filesystem I/O или другой
   длительный `await`.
4. Внутренний mutex/RwLock закрывается (`pub` → owner-only) только там, где это
   даёт реальную защиту. Для seam'ов, где Arc по контракту хранят долгоживущие
   движки (например `TelegramState.client` → `SileroTts`/AI, см. DECISION-018),
   фиксируется **допустимый контракт разделения**: Arc остаётся `pub(crate)`, а
   вместо private-mutex вводится обязательное правило доступа
   `lock → clone → drop guard → await` и запрет прямого лок-доступа из command
   adapter. Строгий private mutex требует перерезки контракта hot path и здесь
   не преследуется — см. DECISION-018 (почему).
5. Один task меняет один owner/seam; Telegram, Twitch, WebView и VTube Studio не
   рефакторятся одним diff.

## Этапы

### P0 — Атомарное сохранение WebView section

1. Добавить один `set_webview_settings` section-level persist operation вместо
   четырёх независимых записей для одного save action.
2. Выполнить validation до записи и сохранить согласованными file/shared cache.
3. Применять service runtime state и публиковать success event только после
   успешного persist.
4. Добавить failure-injection tests для validation/write failure и доказать
   отсутствие частичного file/cache/runtime/emit результата.
5. Отдельные команды access token и runtime typing не смешивать с section save
   без необходимости, подтверждённой текущим flow.

### P1 — Telegram owner extraction

1. Уточнить границу `TelegramService`/`TelegramRuntime`, владеющую client,
   auth operation generation и lifecycle state.
2. Перенести auth/restart/cancel и client replacement за методы владельца,
   сохранив действующие Tauri signatures и frontend states.
3. Мигрировать Telegram, proxy, AI и setup callers по одному проверяемому срезу;
   после миграции сделать client state private.
4. Сохранить retry/restart и stale-result invariants из ROADMAP-041.
5. Разделение transport/parsing/download выполнять только там, где оно требуется
   для owner API, а не как общий rewrite Telegram module.

### P2 — Reconciliation TASK-117

1. Обновить TASK-117: перечислить фактические public-state seams и отделить
   AppState decomposition от integration service encapsulation.
2. Удалить завершённые TTS owner пункты либо зафиксировать их как исходный
   контекст, не как будущую работу.
3. Для каждого оставшегося seam определить owner, callers, lock/await risks и
   отдельные критерии приёмки до implementation task.

### P3 — Закрытие public integration locks

1. Выбирать один сервис по наблюдаемому изменению или дефекту: Twitch, WebView
   либо VTube Studio.
2. Сначала добавить минимальный read/mutate owner API и tests, затем мигрировать
   callers и только после этого сделать lock private.
3. Не переносить settings между владельцами механически: persisted source,
   runtime copy и status должны иметь явно описанные роли.
4. После каждого сервиса повторно инвентаризировать прямые lock accesses и не
   объявлять весь этап завершённым по checklist агента.

## Порядок выполнения

P0 — самостоятельный defect fix и выполняется первым. P1 — следующий
архитектурный seam. P2 актуализирует долговечную задачу по фактическому результату
P1. P3 состоит из независимых service-scoped tasks и может завершиться после
закрытия перечисленных public integration locks либо явного rescope с причиной.

## Критерии завершения

- WebView section save имеет один атомарный persist path и failure-injection
  proof для file/cache/runtime/event;
- Telegram command adapters не раскрывают и не изменяют client/auth state
  напрямую;
- Telegram retry, cancel, restart и stale-result tests остаются зелёными;
- TASK-117 соответствует фактическим owner seams и не предлагает уже
  завершённую TTS работу;
- settings/status locks выбранных integration services private, а callers
  используют owner APIs;
- проходят focused tests, полный `cargo test --locked`, `cargo check --locked`,
  строгий Clippy и `./scripts/check-docs.ps1`.

## Не входит

- big-bang rewrite `AppState`, `commands/` или всех integrations;
- изменение Telegram protocol или auth UX;
- новый глобальный settings transaction manager;
- изменение IPC signatures и serialized DTO без отдельной migration;
- TTS provider lifecycle, уже закрытый ROADMAP-041;
- cosmetic file splitting без уменьшения прямого mutable-state access.

