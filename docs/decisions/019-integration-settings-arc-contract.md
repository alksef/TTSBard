# DECISION-019 — Контракт общих settings-локов integration-сервисов

**Статус:** `accepted`
**Связано:** [ROADMAP-059](../roadmap/completed/059-integration-state-ownership-and-settings-atomicity.md)
(P3 closed), [DECISION-004](004-service-owned-state.md),
[DECISION-018](018-telegram-client-ownership.md)

> **TL;DR.** `settings: Arc<RwLock<...>>` у Twitch / WebView / VTube Studio
> остаётся публичным полем, доступным напрямую из команд и server-loops.
> Строгий owner-API для них **не вводится**: здесь `RwLock` хранит **кэш
> настроек** (read-heavy), а не lifecycle-state, и всем потребителям нужен только
> read-current — как у Telegram client Arc (DECISION-018). Но прямой доступ
> допустим **только при соблюдении порядка операций** (см. «Правила доступа»):
> именно нарушение этого порядка (`runtime write до persist`, `emit до runtime
> write`, не-персист `enabled`) было найдено и исправлено в `0224039` после
> первичного (неполного) аудита P3.

## Контекст

ROADMAP-059 P3 предлагал закрывать публичные locks Twitch/WebView/VTube «по одному
сервису при наблюдаемом дефекте». Первичный аудит (2026-07-30) заявлял «дефектов
нет» и закрыл P3 как acceptable — **но был неполным**: он проверял только
lock-through-await и partial-commit в section-save, пропустив нарушение порядка
`persist → runtime → emit` в соседних командах. Эти дефекты найдены и исправлены
в `0224039` (fix independently проверен: `cargo check/clippy --lib -D warnings`,
1066 lib-тестов зелёные):

| Сервис / команда | Дефект (до `0224039`) | Исправление |
|---|---|---|
| Twitch `save_twitch_settings` | `emit_settings_changed` шёл **до** runtime write → подписчик читал старый runtime | emit после runtime write |
| WebView `save_webview_settings` | `enabled` **не персистился**; emit до runtime write | `enabled` пишется в runtime; emit после |
| WebView `generate/regenerate token`, `set_upnp_enabled` | **runtime write до persist** → при ошибке персиста runtime рассинхронизирован с диском | persist → runtime (как в P0) |
| VTube `test_connection` / `connect` | runtime token write до персиста, без rollback при ошибке | `persist_and_apply_vts_token`: persist → runtime + rollback (disconnect) при ошибке |
| Telegram | stale-result: clear без проверки, что это тот же logical instance | `clear_client_if_current` (ROADMAP-041 invariant) |

Урок зафиксирован: **аудит порядка persist/runtime/emit надо делать grep-проходкой
по всем `persist_blocking` / `write().await` / `emit_settings_changed` в каждом
сервисе, а не доверять однобокому вердикту об «отсутствии lock-through-await».**

## Сводка доступа (после `0224039`)

| Сервис | settings lock | внешних прямых доступов | lock-through-await | порядок persist→runtime→emit |
|---|---|---|---|---|
| Twitch | `Arc<RwLock<TwitchSettings>>` | ~8 | нет | корректен (после `0224039`) |
| WebView | `Arc<RwLock<WebViewSettings>>` | ~14 | нет | корректен (атомарный `set_webview_section` P0 + token/upnp fix `0224039`) |
| VTube Studio | `Arc<RwLock<VTubeStudioSettings>>` | ~14 | нет | корректен (после `0224039`) |

Все внешние доступы следуют паттерну `lock → clone/drop → await` (guard не
пересекает network/IO). Пример: `servers/twitch.rs` — read настроек, clone нужных
полей, drop guard, затем event/restart.

## Контекст

ROADMAP-059 P3 предлагал закрывать публичные locks Twitch/WebView/VTube «по одному
сервису при наблюдаемом дефекте». Аудит (2026-07-30) каждого сервиса дал:

