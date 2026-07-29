---
id: ROADMAP-055
status: in_progress
created: 2026-07-29
updated: 2026-07-29
related_tasks: [TASK-117, TASK-118]
---

# ROADMAP-055 — Качество и AI-ready архитектура

## Контекст

Кодовая база быстро расширилась вокруг нескольких независимых runtime-контуров:
редактора, очереди озвучивания, глобального перехвата, playback, настроек и
сетевых интеграций. Основной UI уже создаёт immutable snapshot задачи через
`submit_speech`, но глобальный перехват и export используют отдельный legacy
pipeline. Настройки проходят через общий persisted cache, service-local state и
frontend reload events. IPC-команды, события и DTO при этом описываются вручную
по обе стороны границы Rust/TypeScript.

Такая структура работоспособна, но усложняет локальное доказательство
инвариантов. Изменение одного пользовательского сценария может потребовать
синхронной правки нескольких путей, а агенту или новому разработчику приходится
восстанавливать контракт по строковым именам и косвенным событиям.

## Цель

Повысить наблюдаемое качество приложения и сделать кодовую базу удобной для
безопасных малых изменений человеком или AI-агентом:

- закрыть дефекты безопасности, конкурентности и lifecycle;
- восстановить воспроизводимые quality gates;
- сделать request-local данные явными и не хранить их в глобальном state;
- сократить число неявных Rust/TypeScript контрактов;
- разделить крупные модули по устойчивым responsibility boundaries;
- закрепить тестами критичные переходы состояния и end-to-end data flow.

## Текущий поток данных

### Основной UI

```text
InputPanel
  → submit_speech(text)
  → build_snapshot(settings, provider, routing flags)
  → SpeechQueue
  → speech_worker
  → prepare / synthesize / effects
  → PlaybackManager
  → WebView и Twitch routing из snapshot
  → typed Tauri events в окна
```

Snapshot уже фиксирует provider, voice, routing flags, AI и audio settings в
момент приёма задачи. Это предпочтительный source of truth для конкретной
операции.

### Глобальный перехват и export

```text
keyboard hook → AppEvent::TextReady
  → отдельная async-задача speak_text_internal
  → RoutedText(text, request-local routing flags)
  → synthesis / playback
  → AppEvent::TextSentToTts
  → EventHandler маршрутизирует по flags из события

save dialog → speak_text_raw_export
  → отдельный synthesis path
  → асинхронная запись файла
```

Глобальные временные routing flags устранены, а export больше не блокирует async
executor синхронной записью. Путь всё ещё не наследует queue invariants и общую
cancellation model; его объединение с основным ingestion flow относится к A1 и
возможно только после отдельного утверждения архитектуры.

### Настройки и frontend state

```text
Vue component → Tauri setter command
  → SettingsManager persistence/shared cache
  → иногда отдельный service/runtime state
  → settings-changed event
  → get_all_app_settings
  → root AppSettingsDto
  → локальные draft/copy в composables и components
```

Shared cache между `SettingsManager` и `AppState` устраняет часть рассинхронизации,
но порядок `validate → persist → apply runtime → emit` реализован командами
неодинаково. Service-local settings и frontend drafts требуют явных правил
commit/rollback.

## Инварианты качества

1. Внешний сетевой доступ закрыт по умолчанию; отсутствие credentials никогда
   не означает успешную авторизацию.
2. Text, routing policy, provider и effects принадлежат одной speech operation и
   не передаются через глобальные временные флаги.
3. Mutex/RwLock guard не пересекает сетевой запрос, audio operation, filesystem
   I/O или другой длительный `await`.
4. Async Tauri command не выполняет блокирующий sleep или потенциально длительный
   sync I/O на executor thread.
5. Пользовательский текст и secrets не попадают в штатные диагностические логи.
6. Frontend listener/timer освобождается даже при unmount во время асинхронной
   регистрации.
