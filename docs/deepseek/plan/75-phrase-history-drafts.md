# DeepSeek Plan 75: История фраз как черновики (извлечение в редактор)

> **Для DeepSeek:** пиши реализацию сам. Здесь — инструкции (файлы/типы/сигнатуры/поведение),
> не готовый код. Общий план — `docs/plans/75-...`, контекст — `docs/stage/05-...`.
> Зависит от планов 71 (редактор) и 72 (`HistoryManager`, persistence-паттерны).

## Контекст кода
- `src-tauri/src/history.rs` — `HistoryManager`: Trie слов + n-граммы, persistence в
  `input_history.json` / `ngrams.json` (через `parking_lot::RwLock`, off-thread save).
- `src-tauri/src/commands/history.rs` — команды `get_history_suggestions`/`record_history`/
  `clear_history` (регистрация в `lib.rs::invoke_handler`).
- Отправка фразы: `TextSentToTts` эмитится в `commands/mod.rs::speak_text_internal` (~176) —
  триггер для записи фразы в журнал.
- Frontend: `InputPanel.vue` содержит `TtsEditor.vue`; composables `useInputHistory.ts` —
  образец обёртки над invoke (debounce).
- Архитектура: sidebar (`App.vue:124`) переключает панели; InputPanel — текущая панель ввода.

> **ЕДИНОЕ хранилище фраз (важно):** `phrase_history.json` — **единственный** журнал фраз в
> приложении. План 74 («5 последних» для мгновенного повтора) и план 75 (полный список с
> поиском для извлечения в редактор) — это **разные views из одного хранилища**, НЕ два
> журнала. Если в реализации плана 74 уже создан отдельный slice/journal «5 недавних» —
> **объедини его** в это хранилище (не оставляй два). Метод `get_phrases(filter, limit)` —
> общий: план 74 зовёт его с `limit=5` без фильтра, план 75 — с фильтром.

## Что сделать

### Backend (Rust)
1. **Единое хранилище фраз** — расширь `HistoryManager` (`history.rs`) slice целых фраз:
   - модель `PhraseEntry { id, text, count, last_used }`; persistent `phrase_history.json`;
   - методы: `record_phrase(text)` — с **дедупликацией** (если фраза есть → incr count +
     обновить last_used; иначе вставить), `get_phrases(filter: Option<String>, limit)` —
     фильтр по подстроке, сортировка по `last_used` desc (этот же метод — источник «5
     последних» для плана 74 при `limit=5` без фильтра), `delete_phrase(id)`, `clear_phrases()`;
   - запись на диск off-thread (как слова/n-граммы), лимит N (кольцевой буфер, напр. 100).
   - Только `parking_lot`, `AppError`/`Result<T,String>`, **без `.expect()`/паник** в методах.
   - **Если план 74 уже завёл свой journal/метод «5 недавних»** — рефактори на использование
     этого единого хранилища (`get_phrases(limit=5)`).
2. **Команды** (`commands/history.rs` или новый `commands/phrase_history.rs`, регистрация в
   `invoke_handler`), возвращают `Result<T, String>`:
   - `get_phrase_history(filter: Option<String>, limit: usize) -> Vec<PhraseEntryDto>`;
   - `delete_phrase(id)`;
   - `clear_phrase_history()`.
   - `record_phrase` — НЕ отдельная команда; вызывается из бэкенда при отправке (п.3).
3. **Триггер записи:** в `speak_text_internal` (там же, где `TextSentToTts`) вызови
   `record_phrase(text)` в единое хранилище. Убедись, что нет **второй** точки записи фраз
   (если план 74 писал свой journal — перенеси сюда).

### Frontend (Vue/TS)
1. **Composable `src/composables/usePhraseHistory.ts`** (по образцу `useInputHistory.ts`):
   `list(filter, limit)` (с debounce), `delete(id)`, `clear()`.
2. **Компонент `src/components/PhraseHistoryList.vue`** (или внутри `InputPanel.vue`):
   - размещение: внутри InputPanel рядом с `TtsEditor` (боковая панель или выпадение);
   - свёрнут/развёрнут по кнопке (иконка) или хоткею;
   - текстовый фильтр по подстроке (debounce);
   - список: превью текста + timestamp/счётчик; клик → загрузить фразу в редактор;
   - опционально: удаление одной фразы (×), «очистить всё».
3. **Загрузка в редактор:** клик по фразе → эмит в InputPanel → `TtsEditor` заменяет документ.
   - Если в редакторе уже есть текст (отличный от выбранной фразы) — **подтверждение**
     (confirm), не затирать молча.
4. Стили — только CSS-переменные `src/styles/variables.css` (dark/light), без хардкода цветов.

## Поведение/ограничения
- Цель — **достать фразу и поправить**, а не мгновенный повтор (повтор — план 74).
- Дедупликация: одинаковые фразы схлопываются (count + last_used), не дублируются.
- Фильтр/сортировка/лимит — на стороне Rust (не тянуть весь журнал на фронт).
- История persistent (переживает перезапуск).
- TypeScript строгий (`<script setup lang="ts">`, без `any`).
- Не ломать quick-режим редактора и существующие события.

## Критерии готовности
- Отправленная фраза появляется в списке (с дедупликацией).
- Поиск по подстроке работает.
- Клик → загрузка в редактор (с подтверждением при затирании).
- История переживает перезапуск.
- Список сворачивается; не мешает вводу.
- `npx vue-tsc --noEmit` и `cargo check` (после touch изменённых .rs) — 0 ошибок, 0 warnings.
