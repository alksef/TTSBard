---
id: ROADMAP-113
status: completed
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

## Outcome

Обе гонки закрыты во frontend-владельцах состояния; backend, DTO и IPC-контракт не
менялись.

Input Server (`src/composables/useInputServer.ts`): payload фиксируется перед
`await`, успешно сохранённый снимок становится `confirmedSettings`, а drain-цикл
после каждого persist перечитывает `settings.value` и дозаписывает правку,
сделанную во время сохранения. Поздний `refreshSettings` больше не откатывает
форму: снимок применяется только при отсутствии несохранённой правки и только
если после его старта не прошёл более новый refresh или persist. Невалидный
промежуточный порт не отправляется и остаётся в поле с подсказкой, а роллбэк при
ошибке возвращает последнее действительно сохранённое значение, не текущую
редактируемую форму.

Twitch (`src/composables/useTwitch.ts`): записи всей секции идут через одну FIFO
очередь `persistTwitchSettings`, а checkbox-сохранения (`saveStartOnBoot`,
`saveSendOriginalText`) объединены в один drain. Параллельное переключение не
создаёт вторую полную запись: следующий снимок снимается после предыдущего
persist, поэтому устаревший полный snapshot не может перезаписать новый, а
соседние поля не теряются. Ошибка откатывает оба checkbox-поля к последнему
persisted значению и показывает локализованную ошибку; эхо `settings-changed`,
пришедшее во время записи, не воскрешает старое значение. Отдельная field-команда
backend не потребовалась.

Доказательства: 7 новых deferred-тестов в `src/composables/useInputServer.test.ts`
(A→B во время await, роллбэк к реально сохранённому значению, невалидный
промежуточный порт, отказ делать несохранённую правку baseline, позднее эхо поверх
новой записи, несохранённая правка при приходе снимка, перекрывающиеся
`refreshSettings`) и 5 новых в `src/composables/useTwitch.test.ts` (coalescing
двойного переключения, сохранение соседнего поля, откат при ошибке второй записи,
сериализация Save-кнопки за checkbox-записью, игнор persisted-эха). Сила тестов
проверена откатом обеих реализаций к baseline: 10 из 12 новых сценариев падают.

Проверки: focused Vitest (`useInputServer.test.ts`, `useTwitch.test.ts`) —
64 passed; `npm test` — 55 files / 1197 passed (было 1187); `npm run build` —
exit 0; `check:ipc` (321 команда), `test:ipc` (3 passed) и `check:settings` —
зелёные; `scripts/check-docs.ps1` — 208 файлов, passed.

Ограничения: живой UI-смоук Tauri-окна не проводился — сессия приложения в этой
среде недоступна; поведение подтверждено composable-тестами с отложенными IPC.
Обратный порядок завершения двух записей больше невозможен по построению (FIFO),
поэтому проверяется порядок вызовов и итоговое persisted-значение, а не гонка
таймингов. Аналогичная несериализованная запись полного снимка остаётся в
`src/composables/useWebView.ts` (`saveStartOnBoot`, `saveSendOriginalText`) и
`src/composables/useVTubeStudio.ts` (`saveStartOnBoot`); полное ревью этот путь не
помечало, поэтому он не входил в scope этого item.
