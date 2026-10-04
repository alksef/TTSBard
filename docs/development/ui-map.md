# Карта UI

Карта новых поверхностей в релизном срезе; не подтверждает прохождение runtime-проверок.

## Восстановление настроек

| Поверхность | Исходники | Условие открытия |
|---|---|---|
| Окно восстановления настроек | `src-settings-recovery/main.ts` → `src-settings-recovery/SettingsRecoveryApp.vue`; backend: `src-tauri/src/settings_recovery.rs` | Ошибка загрузки `settings.json` (ROADMAP-123) |

Окно описано в `src-tauri/tauri.conf.json`, доступы — в
`src-tauri/capabilities/settings-recovery.json`.

При добавлении поверхностей обновлять карту. Процесс постановки и проверки:
[AI-assisted workflow](./ai-workflow.md).
