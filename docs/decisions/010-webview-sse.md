# DECISION-010 — WebView/SSE server

**Статус:** `accepted`

## Контекст

OBS и browser sources должны получать текст и typing state независимо от Tauri
window runtime.

## Решение

Поднимать локальный HTTP server с templates/API и SSE. Доступ зависит от bind
scope и token policy.

## Последствия

Browser contract отделён от внутреннего `AppEvent`. Public network access
требует token; шаблоны используют безопасную вставку текста.
