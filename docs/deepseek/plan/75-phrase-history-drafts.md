# DeepSeek Plan 75: История фраз как черновики (извлечение в редактор)

> **Для DeepSeek:** пиши реализацию сам. Здесь — инструкции (файлы/типы/сигнатуры/поведение),
> не готовый код. Общий план — `docs/plans/75-...`, контекст — `docs/stage/05-...`.
> Зависит от планов 71 (редактор) и 72 (`HistoryManager`, persistence-паттерны).
>
> **Решение стадии 05:** Хранение **A** (`phrase_history.json`, persistent) + UX **Вариант 2c**
> (список фраз в InputPanel, клик → загрузить в редактор для правки). **Вариант 1 рефактора:**
> единое хранилище фраз — `phrase_history.json` становится **единственным журналом**;
> «5 недавних» окна управления воспроизведением (план 74) и полный список (план 75) — это
> разные views из одного хранилища, а не два журнала.

## Контекст кода (проверено Claude, актуально на 2026-06-30)
- `src-tauri/src/history.rs` — `HistoryManager`: Trie слов + n-граммы, persistence в
  `input_history.json` / `ngrams.json` через `parking_lot::RwLock` + off-thread `spawn_save`.
  `history_paths()` (history.rs:230) возвращает `(input_history.json, ngrams.json)` из
  `dirs::config_dir().join("ttsbard")`.
- `src-tauri/src/commands/history.rs` — `HistoryState(pub Arc<HistoryManager>)`; команды
  `get_history_suggestions`/`record_history`/`clear_history`/`get_phrase_completion`.
- `src-tauri/src/lib.rs:232-247` — инициализация: `history_paths()` → `HistoryManager::new` →
  `HistoryState` → `.manage(history_state)`. **`PlaybackManager`** (`setup.rs:88`) создаётся
  **позже** `HistoryManager`, поэтому `Arc<HistoryManager>` можно прокинуть в него.
- **План 74 сегодня (РАЗОБРАНО):** в `src-tauri/src/playback.rs` есть **свой** in-memory
  журнал фраз:
  - `PhraseEntry { id, text, timestamp }` (playback.rs:33);
  - `Shared.phrase_history: VecDeque<PhraseEntry>` (playback.rs:68), `PHRASE_HISTORY_SIZE=5`;
  - `add_history(id, text)` (playback.rs:356) вызывается из `enqueue` (351) и
    `on_playback_finished` (431);
  - `get_state()` (441) отдаёт `recent: Vec<PhraseEntry>` в `PlaybackStateDto`;
  - `replay_from_cache(id)` (403) ищет текст фразы **в этом же** `phrase_history` по id.
  - Команды `get_playback_state` / `replay_phrase` (commands/playback.rs:48,55)
    **зарегистрированы, но фронтенд их НЕ вызывает** (`grep` по `src/` пуст). Т.е. UI-окна
    плана 74 сейчас нет — перенос `recent` в единое хранилище **ничего не ломает на UI**.
- `src-tauri/src/commands/mod.rs::speak_text_internal` (81): эмитит `TextSentToTts` (179) и
  зовёт `pb.enqueue(phrase_id, text, audio_data)` (252). Здесь же есть `state: &AppState`.
- Frontend: `InputPanel.vue` содержит `TtsEditor.vue`; `useInputHistory.ts` — образец обёртки
  над invoke (debounce); `InputPanel.vue:111` уже зовёт `invoke('record_history', …)`.

## Архитектурное решение (Вариант 1 — единый источник правды)
`phrase_history.json` — **единственный** журнал целых фраз. `HistoryManager` расширяем
slice фраз (отдельный файл, отдельный `RwLock`, та же off-thread persistence).

**Кто пишет фразы (ровно ОДНА точка записи):** `speak_text_internal` — после успешного
синтеза/`enqueue` вызывает `record_phrase(text)` в `HistoryManager`. In-memory журнал
`playback.rs::phrase_history` **удаляется**; `enqueue`/`on_playback_finished` больше не пишут
историю.

