# План 93: DeepSeek-провайдер для AI + AI-грамматика по выделению

- **Дата:** 2026-07-08
- **Тип:** feature (backend + frontend)
- **Stage:** `docs/stage/15-ai-features-map-and-token-benchmark.md` (читать обязательно)
- **Подход:** (Часть 1) добавить DeepSeek как `AiProviderType` (OpenAI-совместимый API, дешёвый
  дефолт); (Часть 2) AI-проверка грамматики по **выделенному фрагменту** (точечный AI, не непрерывная проверка).
- **Контекст цикла:** см. `docs/deepseek/WORKFLOW.md`
- **Тестовый ключ DeepSeek** `sk-41451e4ea05a4e75b616ade7df5e36f0` — **НЕ коммитить в код/настройки по
  умолчанию**; использовать только для локального теста. По умолчанию поле пустое.

---

## Контекст (что есть сейчас)
- `AiProviderType` (`config/mod.rs`): `OpenAi`, `ZAi`. DTO-зеркало `AiProviderTypeDto` (`config/dto.rs:609`).
- `AiSettings` (`config/mod.rs`): `provider`, `openai: AiOpenAiSettings`, `zai: AiZAiSettings`, `prompt`.
  DTO `AiSettingsDto` (`config/dto.rs:701`): `provider`, `openai`, `zai`.
- `AiOpenAiSettings`/`AiZAiSettings`: `{ api_key: Option<String>, model, use_proxy }` (+ url для Z.ai).
- `AiProvider` enum (`ai/mod.rs:94`): `OpenAi(OpenAiClient)`, `ZAi(ZAiClient)`.
- `create_ai_client` (`ai/mod.rs:114`) — фабрика по provider.
- `ai/openai.rs` — клиент через `async-openai`; `correct(text, prompt)`.
- Команды `commands/ai.rs`: `set_ai_provider`, `set_ai_zai_*`, `set_ai_openai_*`, `correct_text`,
  `get_ai_completion`.
- Фронт: `SettingsAiPanel.vue`, `types/settings.ts` (AI-настройки), `InputPanel.vue` (`correct_text`).
- Кэш клиента: `AppState::get_or_create_ai_client` + `invalidate_ai_client`.

---

## Часть 1. DeepSeek-провайдер

DeepSeek API: OpenAI-совместимый, `POST https://api.deepseek.com/chat/completions`,
модель `deepseek-chat` (или `deepseek-v4-pro` — проверить актуальную через `opencode models deepseek`
или API docs; `deepseek-chat` deprecated с 2026-07-24 → миграция на V4 Flash; использовать
актуальную stable модель на момент реализации). Auth: `Authorization: Bearer <key>`.

### Бэкенд

#### `config/mod.rs` — новый тип + настройки
1. `AiProviderType` — добавить вариант:
   ```rust
   pub enum AiProviderType { OpenAi, ZAi, DeepSeek }
   ```
   `#[serde(rename_all = ...)]` — проверить как OpenAi/ZAi сериализуются (likely lowercase/kebab),
   добавить DeepSeek в той же схеме (напр. `"deepseek"`).
2. Новая структура (по образцу `AiOpenAiSettings`):
   ```rust
   pub struct AiDeepSeekSettings {
       pub api_key: Option<String>,
       pub model: String,           // default "deepseek-chat" / актуальная
       pub use_proxy: bool,
   }
   ```
   + `Default` impl (model = актуальная default, use_proxy=false, api_key=None).
3. `AiSettings` — добавить поле `pub deepseek: AiDeepSeekSettings`.
   **⚠️ `#[serde(default)]`** на поле `deepseek` в `AiSettings` (урок `playback_pause` / PhraseEntry:
   старые settings.json без `deepseek` должны десериализоваться).

