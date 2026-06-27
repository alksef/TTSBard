# DeepSeek Plan 73: Гибридное автодополнение продолжения (без AI + AI)

> **Для DeepSeek:** пиши реализацию сам. Здесь — инструкции (файлы/типы/сигнатуры/поведение),
> не готовый код. Общий план — `docs/plans/73-...`, контекст — `docs/stage/03-...`.
> Зависит от планов 71 (CM6) и 72 (Trie слов + `record_history`).

## Контекст
- Слой 0 (Trie слов из истории) — уже реализован в плане 72.
- AI-инфраструктура уже есть: модуль `ai`, `AiProvider`, `AppState.ai_client`,
  настройка `ai.*` (см. `docs/plans/55-...ai-correction`, `56-...ai-clients-refactor`).
  Переиспользуй её — не дублируй AI-клиент.
- `EditorSettings` в Rust `AppSettings` и DTO в `src/types/settings.ts` (`editor.quick`).

## Что сделать

### Backend (Rust)
1. **Слой 1 — n-граммы/Марков:** модуль, накапливающий биграммы/триграммы из истории
   ввода. Расширь `record_history` (план 72), чтобы параллельно обновлять n-граммную модель.
   Реализуй `get_phrase_completion(context: String, limit: usize) -> Vec<Suggestion>` —
   по последним 1–2 словам контекста предложить следующее слово / короткое продолжение
   из накопленных n-грамм. Оффлайн.
2. **Слой 2 — AI:** команда `get_ai_completion(context: String)` через существующий
   `AiProvider` (как в AI-коррекции `docs/plans/55-...`). Поведение:
   - включается только если `editor.ai_completion == true` И есть настроенный AI-ключ;
   - без ключа/выключено → graceful (фронтенд не вызывает, либо возвращает пусто).
3. **Настройки:** добавить `editor.ai_completion: bool` в Rust `EditorSettings` и в DTO
   `src/types/settings.ts`, а также в UI-схему настроек (по образцу AI-коррекции).

### Frontend (Vue/TS)
1. **Composable `src/composables/useTextCompletion.ts`**:
   - слой 1 (`get_phrase_completion`) — debounce;
   - слой 2 (`get_ai_completion`) — debounce + индикатор загрузки; вызывается только при
     включённой настройке и наличии ключа.
2. **Autocomplete CM6 (продолжение):** расширить попап из плана 72 так, чтобы он показывал
   группы:
   - точные слова (слой 0 — план 72);
   - n-граммные продолжения (слой 1);
   - AI-варианты (слой 2, отдельная группа/иконка/подпись, возможно с lazy-загрузкой
     после показа слоёв 0–1).
   Принимаются аналогично (Tab/Enter внутри попапа).
3. **UI настроек:** тумблер «AI-продолжение текста» в панели настроек (там же, где
   AI-коррекция).

## Поведение/ограничения
- Оффлайн-слои (0, 1) всегда работают без сети.
- AI-слой не должен блокировать ввод; долгий ответ — показать «догружается»/отменить при
  новом вводе.
- Качество без-AI продолжения на русском ограничено морфологией (ожидаемо; см. stage 03).
- Один попап — визуально различимые слои.
- Переиспользовать `ai` модуль, не плодить второй AI-клиент.
- TypeScript строгий; Rust — существующие паттерны (`AppError`, `State`).

## Критерии готовности
- После частых фраз n-граммный слой предлагает правдоподобное следующее слово/фразу.
- AI-слой работает при включённой настройке и ключе; без — fallback на слои 0–1.
- Попап объединяет слои с понятной визуальной разницей.
- `vue-tsc` / `cargo check` проходят.

---
**Статус: ВЫПОЛНЕНО** (28.06.2026)
- **Backend (Rust):**
  - `src-tauri/src/history.rs` — расширен NgramData: bigram/trigram модель с JSON persistence (`ngrams.json`)
  - `record_text` обновляет n-граммы параллельно с историей слов
  - `suggest_phrase(context, limit)` — поиск по последним 1–2 словам, сортировка по частоте
  - `get_phrase_completion` — Tauri-команда для n-граммного слоя
  - `commands/ai.rs` — добалены `get_ai_completion` (через существующий AiProvider), `set_editor_ai_completion`, `get_editor_ai_completion`
  - `config/settings.rs` — в EditorSettings добавлено `ai_completion: bool`
  - `config/dto.rs` — EditorSettingsDto.expanded с `ai_completion`
  - Команды зарегистрированы в `invoke_handler`
- **Frontend (Vue/TS):**
  - `src/types/settings.ts` — EditorSettingsDto расширен `ai_completion: boolean`
  - `src/composables/useTextCompletion.ts` — composable с getPhraseCompletion / getAiCompletion
  - `TtsEditor.vue` — единый гибридный CompletionSource (layer 0: keyword, layer 1: text, layer 2: class ✨ AI)
  - AI слой вызывается только при `editor.ai_completion == true`
- Все три слоя отображаются в одном попапе с разными `type` (разные иконки)
- `vue-tsc`, `vite build`, `cargo check` проходят
