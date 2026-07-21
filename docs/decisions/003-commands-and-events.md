# DECISION-003 — Commands и typed events

**Статус:** `accepted`

## Контекст

UI-запросы требуют ответа, а hooks, playback и фоновые сервисы порождают
долгоживущие изменения состояния.

## Решение

Действия UI проходят через тонкие Tauri commands. Внутренние producers
отправляют `AppEvent` через MPSC; `EventHandler` маршрутизирует их и при
необходимости публикует typed Tauri events.

## Последствия

Доменные правила не помещаются в IPC boundary. Новое событие требует типа,
исчерпывающего routing и стабильного payload, если его видит frontend.
