# Rust Development Skill

Используйте для Tauri commands, backend services, runtime state и новых Rust
модулей.

## Проектные границы

- Tauri command — тонкая IPC-граница: валидирует вход, вызывает доменный API и
  возвращает стабильный frontend payload.
- Доменный service владеет settings, status, channels и блокировками.
  `AppState` служит composition container, а не хранилищем публичных mutable
  полей.
- Lock guard не должен пересекать длительную операцию или `await`.
- Внутри backend сохраняются contextual errors и structured `tracing`; на IPC
  boundary они преобразуются в безопасное для UI сообщение.
- Background producers используют typed `AppEvent`; frontend-visible payload
  считается частью контракта.
- Новая command регистрируется в Tauri invoke handler и получает тесты для
  чистой логики, когда это возможно.

Источники: [архитектура](../../docs/development/architecture.md),
[commands/events](../../docs/decisions/003-commands-and-events.md),
[service-owned state](../../docs/decisions/004-service-owned-state.md) и
[ошибки/проверка](../../docs/decisions/014-errors-and-validation.md).

## Проверки

```powershell
cargo test --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml
```

Выбирайте целевые тесты до полного suite и не удаляйте dead code автоматически:
сначала выясните, является ли он незавершённым API, platform-specific веткой или
действительно лишним кодом.