#### `config/dto.rs` — зеркальный DTO
1. `AiProviderTypeDto` (`dto.rs:609`) — добавить `DeepSeek` + `From` impls (обе стороны).
2. `AiDeepSeekSettingsDto` (по образцу `AiOpenAiSettingsDto`, `dto.rs:634`) + `From` impls.
3. `AiSettingsDto` (`dto.rs:701`) — добавить `pub deepseek: AiDeepSeekSettingsDto`.

#### `config/settings.rs` — сеттеры/геттеры (по образцу openai/zai)
- `set_ai_deepseek_api_key(Option<String>)`, `set_ai_deepseek_model(String)`,
  `set_ai_deepseek_use_proxy(bool)`, `get_ai_deepseek_model()`. Сохранение в settings.json.
- `set_ai_provider` — расширить match на `DeepSeek`.

#### `ai/mod.rs` + клиент
- Вариант решения **A (переиспользовать async-openai с custom base_url)** — DeepSeek OpenAI-совместим,
  поэтому `OpenAiClient` можно параметризовать base_url `https://api.deepseek.com` и НЕ писать новый
  клиент. Если `OpenAiClient::new` уже принимает configurable base_url — добавить вариант.
- Решение **B (если base_url не параметризуется)** — новый `AiProvider::DeepSeek(DeepSeekClient)` с
  прямой реализацией `AiClient::correct` через `reqwest` POST на `/chat/completions`
  (меньше зависимостей, проще). Рекомендуется B, если A требует рефактора async-openai.
- `create_ai_client` (`ai/mod.rs:114`) — `AiProviderType::DeepSeek => ...`.
- **Prompt caching:** system prompt стабилен между запросами → DeepSeek кэширует автоматически;
  держать prompt жёстко/стабильным (как сейчас). Ничего доп. кода.

#### `commands/ai.rs` — команды
- `set_ai_deepseek_api_key`, `set_ai_deepseek_model`, `set_ai_deepseek_use_proxy`,
  `get_ai_deepseek_model` (по образцу `set_ai_zai_*`, `commands/ai.rs:75-111`).
- `set_ai_provider` (`commands/ai.rs:11`) — match на `"deepseek"`.
- `correct_text` / `get_ai_completion` — проверка `has_key` для `DeepSeek` варианта (`commands/ai.rs:217-220`).
- Регистрация в `lib.rs` `invoke_handler`.

### Фронт
- `types/settings.ts` (AI-настройки ~строки 256-282): добавить `deepseek: { api_key, model, use_proxy }`,
  `provider: 'openai' | 'zai' | 'deepseek'`.
- `SettingsAiPanel.vue`: добавить вариант DeepSeek в выбор провайдера + поля (api_key, model,
  use_proxy) по образцу OpenAI/Z.ai-секций.
- Ничего больше во фронте — `correct_text`/`get_ai_completion` провайдеро-агностичны.

---

## Часть 2. AI-грамматика по выделению

### Бэкенд
- Новая команда (ИЛИ параметр mode в `correct_text`):
  ```rust
  #[tauri::command]
  pub async fn ai_check_grammar(
      settings_manager: State<'_, SettingsManager>,
      state: State<'_, AppState>,
      text: String,            // выделенный фрагмент (или весь текст, если выделения нет)
  ) -> Result<String, String>
  ```
  - Переиспользует `get_or_create_ai_client` + `client.correct(text, GRAMMAR_PROMPT)`.
  - `GRAMMAR_PROMPT` (новая константа в `commands/ai.rs`):
    «Проверь орфографию и грамматику русского текста. Исправь ошибки, сохрани смысл, стиль и
    регистр. Верни только исправленный текст без пояснений. Если ошибок нет — верни текст как есть.»
  - **Валидация** (урок SECURITY): `MAX_GRAMMAR_TEXT_LEN` (напр. 10_000 символов) — отбрасывать/ошибка.
- Регистрация в `lib.rs`.

