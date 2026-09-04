---
id: ROADMAP-089
status: completed
created: 2026-09-02
updated: 2026-09-03
related_tasks: []
---

# ROADMAP-089 — Remediation полного ревью 2026-09-02

## Контекст

Полное snapshot-ревью дерева `2838caa` с delta-акцентом `0313817..2838caa`
(v0.23–v0.25 + OCR-линия) дало verdict `changes required`: 1 CRITICAL,
4 MAJOR, 15 MINOR. Отчёт:
`.work/ai/full-review/2026-09-02-full-code-review/reviews/review-001-2026-09-02.md`
(gitignored). Таски: `tasks/089*.md` в той же области.

MAJOR-пункты 2–4 — доработка контрактов действующих направлений
[ROADMAP-087](./087-external-text-input-server.md) и
[ROADMAP-088](../active/088-one-shot-screen-ocr.md), а не новая функциональность:
state-machine и hotkey-инварианты 088, сетевая инварианта «loopback = только
локальные клиенты» из 087.

## Пункты

1. **[CRITICAL] Host-validation input server** (доработка ROADMAP-087).
   Роутер не проверяет Host/Origin → DNS-rebinding позволяет веб-странице
   выполнить `POST /v1/speech` same-origin; при `incoming.auto_play=true`
   (дефолт) текст сразу озвучивается, в том числе в virtual mic. Фикс:
   middleware, отклоняющий Host ≠ `127.0.0.1:<port>`/`localhost:<port>`,
   + тест на чужой Host. Token не вводится (отдельное решение при появлении
   LAN-режима).
2. **[MAJOR] Потеря правки в `useOcr.saveSettings`** (доработка ROADMAP-088).
   Guard-drop второго save + подтверждение несохранённого состояния после
   await. Фикс: снапшот payload до invoke, сериализация сохранений.
3. **[MAJOR] OCR `stop()` не сбрасывает сессию** (доработка ROADMAP-088).
   `cancel_session`/`finish_selection` публикуют `Ready` из Disabled. Фикс:
   drop сессии в `stop()` + скрытие overlay; не публиковать `Ready` при
   пустом runtime-слоте.
4. **[MAJOR] Смена OCR hotkey во время сессии тихо теряется** (доработка
   ROADMAP-088). Rebind гейтуется на `Ready` в трёх местах, persist проходит
   всегда → persisted ≠ runtime до 15 c. Фикс: разрешить rebind под
   `transition_lock` из `SelectingArea`/`Recognizing` либо отложенный rebind.
5. **[MAJOR] fmt/clippy-долг + недостижимые CI-гейты.** CI срабатывает только
   на `pull_request`, проект пушит в master напрямую. Дельта внесла весь
   fmt-дрифт (4 файла) и часть clippy-долга; остальное — ruaccent-эра.
   Фикс: погасить fmt+clippy до зелёного, добавить `push: branches: [master]`.
6. **[MINOR-1]** `refreshSettings` в `useOcr` без out-of-order токена.
7. **[MINOR-2]** Снапшоты `useIncomingTexts` перезаписывают более новое
   событие (нужен event-wins guard как в `runtimeStatusSource`).
8. **[MINOR-3]** Невалидный payload `list_incoming_texts` молча → `[]`;
   нужен `loadError`.
9. **[MINOR-4]** Failed server/OCR job во «Входящих» без действий — dead-end
   строка и незаряжающийся счётчик.
10. **[MINOR-5]** Супервизор input server: `abort()` рвёт in-flight запросы;
    быстрый stop→start даёт двойной rebind. Нужен graceful shutdown + coalesce
    wake.
11. **[MINOR-6]** CPU-bound `crop_to_rgb` на Tokio-воркере → `spawn_blocking`.
12. **[MINOR-7]** Blur-cancel гонка overlay после TooSmall-retry (поздний blur
    убивает восстановленную сессию). Фикс в overlay; требует ручной проверки.
13. **[MINOR-8]** `OcrRuntimeError::Init` несёт несанитизированные пути в
    статус-сообщение; санитизировать по дисциплине `secret_log`.
