---
id: ROADMAP-063
status: completed
created: 2026-07-31
updated: 2026-08-13
related_tasks: []
---

# ROADMAP-063 — Фактическое runtime-состояние WebView-сервера

## Контекст

Панель WebView показывает «Запущен», когда в настройках установлено
`enabled=true`. Это desired/config state, а не доказательство, что TCP listener
успешно привязан и server task продолжает работать.

`WebViewServer::start()` выполняется в отдельной задаче. Ошибка bind или позднее
завершение сервера отправляет transient `webview-server-error`, после чего task
заканчивается. Управляющий цикл не наблюдает завершение `JoinHandle`, сохраняет
локальный `server_running=true`, а runtime settings и UI продолжают считать
сервер запущенным. Кнопки, блокировка port/bind settings и отправка тестового
сообщения поэтому основываются на намерении, а не на фактическом listener.

Атомарность сохранения WebView settings уже закрыта
[ROADMAP-059](../completed/059-integration-state-ownership-and-settings-atomicity.md).
Этот roadmap не меняет persist-контракт: он разделяет desired configuration и
runtime server status.

## Цель

Ввести один достоверный owner-controlled статус WebView-сервера, отслеживать
startup, bind failure, неожиданное завершение и штатную остановку, а в UI
показывать фактическое состояние независимо от сохранённого `enabled`.

## Продуктовый контракт

1. `settings.enabled` означает «сервер должен работать» и остаётся persisted
   desired state.
2. Runtime status имеет как минимум `Stopped`, `Starting`, `Running` и
   `Error(message)`; только `Running` означает успешно созданный TCP listener.
3. Статус `Running` публикуется после успешного bind, а не сразу после spawn
   server task или сохранения settings.
4. Любое завершение server task наблюдается supervisor и приводит к `Stopped`
   либо `Error`, даже если `enabled` осталось `true`.
5. Start/Stop/Restart завершаются наблюдаемым status transition; сообщение об
   успехе не опережает фактический bind/stop outcome.
6. Frontend показывает runtime status, но отдельно сохраняет desired state и
   позволяет пользователю исправить port/bind после ошибки запуска.
7. Один supervisor владеет server task; параллельные listener tasks для одной
   конфигурации не допускаются.

## Этапы

### P0 — Runtime status и сигнал готовности

1. Добавить сериализуемый `WebViewServerStatus` и owner storage рядом с
   WebView runtime state, не смешивая его с persisted settings.
2. Перед запуском публиковать `Starting`.
3. Перенести bind либо добавить readiness channel, чтобы supervisor получал
   подтверждение успешного `TcpListener::bind` и только тогда ставил `Running`.
4. Сохранять user-friendly error отдельно от полного диагностического контекста.
5. Добавить read command и событие `webview-server-status-changed` для начальной
   загрузки и фоновых transitions.

### P1 — Supervisor lifecycle

1. В основном `select!` одновременно наблюдать shutdown, управляющие события и
   завершение server `JoinHandle`.
2. Различать штатный Stop/Restart, startup failure и неожиданное runtime
   завершение.
3. На Stop дождаться остановки listener в ограниченный timeout; abort оставить
   fallback, а не единственным механизмом завершения.
4. На Restart гарантировать порядок `stop old → terminal status → start new` и
   запретить возврат статуса от старой generation.
5. Если `enabled=true`, не входить в фиктивный running-loop после завершения
   server task. Retry policy должна быть явной и ограниченной.

### P2 — UI по фактическому состоянию

1. Отображать `Запускается / Запущен / Остановлен / Ошибка` по runtime status,
   а не по `settings.enabled`.
2. Разрешать изменение port/bind и повторный запуск после `Error`, даже если
   desired `enabled=true`.
3. Активировать Send test и URL-as-running affordances только в `Running`.
4. Не показывать «Сервер успешно запущен» до readiness transition; bind failure
   должен сохраняться на экране дольше transient toast либо быть доступен рядом
   со статусом.
5. При монтировании сначала получить snapshot status, затем подписаться без
   потери перехода между read и listen.

### P3 — Recovery и наблюдаемость

1. Для startup bind/config errors использовать ручной retry после исправления,
   без фонового бесконечного цикла каждые несколько секунд.
2. Для неожиданного runtime failure определить ограниченный retry/backoff либо
   оставить явный `Error`; policy зафиксировать тестами.
3. Логировать generation, bind address, port, transition, exit class и elapsed
   startup/shutdown time без access token.
4. Проверить взаимодействие с UPnP: mapping создаётся только для запускаемой
   generation и удаляется при terminal transition.
5. Не считать отсутствие подключённых SSE-клиентов ошибкой сервера.

## Порядок выполнения

P0 и P1 выполняются одним backend-срезом с тестовым readiness seam. P2 следует
после стабилизации wire/status contract. P3 не должен задерживать устранение
ложного «Запущен»: автоматический retry является отдельным policy-решением.

## Критерии завершения

- свободный port даёт наблюдаемый переход `Stopped → Starting → Running`;
- занятый или запрещённый port даёт `Starting → Error`, при этом UI не показывает
  «Запущен» и позволяет изменить настройки;
- неожиданное завершение server task немедленно отражается в backend snapshot и
  frontend event;
- Stop и Restart не оставляют orphan listener или status от старой generation;
- `settings.enabled` и runtime status явно различаются в типах, коде и UI;
- тестовое сообщение и running-only controls недоступны вне `Running`;
- UPnP cleanup соответствует фактической generation сервера;
- проходят focused Rust/frontend tests, `cargo check --locked`, `npm run build`
  и `./scripts/check-docs.ps1`;
- ручная проверка покрывает свободный port, port-in-use, изменение bind address,
  Stop/Start/Restart, shutdown и подключение SSE-клиента.

## Не входит

- изменение SSE payload protocol и шаблонов WebView;
- замена Axum или Tokio server stack;
- объединение runtime-status Twitch, VTube Studio и WebView в общий enum;
- сохранение transient runtime error в пользовательский config;
- автоматический UPnP recovery вне lifecycle активной server generation.

## Outcome

Реализован runtime owner status `Stopped` / `Starting` / `Running` / `Error`,
который не сохраняется в `WebViewSettings`. `Running` следует только за
successful listener bind; snapshot command и typed Tauri event синхронизируют
UI без подмены desired `enabled`. Ошибка bind остаётся видимой и позволяет
исправить address/port, а test-send доступен только в `Running`.

## Связанные материалы

- [ROADMAP-043 — WebView typing events](../completed/043-webview-editor-typing-events.md)
- [ROADMAP-059 — integration state и атомарность settings](../completed/059-integration-state-ownership-and-settings-atomicity.md)
- [DECISION-019 — integration settings Arc contract](../../decisions/019-integration-settings-arc-contract.md)