### Фронт
- Пункт в EditorMenu (если меню реализовано — stage 07 / план 79) «Проверить грамматику (AI)»:
  берёт **выделение** в CodeMirror (если есть) или весь текст → `invoke('ai_check_grammar', {text})`
  → заменяет выделение/текст результатом.
  - **Если EditorMenu ещё не реализован** — добавить кнопку рядом с «AI» (correct-button) в
    InputPanel: «Грамматика» (или иконка). Или отложить до EditorMenu — зафиксировать зависимость.
- CodeMirror: получить selection через `view.state.selection.main` → `doc.slice(from, to)`.
  Заменить через `view.dispatch({ changes: { from, to, insert: result } })`.
- Индикация loading (как `isCorrecting` для correct-button).

> Зависимость: Часть 2 (AI-грамматика) **не зависит** от Части 1 (DeepSeek) — работает с любым
> провайдером. Но имеет смысл делать вместе: DeepSeek = дешёвый путь тестировать грамматику.

---

## Риски (для ревью — особо проверить)
1. **Миграция настроек** — `#[serde(default)]` на `deepseek` в `AiSettings` (и DTO). Старые
   settings.json должны грузиться. Тест: удалить `deepseek` из settings.json → запуск без паники.
   (Урок `playback_pause`, commit 704be39.)
2. **OpenAI-совместимость DeepSeek** — `async-openai` может не позволить custom base_url легко.
   Решение B (reqwest напрямую) надёжнее; проверить, что response-формат совпадает
   (`choices[0].message.content`).
3. **Актуальная модель DeepSeek** — `deepseek-chat` deprecated 2026-07-24. На момент реализации
   использовать актуальную stable (V4 Flash / эквивалент). Зафиксировать в `Default::default()`.
4. **has_key для DeepSeek** — в `get_ai_completion` (`commands/ai.rs:217`) проверить ключ deepseek.
5. **Тестовый ключ не в коде** — `sk-41451e4e...` только локально (env / settings при тесте), НЕ
   в дефолтах/коммите. Проверить `git diff` на отсутствие ключа.
6. **Prompt caching стабильность** — system prompt не должен включать переменные (timestamp/рандом),
   иначе кэш не сработает (−98% input на DeepSeek). Сейчас prompt жёсткий — хорошо.
7. **MAX_GRAMMAR_TEXT_LEN** — валидация длины выделения; иначе большой текст = большой счёт.
8. **Приватность warn** — в SettingsAiPanel подсветить, что текст уходит на API (как с ключами).

## Критерии готовности (Definition of Done)
- [ ] `AiProviderType::DeepSeek` + `AiDeepSeekSettings` + DTO-зеркало + `#[serde(default)]`.
- [ ] Команды `set_ai_deepseek_*`, `get_ai_deepseek_model` + регистрация.
- [ ] `create_ai_client` создаёт DeepSeek-клиент; `correct`/`get_ai_completion` работают с ним.
- [ ] `ai_check_grammar` команда + GRAMMAR_PROMPT + валидация длины.
- [ ] Фронт: SettingsAiPanel — выбор DeepSeek + поля.
- [ ] Фронт: AI-грамматика (EditorMenu пункт ИЛИ кнопка в InputPanel) — по выделению.
- [ ] `cargo check` 0/0; `npx vue-tsc --noEmit` 0.
- [ ] Тест: settings.json без `deepseek` → загрузка без паники (миграция).
- [ ] Тест: `correct_text` через DeepSeek (локальный тест-ключ) возвращает осмысленный результат.
- [ ] Тест: AI-грамматика исправляет опечатки в выделении, возвращает текст без пояснений.
- [ ] Тестовый ключ НЕ в git diff.

## Не делать (out of scope)
- Авто-озвучку чата (отдельное направление, stage 15).
- Перефразирование (отдельный малый план после этого).
- LanguageTool онлайн (отказ — stage 15 решение).
- Непрерывную AI-проверку орфографии (гибрид: hunspell база + AI точечно — это и есть Часть 2).
