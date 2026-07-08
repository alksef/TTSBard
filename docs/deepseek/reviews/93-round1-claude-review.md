# Review 93 round1 — план 93 (DeepSeek + AI-грамматика): APPROVED (после правок Claude)

**Дата:** 2026-07-08
**План:** `docs/deepseek/plan/93-2026-07-08-deepseek-provider-and-ai-grammar.md`
**Task:** `docs/deepseek/tasks/93-round1-01.md`
**Вердикт:** ✅ **APPROVED** — после 1 правки Claude (bug в Default impl, пойманный тестом).

## Что реализовал DeepSeek (всё в scope)
- `ai/deepseek.rs` (новый, 206 строк) — `DeepSeekClient`: async-openai + фиксированный
  `https://api.deepseek.com/v1`, прокси по образцу openai.rs. ✅ точно как ZAi, но без поля url.
- `config/settings.rs` — `AiProviderType::DeepSeek`, `AiDeepSeekSettings`, поле `deepseek` в
  `AiSettings` с `#[serde(default)]`, Default impl, сеттеры/геттеры. ✅
- `config/dto.rs` — `AiProviderTypeDto::DeepSeek` + `AiDeepSeekSettingsDto` + оба From impls. ✅
- `config/mod.rs` — re-export. ✅
- `ai/mod.rs` — `AiProvider::DeepSeek` + match arms (correct/create_ai_client/hash). ✅
- `commands/ai.rs` — `set_ai_deepseek_*` (4 команды) + `has_key` в get_ai_completion + `ai_check_grammar`
  (MAX_GRAMMAR_TEXT_LEN=10K, GRAMMAR_PROMPT). ✅
- `lib.rs` — регистрация всех команд. ✅
- `types/settings.ts` — DeepSeek-типы. ✅
- `SettingsAiPanel.vue` — DeepSeek-карточка (api_key, use_proxy, save). ✅
- `EditorMenu.vue` — `grammar` emit + пункт «AI: грамматика». ✅
- `InputPanel.vue` — `@grammar="checkGrammar"`, `isCheckingGrammar`, DeepSeek в `isProviderConfigured`. ✅

Все изменения — в scope (AI/настроек/EditorMenu/InputPanel). Вне-scope правок НЕТ (в отличие от
плана 92, где DeepSeek тронул SettingsAiPanel — здесь это легитимно).

## Правка Claude (после ревью)

### CRITICAL — `AiDeepSeekSettings::default()` давал пустую модель
**Найдено тестом** `ai_settings_deserializes_without_deepseek_field`: старый settings.json (без
поля `deepseek`) десериализовался БЕЗ паники (`#[serde(default)]` сработал), НО `model` оставался
`""` вместо `"deepseek-chat"`.

**Причина:** `#[derive(Default)]` на структуре + `#[serde(default = "default_deepseek_model")]` на
поле `model`. Атрибут `serde(default = "fn")` применяется **только при десериализации этого поля**,
а `Default::default()` структуры даёт `String::default()` = `""`. Когда `deepseek` отсутствует в
JSON → serde зовёт `AiDeepSeekSettings::default()` → `model = ""`. DeepSeek-клиент с пустой моделью
упал бы при запросе (новые пользователи / старые конфиги).

**Фикс:** убран `Default` из derive, добавлен явный `impl Default for AiDeepSeekSettings` с
`model: default_deepseek_model()`. Аналогично как `AiSettings` уже имеет явный Default.
(Унаследованный latent-баг в `AiOpenAiSettings`/`AiZAiSettings` — тот же derive(Default) + serde-fn
— НЕ правлю: там поля всегда присутствовали в существующих конфигах. Зафиксировано как известное.)

## Тесты (добавил Claude, в `config/settings.rs::tests`)
- `ai_settings_deserializes_without_deepseek_field` — backwards-compat: старый JSON без deepseek
  грузится, model = `"deepseek-chat"` (именно он поймал баг). ✅
- `ai_settings_deepseek_round_trip` — serialize→deserialize DeepSeek-настроек. ✅
- `ai_provider_type_serde_lowercase` — `"deepseek"` ↔ enum (фронт шлёт lowercase). ✅

## Сборка / верифика
- `cargo check` — 0 ошибок, 0 warnings.
- `cargo clippy --lib` — чисто по deepseek/grammar.
- `npx vue-tsc --noEmit` — 0 ошибок.
- `cargo test --lib config::settings::tests::` — **3/3 passed**.

## Осталось (runtime, не автоматизировано)
- Реальный вызов DeepSeek API (тестовый ключ `sk-41451e4e...`, локально): `correct_text` /
  `get_ai_completion` / `ai_check_grammar` через DeepSeek-провайдер. Требует запуска приложения +
  ввода ключа в Settings → проверка ответа. Юнит-тесты покрывают конфиг/миграцию; API-вызов —
  ручной тест.
- AI-грамматика работает с **всем текстом** (как correct-button), не с выделением — selection можно
  добавить позже (зафиксировано в задаче C3 как минимально-жизнеспособный подход).

## Итог
План 93 реализован корректно. Один баг пойман тестом (пустая модель Default — не DeepSeek-чекбоксом).
Сборка 0/0, тесты 3/3, clippy чист. DeepSeek-провайдер + AI-грамматика готовы (pending ручной API-тест).
