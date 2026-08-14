---
id: ROADMAP-076
status: in_progress
created: 2026-08-17
updated: 2026-08-17
related_tasks: []
---

# ROADMAP-076 — Remediation по release-review v0.21.0..HEAD

## Контекст

Код-ревью диапазона v0.21.0..HEAD (26 коммитов) подтвердило 10 находок
(отчёт: `.work/ai/2026-08-17-075-ui-polish/reviews/release-review-v0.21.0..HEAD-2026-08-17.md`,
gitignored). Три из них заметны пользователю напрямую (маршрутизация по
default_route, перехват клавиш, мигание статусов), одна жжёт CPU
(respawn-цикл WebView). Остальные — надёжность и чистота.

## Пункты

1. **Submit применяет effective route** — `InputPanel.vue handleSubmit`
   маршрутизирует только по префиксу в тексте; default_route и tab-route
   влияют на UI, но не на доставку. Фикс: routing-решение на submit берёт
   `effectiveRoute(activeDecoded, tab.route, defaultRoute)`; для
   twitch_only без префикса — `deliverTwitchMessage(text)`; для
   no_twitch/voice_only — префикс добавляется в текст, отправляемый в
   backend (редактор не мутируется).
2. **Snapshot-race статус-компосаблов** — во всех трёх init() не await'ится
   listen() перед snapshot; старый снапшот может перезаписать свежее
   событие. Фикс: await'ить подписку до snapshot-запроса (паттерн уже есть в
   `useWebView.ts`).
3. **sendTest retry** — одноразовый get_webview_server_status при ошибке
   оставляет кнопку навсегда disabled; добавить retry/повторную сверку при
   открытии панели или повторный invoke по клику.
4. **Surface ошибок save_tabs** — runSave глотает Err; поднять ошибку в
   ref/toast (не молчать о потере вкладок).
5. **Respawn-loop WebView** — supervisor-ветка `Err(_)` без backoff; добавить
   задержку/проверку shutdown перед continue (и для Ok(Err(_)) путь уже
   ждёт задачу — выровнять).
6. **Intercept hook** — при Full/Disallow клавиша утекает в ОС и после
   смерти диспетчера перехват мёртв; swallow при успешном перехвате клавиши
   независимо от enqueue, логировать дроп, пересоздавать диспетчер при
   re-init.
7. **Grace period при выходе** — вернуть короткое ожидание (или join
   ключевых задач) между shutdown.cancel() и exit(0).
8. **Общая фабрика статус-источников** — один createRuntimeStatusSource
   вместо трёх копий init/convert; convertStatusFromRust — один модуль.
9. **history.rs не держать локи через fsync** — сериализовать под локом,
   писать файлы вне лока; убрать полные клоны datasets.
10. **Префикс в phrase-history** — писать в историю текст без маршрутного
    префикса (submittedDecoded.text).

## Этапы

- **076a** — пп. 1, 10 (frontend, InputPanel).
- **076b** — пп. 2, 3, 4, 8 (frontend, composables).
- **076c** — пп. 5, 6, 7, 9 (Rust backend; cargo проверяет Claude —
  у DeepSeek нет LIBCLANG_PATH).

## Критерии завершения

- маршрут доставки всегда совпадает с показанным в RouteSelector;
- иконки статусов не залипают после гонки init;
- sendTest доступен при живом сервере даже после неудачного snapshot;
- ошибка сохранения вкладок видна пользователю;
- WebView supervisor не зацикливается без backoff;
- перехват не теряет клавиши и переживает stop/start;
- выход даёт задачам отработать отмену;
- статус-компосаблы — одна реализация;
- history не блокирует читателей на fsync;
- история фраз без служебных префиксов;
- npm test/build, cargo check/test, check-docs зелёные.

## Не входит

- изменение контракта префиксов `!`/`!!`/`!t`;
- backend-диспетчер маршрутов (altitude-вопрос, отдельный roadmap при
  появлении новых destinations);
- UX-полировка из ROADMAP-075 (пп. 4, 6, 7 там).
