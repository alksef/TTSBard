---
id: ROADMAP-055
status: completed
created: 2026-07-29
updated: 2026-07-30
related_tasks: [TASK-117]
---

# ROADMAP-055 — Качество и AI-ready foundation

## Контекст

Кодовая база быстро расширилась вокруг нескольких независимых runtime-контуров:
редактора, очереди озвучивания, playback, настроек и сетевых интеграций. Основной
UI уже создаёт immutable snapshot задачи через `submit_speech`, а рядом оставались
legacy `TextReady`/`speak_text` declarations и отдельный export pipeline.
Настройки проходят через общий persisted cache, service-local state и frontend
reload events. IPC-команды, события и DTO при этом описываются вручную по обе
стороны границы Rust/TypeScript.

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

### Legacy TextReady и export

```text
legacy AppEvent::TextReady (активный producer отсутствует)
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
executor синхронной записью. При последующей инвентаризации выяснилось, что у
`TextReady` нет producer, а IPC-команда `speak_text` не вызывается frontend.
Удаление этого legacy path вынесено в ROADMAP-056; объединять его с рабочей
очередью через новый application service не требуется.

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
3. Исправить online spellcheck compatibility path: оба включённых режима (`online`,
   `offline`) используют зарегистрированную команду `spellcheck`; устранён вызов
   несуществующей `check_spelling_online`. Добавить тесты для `online`, `offline`,
   `off` и пустого input.
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

## Outcome

Завершён обязательный defect-fix срез:

- WebView authentication и UPnP переведены в fail-closed режим;
- routing flags legacy speech path стали request-local с сохранением wire
  shape frontend event;
- устранены выявленные lock-across-await, blocking sleep/sync I/O и утечки
  асинхронно регистрируемых frontend listeners;
- spellcheck использует зарегистрированную backend-команду для обоих enabled
  режимов, а диагностические логи больше не содержат пользовательский текст;
- восстановлены строгий Clippy gate и запуск Rust test harness на Windows;
  несовместимый eSpeak Debug CRT link устранён воспроизводимым Cargo profile,
  а существующие недетерминированные тестовые ожидания приведены к фактическим
  контрактам.

Незавершённые направления не удерживают этот item открытым. Автоматическая
проверка IPC/event parity, удаление legacy `TextReady`, разделение internal и
frontend events, локальная консистентность settings и декомпозиция крупных
модулей получили уточнённый scope в ROADMAP-056.

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
- request-local routing и cancellation tests;
- проверка Windows PE imports для eSpeak CRT;
- task-scoped diff против baseline после каждой итерации.

## Критерии завершения

- P0 и P1 defects закрыты regression tests;
- CI и локальная матрица зелёные, либо platform-specific blocker оформлен как
  отдельная воспроизводимая задача с рабочим CI gate;
- пользовательский text отсутствует в штатных logs;
- Windows test harness запускается без Debug CRT mismatch;
- оставшиеся архитектурные и contract improvements перенесены в ROADMAP-056 с
  утверждённым scope.

## Не входит

- новый TTS/AI provider;
- смена UI framework или state-management library;
- big-bang переписывание `AppState`, Telegram или VTube Studio;
- изменение модели хранения secrets без отдельного решения;
- cosmetic refactoring, не уменьшающий риск или размер контекста изменения.
