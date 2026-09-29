---
id: ROADMAP-113
status: planned
created: 2026-09-29
updated: 2026-09-29
related_tasks: []
---

# ROADMAP-113 — Согласованное сохранение настроек Input Server и Twitch

## Контекст

Полное ревью на `c873ff9` подтвердило две гонки: форма Input Server может
подтвердить порт, который backend не сохранил; параллельные полные записи Twitch
settings могут завершиться в обратном порядке и оставить устаревшее значение.
Локальное evidence —
`.work/ai/full-review/2026-09-29-full-code-review/reviews/review-001-2026-09-29.md`.

## Цель

После сохранения UI показывает подтверждённое persisted значение; быстрые правки
и ошибки не оставляют frontend и backend с разными настройками. Отдельные формы
исправляются независимыми bounded tasks.

## Этапы

1. Input Server: фиксировать payload в момент Save и определить поведение поля
   при in-flight save и позднем `refreshSettings`. Проверить A→B во время await,
   success и failure/rollback.
2. Twitch: упорядочить или объединить сохранения checkbox-полей, чтобы старый
   полный snapshot не перезаписывал новый. Проверить обратный порядок завершения,
   ошибку одной операции и сохранение соседних полей.

## Затрагиваемые места

- `src/composables/useInputServer.ts`, `src/components/InputServerPanel.vue`,
  ближайший composable/UI test.
- `src/composables/useTwitch.ts`, `src/components/TwitchPanel.vue` и тесты.
- `src-tauri/src/commands/twitch.rs` и settings owner — только если выбранная
  семантика требует backend API; выбор между сериализацией полных записей и
  отдельной field-командой согласовать перед такой правкой.

## Критерии результата

- Deferred IPC tests показывают, что UI не утверждает сохранение несохранённого
  порта и не откатывает правку к ложному baseline.
- Пересекающиеся Twitch saves завершаются тем же значением, которое UI показывает
  как подтверждённое; соседние поля не теряются.
- Проходят focused Vitest tests, `npm test`, `npm run build`, IPC/settings checks.

## Не входит

Общий rewrite settings DTO или всех форм настроек без доказанного такого же риска.
