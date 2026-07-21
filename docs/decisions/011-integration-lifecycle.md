# DECISION-011 — Lifecycle интеграций

**Статус:** `accepted`

## Контекст

Twitch, Telegram, WebView и VTube Studio имеют независимые подключения, ошибки
и повторные запуски.

## Решение

Каждая интеграция получает service с owned settings/status, идемпотентными
connect/disconnect и typed events. Задачи используют общий Tokio runtime и
cancellation token.

## Последствия

Сбой одного consumer не блокирует остальных. UI отображает backend status, а не
считает локальный флаг подтверждением подключения.
