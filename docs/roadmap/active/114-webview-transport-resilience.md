---
id: ROADMAP-114
status: exploring
created: 2026-09-29
updated: 2026-09-29
related_tasks: []
---

# ROADMAP-114 — Отзывчивый WebView lifecycle и устойчивый SSE

## Контекст

Полное ревью на `c873ff9` обнаружило синхронные gateway discovery и UPnP
add/remove в async lifecycle WebView. Оно также подтвердило, что `Lagged` у
broadcast receiver завершает SSE-поток. Локальное evidence —
`.work/ai/full-review/2026-09-29-full-code-review/reviews/review-001-2026-09-29.md`.

## Цель

Медленный или недоступный router не задерживает готовность listener и обработку
остальных async tasks. Медленный SSE consumer переживает пропуск событий и может
получать последующие события по тому же соединению.

## Этапы

1. Определить семантику UPnP при позднем успехе, stop/restart и timeout; выбрать
   lifecycle-safe ownership до реализации. Изолировать router I/O от async
   executor и не связывать readiness TCP listener с optional port forwarding.
2. Разделить `RecvError::Lagged` и `Closed` в SSE stream. Продолжать чтение после
   lag, завершать stream после close; определить необходимую диагностику пропуска.

## Затрагиваемые места

- `src-tauri/src/webview/server.rs`, `src-tauri/src/webview/upnp.rs`,
  `src-tauri/src/servers/webview.rs` и их focused tests.
- `docs/integrations/sse.md` — только если требуется уточнить контракт потери
  событий; bundled browser template и внешних consumers проверить на совместимость.

## Критерии результата

- Тест с искусственно медленным UPnP adapter подтверждает readiness, stop/restart
  и отсутствие позднего mapping после остановки по утверждённой политике.
- Тест превышения broadcast capacity подтверждает последующее событие на том же
  SSE stream; `Closed` завершает поток.
- Проходят focused Rust tests, `scripts/cargo.ps1` test/check/clippy и
  релевантные WebView/contract checks.

## Не входит

Замена SSE-протокола, новый брокер событий, изменение модели доступа WebView.