**Кто читает `recent` для плана 74:** `PlaybackManager::get_state()` должен получать «5
последних» из `HistoryManager` (`get_phrases(None, 5)`). Так как у `PlaybackManager` нет
ссылки на `HistoryManager`, прокидываем `Arc<HistoryManager>` в `PlaybackManager::new`
(setup.rs:88) — он уже создаётся после `HistoryManager` в lib.rs.

**`replay_from_cache(id)`:** сейчас ищет текст по id в `phrase_history`. После удаления
in-memory журнала текст для replay берётся из аудио-кеша/текущей фразы (`s.current` /
`queue`), НЕ из истории. См. раздел «Рефактор playback.rs» ниже — это **критичная** точка,
DeepSeek обязан сохранить работоспособность replay.

## Что сделать

### Backend (Rust)
1. **Единое хранилище фраз** — расширь `HistoryManager` (`history.rs`):
   - тип `PhraseEntry { id: String, text: String, count: u32, last_used: i64 }` (serde,
     `Serialize`/`Deserialize`/`Clone`/`Debug`);
   - поле `phrase_path: PathBuf` + `phrases: RwLock<Vec<PhraseEntry>>` (новый lock);
   - `new` принимает **третий** аргумент `phrase_path: PathBuf`; загружает `phrase_history.json`
     (`unwrap_or_default()`, без паник);
   - `history_paths()` возвращает **три** пути: `input_history.json`, `ngrams.json`,
     `phrase_history.json` (обновить сигнатуру + вызов в lib.rs:232);
   - `spawn_save` — расширить, чтобы персистить и `phrase_history.json` (или отдельная
     `spawn_save_phrases`); снимок под локом, drop lock, off-thread write;
   - методы:
     - `record_phrase(&self, text: &str)` — **дедупликация**: если фраза (по `text`,
       case-insensitive trim) уже есть → `count += 1`, `last_used = now`; иначе вставить
       новый с уникальным `id` (uuid v4) и `count=1`. Лимит N (кольцевой буфер по
       `last_used`, N=200). Вытеснение — самая старая по `last_used`. После мутации — save.
     - `get_phrases(&self, filter: Option<&str>, limit: usize) -> Vec<PhraseEntry>` —
       фильтр по подстроке (case-insensitive, по `text`), сортировка `last_used` desc,
       затем truncate `limit`. **Этот метод — источник и «5 последних» (plan 74), и полного
       списка (plan 75).**
     - `delete_phrase(&self, id: &str)` — удалить по id, save.
     - `clear_phrases(&self)` — очистить slice, save.
   - Только `parking_lot`, `anyhow`/`Result` где уже есть, uuid, chrono; **без `.expect()`/
     паник** в методах журнала (как уже в `record_text`/`clear`).

2. **Команды** (`commands/history.rs`), возвращают `Result<T, String>`, регистрация в
   `lib.rs::invoke_handler` (после `get_phrase_completion`, ~277):
   - `get_phrase_history(filter: Option<String>, limit: usize) -> Vec<PhraseEntry>` (DTO =
     сам `PhraseEntry`, он уже `Serialize`);
   - `delete_phrase_history(id: String)`;
   - `clear_phrase_history()`.
   - `record_phrase` — **НЕ команда**; вызывается из бэкенда (п.3).

3. **Триггер записи (единственная точка):** в `speak_text_internal` (commands/mod.rs), после
   успешного `pb.enqueue(...)` (строка ~252, внутри блока `if let Some(pb)`), вызвать
   `record_phrase` через `HistoryManager`. Доступ: `state` уже есть; `HistoryState`
   управляется в `Builder` — получить `HistoryManager` через `app_handle.state::<HistoryState>()`
   ИЛИ (чище) прокинуть `Arc<HistoryManager>` в `AppState` (state.rs), как сделано для
   `playback_manager`. DeepSeek выберет вариант, соответствующий существующим паттернам
   AppState, и явно опишет выбор в комментарии. **Важно:** фраза пишется ровно один раз на
   отправку — не дублировать в `enqueue`/`on_playback_finished`.

