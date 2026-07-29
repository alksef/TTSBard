---
id: ROADMAP-040
status: completed
created: 2026-07-18
updated: 2026-07-30
related_tasks: []
---

# ROADMAP-040 — Пробелы тестового покрытия

Основные пробелы unit-покрытия закрыты. Интеграционные HTTP mock-тесты и
связанные улучшения CI осознанно отложены и не являются условием завершения
этого roadmap item.

## Исходный baseline

На момент создания roadmap (2026-07-18) состояние было следующим:

- В `src-tauri/src` около 287 unit-тестов; тесты есть в 31 из 95 Rust-модулей.
- Лучше всего покрыты audio DSP/effects/boundary, settings/migrations, history,
  Piper scanner/runtime, TTS registry и WebView security/server.
- Frontend unit-тестов нет: отсутствуют Vitest, Vue Test Utils и `npm test`.
- CI запускает `cargo test` на Ubuntu, но Windows job только собирает приложение.
- `src-tauri/tests/hook_filter_test.rs` тестирует локальную копию старого helper,
  а не production-код, поэтому реального регрессионного покрытия не даёт.

## Что покрыть

### P0 — быстрые и изолированные тесты

1. Frontend test foundation: Vitest, `npm test`, fake timers и тесты чистых
   функций `debounceAsync`/`relativeTime`.
2. Чистые Telegram parser-функции в `telegram/bot.rs`: ответы RU/UA/EN,
   лишние пробелы, частичные и неизвестные ответы.
3. Proxy/MTProxy validation: допустимые схемы, malformed URL, hex/base64
   secrets и граничные длины.
4. JSON persistence: atomic replace, отсутствие временных файлов, recovery с
   backup и сохранение исходника при ошибке.
5. Intercept production logic: `vk_to_name`, load/save/default/malformed JSON;
   удалить тест-копию, не связанную с production.

### P1 — после test foundation

6. `spellLinter.ts`: disabled/error paths, предложения, повторяющиеся слова и
   корректные диапазоны diagnostics.
7. Composables с mock Tauri API: `useAppSettings`, `useEditorTabs`,
   `useTelegramAuth`, cleanup listeners и ошибки `invoke`.
8. Playback state machine: queue limit/order, pause/resume/stop/repeat,
   cache eviction/replay и переход к следующей фразе. Нужен mockable audio seam.

### P2 — интеграционные контракты и CI

9. Отложено: локальный mock HTTP server для Fish/OpenAI/DeepSeek/custom providers:
   URL, headers/body, non-2xx, timeout и malformed response.
10. GitHub build workflow запускает frontend-тесты и Rust-тесты на Windows с
    настройкой `LIBCLANG_PATH`. Публикация отчёта покрытия отложена; жёсткий
    percentage gate не вводится.

## Порядок реализации

- Round 1: пункты P0 параллельными задачами по непересекающимся файлам.
- Round 2: spell linter и composables после появления Vitest foundation.
- Round 3: playback seam и HTTP contract tests отдельными узкими задачами.
- После каждого round: независимый просмотр diff и реальные targeted/full checks.

## Статус

Round 1 завершён 2026-07-18:

- добавлены Vitest foundation и 10 frontend-тестов;
- добавлены 72 Rust unit-теста: Telegram parsers, proxy/MTProxy, persistence и
  intercept;
- устаревший integration-тест локальной копии helper удалён;
- regression test обнаружил и зафиксировал зависающий Promise в
  `debounceAsync`; production-дефект исправлен минимально;
- независимо подтверждены `npm test` (10/10), `npm run build`,
  `cargo test --lib` (359/359) и `cargo check --locked`.

P1 завершён последующими изменениями:

- добавлены тесты `spellLinter`, `useAppSettings`, `useEditorTabs` и
  `useTelegramAuth` с mock Tauri API, включая ошибки и cleanup listeners;
- playback state machine покрыта тестами очереди, pause/resume/stop, replay,
  cache eviction и перехода к следующей фразе;
- по итоговой проверке 2026-07-30 проходят 17 frontend test-файлов с 424
  тестами и 1037 Rust-тестов.

## Outcome

Unit-test foundation и приоритетное P0/P1 покрытие реализованы. GitHub build
workflow запускает frontend-тесты и Rust-тесты на Windows до сборки приложения.
Локальные HTTP mock-серверы для Fish/OpenAI/DeepSeek/custom providers и
публикация coverage report отложены до появления практической необходимости;
жёсткий percentage gate не вводится. Roadmap закрыт без этих пунктов по
принятому решению о сокращении scope.