| Сервис | settings lock | внешних прямых доступов | lock-through-await | partial-commit | persist→runtime порядок |
|---|---|---|---|---|---|
| Twitch | `Arc<RwLock<TwitchSettings>>` | ~8 | нет | нет | корректен |
| WebView | `Arc<RwLock<WebViewSettings>>` | ~14 | нет | нет | корректен (атомарный `set_webview_section`, P0) |
| VTube Studio | `Arc<RwLock<VTubeStudioSettings>>` | ~14 | нет | нет | корректен |

Все ~39 внешних доступов следуют паттерну `lock → clone/drop → await` (guard не
пересекает network/IO). Пример: `servers/twitch.rs` — read настроек, clone нужных
полей, drop guard, затем event/restart.

## Почему не owner-API (в отличие от Telegram)

Telegram (`TelegramState.client`, DECISION-018) потребовал owner-API, потому что:
- `client` — **lifecycle state** (init/disconnect/reconnect/sign_out) с несколькими
  мутаторами и двумя реальными lock-through-await дефектами;
- swap-операция атомарно меняет клиента и возвращает старый для disconnect.

Settings-локи трёх сервисов принципиально другие:
- хранят **кэш настроек** (read-heavy), а не lifecycle-state;
- единственный мутатор — «применить новые настройки после persist»
  (`*write() = settings.clone()`), и он идёт **после** успешной записи файла;
- нет swap/disconnect-подобных операций под локом;
- `RwLock` (не `Mutex`) оптимизирован под многих читателей, что и происходит
  (server-loop + команды читают concurrently).

Введение owner-API (`service.save_settings(...)`) здесь — чисто структурная
правка на ~36 call-sites **без технической выгоды**: найденные дефекты (`0224039`)
касались **порядка операций** внутри команд, а не публичности lock'а — тот же
порядок (`persist → runtime → emit`) пришлось бы соблюдать и внутри owner-метода.
Закрывать дефект порядка owner-API вместо явного правила — значит прятать
контракт за слоем косвенности. ROADMAP-059 прямо предостерегает от закрытия этапа
«по checklist агента» и от owner-API без наблюдаемой проблемы, решимой только
инкапсуляцией.

## Решение: зафиксировать settings-Arc как допустимый контракт

1. `settings: Arc<RwLock<_>>` у Twitch / WebView / VTube Studio остаётся публичным.
2. **Обязательные правила** для всех прямых доступов (включая команды и
   server-loops):
   - `lock → clone/drop → await`: guard **не пересекает** network/IO/await
     (ROADMAP-059 инвариант #3);
   - любое пользовательское действие, меняющее настройки, применяет их строго как
     `validate → persist → runtime write → emit` (ROADMAP-059 инвариант #1):
     **сначала** успешная запись файла, **затем** runtime write, **затем** emit.
    Runtime write до persist — дефект (рассинхрон при ошибке персиста); emit до
     runtime write — дефект (подписчик читает старый runtime). Это правило было
     нарушено и исправлено в `0224039`;
   - валидация — до персиста;
   - **все** поля пользовательского действия должны персиститься (не только
     часть секции — см. пропущенный `enabled` в `0224039`).
3. Если появится **наблюдаемый дефект, не покрываемый этими правилами** (новый
   lifecycle-mutатор на settings-локе, гонка) — только тогда заводится отдельный
   owner-API task под этот сервис (как было с Telegram).

## Где зафиксировано, чтобы подтягиваться при ревью

- Настоящий decision (обоснование «почему не owner-API» + таблица дефектов).
- ROADMAP-059 Outcome — fix `0224039` отмечен как часть закрытия.
- При ревью изменений в `commands/twitch.rs`, `commands/webview.rs`,
  `commands/vtube_studio.rs`, `servers/twitch.rs` — проверять соблюдение правил
  выше. Практическая проходка: grep `persist_blocking`, `\.write().await`,
  `emit_settings_changed` в затронутом файле и сверить, что порядок
  `persist → runtime → emit` соблюдён и guard не пересекает await.

## Условия пересмотра

Контракт пересматривается, если: (1) появится lifecycle-mutator на settings-локе,
аналогичный Telegram swap/disconnect; (2) обнаружится реальный lock-through-await
или partial-commit; (3) settings-лок начнёт хранить не кэш, а состояние с
invariant'ами, нарушаемыми прямым доступом. До этого owner-API для этих сервисов
не вводится.