7. Rust и TypeScript используют один проверяемый IPC/event contract.
8. Изменение settings либо полностью применено к persistence/runtime/UI, либо
   возвращает явную ошибку без частичного commit.
9. CI-команды воспроизводятся локально и не требуют знания неописанных env
   настроек.

## Этапы исправления

### P0 — Security и request correctness

1. Сделать WebView authentication fail-closed для non-local clients:
   public access без stored token получает `401`; UPnP нельзя включить без token.
2. Добавить regression matrix для local/public IP, missing/wrong/valid token,
   query auth, cookie auth и UPnP preconditions.
3. Убрать prefix flags конкретного запроса из глобального `AppState`; передавать
   routing policy вместе с текстом до фактического WebView/Twitch dispatch.
4. Покрыть конкурирующие `normal`, `!` и `!!` запросы и export contamination.
5. Удалить полный пользовательский text из штатных `info/debug` logs; оставить
   operation id, длину, provider, channel status и безопасные причины ошибок.

### P1 — Runtime, lifecycle и локальные дефекты

1. Освобождать outer Telegram client mutex до Silero network operations;
   filesystem read выполнять вне async executor или через async API.
2. Убрать blocking sleep из preview/shutdown commands и sync write из export;
   заменить фиксированные ожидания на async signal/join с bounded timeout.
3. Исправить online spellcheck compatibility path по TASK-118 и добавить тесты
   для `online`, `offline`, `off` и пустого input.
4. Ввести общий frontend helper для async listener lifecycle с rollback частично
   зарегистрированного набора; мигрировать settings, Twitch, WebView, TTS,
   phrase history и playback window.
5. Сохранить существующие UI состояния error/cancel/retry и не скрывать
   command-not-found/network errors как успешный пустой результат.

### P2 — Воспроизводимые quality gates

1. Вернуть зелёный `cargo clippy --all-targets --all-features -- -D warnings`,
   устраняя diagnostics малыми module-scoped изменениями. `allow` допустим только
   на внешне заданной boundary и с комментарием причины.
2. Исправить Windows test runtime/linkage для eSpeak/native dependencies;
   `cargo test` должен запускать binary, а не только собирать его без ошибок.
3. Сохранить `cargo fmt --check`, `cargo check`, frontend tests/build и docs check
   как обязательную матрицу.
4. Добавить автоматическую проверку literal frontend invokes/listens против
   зарегистрированных backend commands/events.
5. Добавить contract tests для security, routing, settings transaction и
   cancellation; runtime-only сценарии описать короткими воспроизводимыми
   runbooks.

## Состояние на 2026-07-29

Завершён обязательный defect-fix срез:

- WebView authentication и UPnP переведены в fail-closed режим;
- routing flags глобального перехвата стали request-local с сохранением wire
  shape frontend event;
- устранены выявленные lock-across-await, blocking sleep/sync I/O и утечки
  асинхронно регистрируемых frontend listeners;
- spellcheck использует зарегистрированную backend-команду для обоих enabled
  режимов, а диагностические логи больше не содержат пользовательский текст;
- восстановлены строгий Clippy gate и запуск Rust test harness на Windows;
  несовместимый eSpeak Debug CRT link устранён воспроизводимым Cargo profile,
  а существующие недетерминированные тестовые ожидания приведены к фактическим
  контрактам.

Следующий неархитектурный срез: автоматическая literal IPC/event parity check и
расширение contract tests для settings transaction и cancellation. Пункты
A1–A5 ниже не начаты и ожидают отдельного решения.

## Архитектурные proposals — требуют отдельного утверждения

Следующие пункты не реализуются автоматически в рамках defect-fix этапов.

### A1 — Единый SpeechApplicationService

Свести UI submit, global interception, replay/export preparation к одному
application-level входу с типом `SpeechRequest`:

```text
SpeechRequest {
  source,
  original_text,
  routing_policy,
  output_policy,
  correlation_id
}
```

