# DECISION-002 — Tauri application shell

**Статус:** `accepted`

## Контекст

Нужны гибкий web UI и нативный Windows backend без отдельного Chromium runtime.

## Решение

Использовать Tauri 2: Vue 3 отвечает за интерфейс, Rust — за системный ввод,
аудио, состояние, хранение и внешние интеграции.

## Последствия

Frontend/backend общаются через объявленные commands и events. Нативные
возможности требуют Tauri permissions/capabilities и проверки сборки.
