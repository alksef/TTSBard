# План исправлений review-020-agy

Security-находка из отчёта намеренно исключена и будет обсуждаться отдельным решением.

## Последовательность коммитов

1. Frontend settings refresh: coalescing reload и устранение `invoke<any>` в Telegram.
2. Backend settings event contract: единое событие после успешного изменения persisted settings.
3. Persistence: общие atomic write/lock/cache-гарантии для `SettingsManager` и `WindowsManager`.
4. Runtime TTS ownership и hot path: единый порядок persist/apply и использование managed `SettingsManager`.
5. Blocking I/O: точечный перевод дисковых setter-команд на безопасную async/spawn_blocking модель.
6. Config recovery: восстановление после повреждённых `settings.json`/`windows.json` с backup и тестами.
7. Keyboard hook lifecycle: отмена, `WM_QUIT`, `UnhookWindowsHookEx` и shutdown test/diagnostics.
8. Contract/architecture tests и актуализация документации без security-изменений.

Каждый шаг выполняется отдельным DeepSeek task-файлом, затем независимо проверяется (`cargo check`, `vue-tsc`, релевантные тесты), ревьюится и коммитится отдельно. При обнаружении проблем создаётся следующий iteration task, а не расширяется текущий.