Service создаёт immutable `SpeechSnapshot`, после чего queue worker остаётся
единственным владельцем state transitions. Export может использовать тот же
prepare/synthesize слой, но отдельную output policy без playback и routing.

Ожидаемый эффект: один preprocessing/provider/routing contract, отсутствие
legacy divergence и локальные тесты на все источники текста.

### A2 — Явный IPC contract layer

Ввести один contract module/manifest для command names, event names, DTO и error
codes. Минимальный вариант — Rust constants + TypeScript generated constants и
parity test; расширенный — schema/code generation для сериализуемых DTO.

Command boundary возвращает структурированную ошибку:

```text
CommandError { code, message, retryable }
```

Внутренний diagnostic context остаётся только в tracing. Frontend перестаёт
зависеть от текста Rust error и получает предсказуемые ветки retry/cancel.

### A3 — Settings transaction и ownership

Определить для каждого settings domain одного владельца и единый порядок:

```text
validate → persist shared cache → apply service runtime → emit revision
```

Setter возвращает обновлённый section DTO/revision. Frontend применяет ответ
либо откатывает draft, а общий reload остаётся recovery path, не единственным
способом синхронизации. Этот proposal согласуется с постепенной декомпозицией
`AppState` из TASK-117 и не требует big-bang rewrite.

### A4 — Разделение internal events и frontend events

Разнести `AppEvent` на внутренние domain/application events и явные
serializable frontend notifications. Routing выполняет один adapter, а payload
содержит correlation id и необходимые request-local данные. Строковые emit в
произвольных командах постепенно заменяются contract adapter-ом.

### A5 — Module boundaries для AI-ready изменений

Разделить особенно крупные файлы по причинам изменения, не по произвольному
размеру:

- Telegram: transport/auth, bot request correlation, parsers, download policy;
- VTube Studio: connection/auth, request correlation, typing action, item sync;
- settings: schema/defaults, validation, persistence, DTO mapping;
- commands: только parse/validate/dispatch, без domain workflow.

Для каждого выделенного модуля обязательны owner API, invariants в module docs,
focused tests и отсутствие публичного доступа к внутренним locks. Это уменьшает
контекст одной задачи и делает task-scoped diff проверяемым для AI-агента.

## Порядок согласования архитектуры

1. Сначала завершить P0 и локальные P1 fixes без изменения публичных контрактов.
2. Отдельно утвердить или отклонить A1–A5; спорные пункты вынести в decisions.
3. Реализовывать по одному seam: contract/helper → callers → закрытие legacy API.
4. Не совмещать service extraction, DTO migration и UI redesign в одном diff.

## Проверки этапа

```powershell
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
./scripts/check-docs.ps1
```

Дополнительно:

- security tests для WebView auth/UPnP;
- concurrency tests для request-local routing и cancellation;
- IPC/event parity check;
- ручной Windows smoke: interception, `!`/`!!`, export, disconnect Silero,
  preview cancel, WebView local/public access;
- task-scoped diff против baseline после каждой итерации.

## Критерии завершения

- P0 и P1 defects закрыты regression tests;
- CI и локальная матрица зелёные, либо platform-specific blocker оформлен как
  отдельная воспроизводимая задача с рабочим CI gate;
- пользовательский text отсутствует в штатных logs;
- literal IPC mismatch не проходит автоматическую проверку;
- architecture proposals A1–A5 получили явный статус: approved, rejected или
  deferred; утверждённые пункты реализованы отдельными этапами;
- `docs/development/architecture.md` отражает только фактически принятую и
  реализованную модель, а не предварительные proposals.

## Не входит

- новый TTS/AI provider;
- смена UI framework или state-management library;
- big-bang переписывание `AppState`, Telegram или VTube Studio;
- изменение модели хранения secrets без отдельного решения;
- cosmetic refactoring, не уменьшающий риск или размер контекста изменения.
