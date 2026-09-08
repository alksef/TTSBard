---
id: ROADMAP-098
status: completed
created: 2026-09-08
updated: 2026-09-08
related_tasks: []
---

# ROADMAP-098 — Снижение стоимости безопасных AI-изменений

## Контекст

Repository-wide AI-ready review на baseline `1d91e7e` проверил холодный старт,
навигацию, ownership, локальность изменений и исполнимые доказательства по
основным доменам проекта. Локальный отчёт:
`.work/ai/full-review/2026-09-08-ai-ready-project/reviews/ai-ready-review-001-2026-09-08.md`.

Общий verdict — `ready with targeted gaps`: карта репозитория и большинство
границ уже помогают агенту работать безопасно, 1088 frontend- и 1879
Rust-тестов проходят, однако четыре конкретных пробела увеличивают риск или
стоимость следующего малого изменения.

## Цель

Закрыть доказанные пробелы AI-readiness без общего архитектурного rewrite:
обязательные проверки должны выполняться в CI, карта TTS provider onboarding —
соответствовать фактическим consumers, focused Rust proof — находиться без
догадок, а mutable state — извлекаться из `AppState` только по одному реально
мешающему доменному seam.

## Пункты remediation

### 1. Полный обязательный frontend и contract gate

CI сейчас запускает frontend build и IPC checks, но не запускает основной
Vitest suite, settings parity и speech contract checks. Использовать
существующие команды, не создавая параллельный test harness:

- `npm test`;
- `npm run check:settings` и `npm run test:settings`;
- `npm run check:speech-contract` и `npm run test:speech-contract`.

Эти проверки должны быть обязательной частью обычного CI на pull request и push
в `master`. Разделение на отдельный Node job допустимо, если оно сохраняет
понятную диагностику и не дублирует установку зависимостей без необходимости.

### 2. Достоверная карта добавления TTS-провайдера

Текущая строка `docs/development/architecture.md` называет только backend engine,
реализацию и registry, хотя фактический маршрут также включает settings/DTO,
command registration, frontend types, selection/card, visibility metadata,
локализацию и contract checks.

Сначала обновить архитектурную карту до фактического onboarding route и назвать
обязательные проверки. Затем отдельным bounded probe определить, создаёт ли
дублирование built-in metadata в `ttsProviderVisibility.ts` и `TtsPanel.vue`
реальную ошибку при добавлении следующего провайдера. Единый metadata source
вводить только при подтверждённой пользе; документационная правка не должна
маскировать существующий широкий consumer set.

### 3. Находимый focused Rust proof

Дополнить development guide одним каноническим примером запуска теста по
module/test-name filter через `scripts/cargo.ps1` и коротким способом найти
colocated test через `rg`. Не поддерживать вручную полный каталог тестов:
пример должен обучать маршруту, а имена конкретных доменов остаются source of
truth в коде.

### 4. AppState — finding принят без превентивного рефакторинга

Review подтвердил наличие публичных mutable handles, но не доказал конкретный
дефект, потерю атомарности или невозможность focused test. Общая декомпозиция
`AppState` поэтому не является remediation этого roadmap.

Trigger-based правило закрепляется в DECISION-004: новый owner/service
выделяется только под наблюдаемый дефект или конкретное изменение, которому
мешает текущая граница. Размер `AppState` и наличие lock сами по себе не создают
implementation work.

## Порядок выполнения

1. **098a — CI proof gates.** Самая дешёвая механическая защита уже существующих
   invariants.
2. **098b — TTS onboarding map и focused-test navigation.** Устранение
   misleading/скрытого маршрута без продуктового изменения.
Этапы 098a и 098b независимы и не должны смешиваться с продуктовым кодом.
Значимый выбор новой service boundary остаётся отдельной будущей работой и
требует конкретного сигнала и согласования пользователя.

## Критерии завершения

- CI блокирует merge/push при падении frontend Vitest suite, settings parity или
  speech contract checks и показывает, какой gate нарушен.
- Архитектурная карта нового TTS provider перечисляет фактические слои и
  обязательные consumer/contract проверки; cold-start агент не восстанавливает
  маршрут широким repo scan.
- Development guide показывает воспроизводимый Windows-safe маршрут от имени
  Rust invariant до focused test через `scripts/cargo.ps1`.
- DECISION-004 содержит trigger-based правило и не создаёт blanket backlog
  декомпозиции `AppState`.
- Проходят `npm test`, `npm run build`, все IPC/settings/speech contract checks,
  `./scripts/check-docs.ps1`, Rust test/check/clippy через `scripts/cargo.ps1`.
- Финальный cold-start probe по изменённым маршрутам не обнаруживает прежние
  препятствия; task-scoped diff и результаты команд сохранены в `.work/ai/`.

## Не входит

- объявление всего репозитория «AI-ready навсегда» или введение числового score;
- big-bang decomposition `AppState`;
- добавление нового TTS-провайдера как продуктовой функции;
- code generation IPC/DTO только ради унификации стиля;
- дробление крупных файлов без наблюдаемого препятствия конкретному изменению;
- новый test framework вместо уже существующих Vitest, Rust tests и scripts.

## Outcome

Все actionable gaps закрыты без продуктового или архитектурного rewrite.

- CI запускает основной Vitest suite, settings parity и speech contract checks
  вместе с их собственными regression tests. Существующие IPC checks и build
  сохранены отдельными диагностируемыми шагами.
- Architecture содержит фактический onboarding route нового TTS provider через
  backend contract/registry, settings/DTO, command registration, frontend
  type/card/selection/visibility/localization и обязательные contract checks.
- Development guide показывает Windows-safe маршрут поиска и запуска focused
  Rust test через `scripts/cargo.ps1`; приведённый пример выполнен фактически.
- Bounded probe истории добавления ElevenLabs дал verdict `no refactor` для
  единого frontend metadata source: текущий typed ID map уже механически требует
  stable ID, а общая таблица не устранит provider-specific card/config/runtime
  wiring. Повторный refactor рассматривается только после наблюдаемого
  metadata-only пропуска.
- Общая задача декомпозиции `AppState` удалена. DECISION-004 теперь содержит trigger-based правило:
  owner/service выделяется по конкретному дефекту или препятствию изменению, а
  не из-за размера `AppState` или самого наличия lock.

Независимые проверки: `npm test` (1088), `npm run build`, IPC check+tests,
settings check+tests, speech contract check+tests, focused Rust test и
`scripts/check-docs.ps1` прошли. Локальные review/probe материалы сохранены в
`.work/ai/full-review/2026-09-08-ai-ready-project/`.