4. **Рефактор `playback.rs` (удалить дубль-журнал):**
   - удалить `PhraseEntry` (переехал в history.rs), поле `Shared.phrase_history`,
     `PHRASE_HISTORY_SIZE`, метод `add_history`;
   - `PlaybackManager::new` принимает доп. `Arc<HistoryManager>` (хранить в поле);
   - `enqueue`/`on_playback_finished` — **не пишут** историю (вызовы `add_history` убрать);
   - `get_state()` — `recent` заполнять из `history.get_phrases(None, 5)` (read-lock history),
     приводя `history::PhraseEntry` к тому, что ожидает `PlaybackStateDto.recent`. Если типы
     расходятся (`count`/`last_used` vs `timestamp`) — **привести `PlaybackStateDto.recent`
     и фронтенд к единому `PhraseEntry`** из history.rs (см. ниже Frontend п.0);
   - `replay_from_cache(id)` — **сохранить работоспособность**: текст для replay брать НЕ из
     истории, а из аудио-кеша + `s.current`/`queue` (там есть `QueuedPhrase.text`). Если по id
     текст не найден в кеше/текущем — вернуть без действия (как сейчас). Это поведение надо
     явно проверить end-to-end (см. критерии).

### Frontend (Vue/TS)
0. **Единый тип фразы:** зафиксировать TS-тип `PhraseEntry { id, text, count, last_used }`
   (например в `src/types/` или рядом с composables), соответствующий бэкенд-DTO. Убрать
   старый `{ id, text, timestamp }`, если он где-то описан.
1. **Composable `src/composables/usePhraseHistory.ts`** (по образцу `useInputHistory.ts`):
   `list(filter: string, limit: number)` (с debounce), `remove(id)`, `clear()`.
2. **Компонент `src/components/PhraseHistoryList.vue`** (подключается внутри `InputPanel.vue`
   рядом с `TtsEditor`):
   - размещение: свёрнут/развёрнут по кнопке (иконка) или хоткею;
   - текстовый фильтр по подстроке (debounce, через composable);
   - список: превью текста (обрезка), `count` × N, относительное `last_used`; клик по строке →
     эмит `select(text)`;
   - опционально: удаление одной фразы (×), «очистить всё» (с подтверждением).
3. **Загрузка в редактор:** `InputPanel` слушает `select` → вызывает метод `TtsEditor`
   (expose) для замены документа. Если в редакторе уже есть **отличный** текст — `confirm()`
     перед затиранием.
4. Стили — только CSS-переменные `src/styles/variables.css` (dark/light), без хардкода цветов.

## Поведение/ограничения
- Цель — **достать фразу и поправить**, а не мгновенный повтор (повтор — план 74).
- Дедупликация: одинаковые фразы схлопываются (`count` + `last_used`).
- Фильтр/сортировка/лимит — на стороне Rust (не тянуть весь журнал на фронт).
- История persistent (переживает перезапуск).
- TypeScript строгий (`<script setup lang="ts">`, без `any`).
- Не ломать очередь/паузу/replay редактора и существующие события.

## Критерии готовности
- Отправленная фраза появляется в списке (с дедупликацией: повтор → `count++`, `last_used`).
- Поиск по подстроке работает (Rust-сторона).
- Клик → загрузка в редактор (с `confirm` при затирании отличного текста).
- История переживает перезапуск приложения.
- Список сворачивается; не мешает вводу.
- `recent` в `get_playback_state` берётся из `get_phrases(None, 5)` (план 74 — view из
  единого хранилища).
- **replay (`replay_phrase`) по-прежнему работает** — текст берётся из кеша/текущей фразы, а
  не из удалённого in-memory журнала.
- **Единственная точка записи фраз** — `speak_text_internal` (в `enqueue`/`on_playback_finished`
  записи нет).
- `npx vue-tsc --noEmit` и `cargo check` (после `touch` изменённых .rs) — 0 ошибок, 0 warnings.

## Риски / что проверить ревьюеру
- Не осталось ли «двух журналов»: in-memory `phrase_history` в playback.rs должен быть
  удалён полностью.
- `replay_from_cache` после рефактора реально находит текст (трассировка пути кода, не чек-бокс).
- `record_phrase` не вызывается дважды на одну отправку.
- `PlaybackStateDto.recent` и фронтенд-тип приведены к единому `PhraseEntry` (расхождение
  `timestamp` vs `last_used` — типичная дыра).
