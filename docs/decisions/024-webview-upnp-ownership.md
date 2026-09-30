# DECISION-024 — UPnP port forwarding вне async lifecycle WebView

**Статус:** `accepted` (ROADMAP-114)
**Связано:** [ROADMAP-114](../roadmap/active/114-webview-transport-resilience.md), [DECISION-010](./010-webview-sse.md)

> **TL;DR.** Router I/O (`igd`: SSDP discovery, локальный адрес, SOAP add/remove)
> больше не выполняется в async-задачах. Каждый запрос уходит на blocking pool с
> ограниченным ожиданием (forward ≤ 5 с, remove ≤ 3 с) и получает номер
> поколения; последний запрос владеет состоянием, а устаревший успешный результат
> компенсируется — mapping, подтверждённый после stop/disable, снимается.
> Readiness TCP listener от forwarding не зависит.

## Контекст

`WebViewServer::start` вызывал `manager.forward()` синхронно **до**
`TcpListener::bind` и до отправки readiness, а `stop()`/`toggle_upnp` делали
синхронные SOAP-вызовы из async-задач супервизора. Медленный или недоступный
router задерживал готовность listener (панель показывала `Starting`) и занимал
Tokio-воркер; событие `ToggleUpnp` блокировало supervisor loop вместе с
broadcast текста. Full code review 2026-09-29, MAJOR.

Отдельно: `UpnpManager` был нетестируем — `search_gateway` вызывался напрямую, шва
для «искусственно медленного» adapter'а не было, поэтому политику нельзя было
подтвердить тестом.

## Решение

- Router I/O инкапсулирован в trait `RouterPortMapper` (`open`/`close`).
  Production-реализация — `IgdPortMapper` (discovery gateway, локальный адрес,
  SOAP); тесты подставляют fake с управляемым завершением.
- `UpnpManager` владеет желаемым состоянием (`desired`) и подтверждённым mapping
  (`mapping_open`). Каждый вызов `set_enabled` получает новое поколение (epoch).
- Любой router-вызов выполняется через `spawn_blocking` и `tokio::time::timeout`
  с бюджетом: `UPNP_FORWARD_TIMEOUT = 5s` (discovery + add), `UPNP_REMOVE_TIMEOUT
  = 3s`. Async-поток не блокируется ни на одном пути.
- Результат, чьё поколение перехвачено более новым запросом, не подтверждается
  (`Err("superseded")`). Если он физически противоречит текущему намерению, router
  приводится к намерению компенсирующим вызовом; belief `mapping_open` при
  компенсации не используется как доказательство.
- Ожидание истекло — операция не подтверждена, но blocking-задача продолжается:
  её поздний успех обрабатывается отдельной задачей (подтверждение, если
  намерение не изменилось, иначе компенсация).
- `start()` отправляет readiness сразу после bind, а forwarding запускает
  отдельной задачей только после readiness. `stop()`/`toggle_upnp` — `async` и
  ожидают bounded-операцию; supervisor выполняет toggle в отдельной задаче, чтобы
  router не задерживал обработку остальных событий.
- `Drop` не блокирует уничтожающий поток: закрытие уходит в blocking pool
  текущего runtime. Если runtime уже остановлен, mapping снимается роутером по
  истечении lease (1 час).

## Последствия

- Readiness listener и обработка остальных async-задач не зависят от состояния
  router; недоступный router даёт warning в лог, но не задержку `Running`.
- Поздний `add_port`, завершившийся после `stop()`/`toggle_upnp(false)`, больше не
  оставляет mapping: он гасится компенсацией. Обратный случай — поздний
  `remove_port` после нового `enable` — приводит к повторному открытию.
- Провал forwarding по-прежнему не влияет на доступность сервера: тумблер
  `upnp_enabled` отражает настройку, а не факт mapping (как и раньше); результат
  виден в логах.
- Появился шов для тестов: политика подтверждена fake adapter'ом с управляемым
  завершением, включая timeout, superseded-результат и «поздний mapping после
  stop».

## Условия пересмотра

- Понадобится видимый пользователю статус mapping (успех/причина отказа) — тогда
  нужен request/response-путь от команды `set_webview_upnp_enabled` к владельцу
  forwarding, а не текущая fire-and-forget семантика.
- Понадобится несколько одновременных mapping (несколько портов/протоколов) —
  тогда `UpnpManager` перестаёт владеть одним портом и становится реестром.
- Router I/O появится в другом месте приложения — тогда `RouterPortMapper`
  переезжает в общий модуль, а не дублируется.