14. **[MINOR-9]** Выход из приложения ждёт `transition_lock` до ~15 c при
    активном распознавании; shutdown не должен ждать инференс.
15. **[MINOR-10]** Сбой `SetWindowDisplayAffinity` фатален для сессии захвата
    (RDP/VM); понизить до warn — кадр уже снят до overlay.
16. **[MINOR-11]** Дублирование wire-контракта `OcrSettingsDto` в `useOcr.ts`
    и `types/settings.ts`; импортировать единый тип.
17. **[MINOR-12]** `OcrPanel`: `nowrap` + сырой текст ошибки бэкенда — клип на
    узком окне; ограничить ширину, категории вместо сырых строк.
18. **[MINOR-13]** Новые icon-only кнопки Sidebar без `aria-label`.
19. **[MINOR-14]** `ocr-result` эмитится без слушателей; удалить событие.
20. **[MINOR-15]** Окно `ocr-selection` в общей capability со всеми
    разрешениями; выделить минимальную capability.

## Этапы и коммиты

Каждый этап — отдельный коммит; CRITICAL строго первым и отдельно:

- **089a** — п.1. `fix(input-server): reject non-loopback Host headers`.
- **089b** — п.2. `fix(ocr): serialize settings saves in frontend composable`.
- **089c** — пп.3,4. `fix(ocr): reset session on stop and rebind hotkey during
  session`.
- **089d** — пп.6–9, 12 (frontend minors), 16–18. `fix(ui): incoming and ocr
  review minors`.
- **089e** — пп.10, 11, 13–15, 19, 20. `fix(backend): input server and ocr
  review minors`.
- **089f** — п.5, выполняется последним. `chore: clear fmt and clippy debt and
  gate master pushes`.

Перед 089f отдельным docs-коммитом идут правки workflow (full-review в
`.work/ai/full-review/`) и эта roadmap.

## Критерии завершения

- запрос с чужим Host отклонён (тест), DNS-rebinding больше не проходит;
- быстрая повторная смена настроек OCR не теряет последнюю правку (тест
  switch/edit-before-resolve);
- после `stop()` во время сессии статус никогда не публикует `Ready` при
  выключенном runtime; overlay скрыт;
- смена hotkey во время SelectingArea/Recognizing применяется к runtime;
- failed server/OCR job имеет действие во «Входящих» либо не копится;
- входящие не «пропадают молча» при контрактном дрейфе (loadError);
- `cargo fmt --check` и `cargo clippy --all-targets -- -D warnings` зелёные;
  CI выполняется на push в master;
- npm test, cargo test, check:ipc, check:settings, check-docs, debug build —
  зелёные;
- ручной smoke overlay (мульти-мониторы/DPI, blur-гонка) — на пользователе.

## Не входит

- token/auth для input server (отдельное решение при появлении LAN-режима);
- общий `nowrap`-паттерн message-box остальных 9 панелей (вне дельты ревью);
- перепроверка старого webview-сервера (вне дельты);
- упаковка моделей в установщик.

## Outcome

Remediation выполнена в шести коммитах `f30a1dd..3761369` и принята по итогам
независимого ревью: 20/20 пунктов закрыты, host-validation реализована шире
таска. Два нита ревью закрыты follow-up-коммитами: двухуровневая санитизация
путей в ошибках инициализации OCR (`31806cf`, фиксированная строка в UI,
маскированные пути только в логе) и env-устойчивый тест супервизора input server
на свободном loopback-порту (`e8dcebc`).

Итоговые проверки: `cargo fmt --check`, `cargo clippy --all-targets -- -D
warnings`, `cargo test` (1643 passed), `npm test` (789), `check:ipc`,
`check:settings`, `check:speech-contract`, `npm run build` — зелёные. CI
дополнительно запускается на push в master.

Отчёты ревью и таски остаются локально в
`.work/ai/full-review/2026-09-02-full-code-review/`. Ручной runtime smoke
оверлея OCR (мульти-мониторы/DPI, blur-гонка) остаётся на пользователе в
контексте [ROADMAP-088](../active/088-one-shot-screen-ocr.md).
