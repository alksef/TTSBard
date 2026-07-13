# Review: Task 115 / Round 1

## Verdict

APPROVED

## Проверка

- Добавлено поле `windows.soundpanel.hide_on_blur` с default `true`, совместимое со старыми `windows.json`.
- Обработчик потери фокуса снова скрывает SoundPanel только при включённой настройке.
- В общих настройках интерфейса добавлен checkbox «Скрывать при потере фокуса».
- Основные настройки окна восстановлены без изменений после исправления ошибочного места вставки.
- `clickthrough` и `stay_visible` остаются отдельными настройками.

## Независимые проверки

- `cargo check --manifest-path src-tauri/Cargo.toml` — успешно.
- `npx vue-tsc --noEmit` — успешно.
- `git diff --check` — успешно; только предупреждения Git о LF/CRLF.
