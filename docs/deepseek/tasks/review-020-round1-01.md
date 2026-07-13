# Review-020 round 1, step 1: frontend settings refresh

## Цель

Исправить только frontend-проблемы из review-020-agy, связанные с потерей обновления настроек во время загрузки и обходом типового DTO.

## Ограниченный набор файлов

- `src/composables/useAppSettings.ts`
- `src/composables/useTelegramAuth.ts`
- при необходимости `src/types/settings.ts`

Не изменять Rust/backend, security DTO, UI-разметку и бизнес-логику Telegram.

## Требования

1. В `useAppSettings` заменить ранний выход при `isLoading` на coalescing/serial reload: если `settings-changed`, `backend-ready`, `tts-provider-changed` или `soundpanel-bindings-changed` пришло во время загрузки, после текущего запроса должен выполниться ровно один дополнительный reload.
2. Не допустить бесконечного reload loop и сохранить существующий cleanup listeners.
3. В `useTelegramAuth.ts` заменить `invoke<any>('get_all_app_settings')` на существующий `AppSettingsDto` и сохранить текущее поведение загрузки голосов.
4. Не добавлять новые `any`, небезопасные casts или отдельную копию DTO.

## Приёмка

- `npx vue-tsc --noEmit` проходит.
- В diff нет `invoke<any>` и новых `any` в затронутых файлах.
- Ручная трассировка подтверждает: событие во время загрузки не теряется, несколько событий схлопываются в один следующий reload, cleanup не меняется.
- Изменения ограничены указанной целью и файлами.

