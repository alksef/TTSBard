# ROADMAP-002 — Локальная история ввода и автодополнение

**Дата:** 2026-06-28
**Статус:** research / ✅ РЕШЕНО
**Решения:** Persistent (между запусками) · хранение в **Tauri-стор** (Rust, `%APPDATA%\ttsbard\`)
· **Trie** (взвешенный) · поиск **по подстроке**
**Связано:** `01-monaco-vs-codemirror-editor-research.md`, `03-text-completion-without-ai.md`

---

## Цель

Сохранять **локально** (без сети) слова и фразы, которые пользователь вводит повторно, и
предлагать их в автодополнении при вводе — как в IDE (по префиксу / подстроке), с приоритетом
самых частых.

> Это **локальный автокомплит слов** — отдельная фича от AI-продолжения фраз
> (`PROBLEMS.md:57,74`, Anthropic Claude API) и от не-AI продолжения предложений
> (`03-text-completion-without-ai.md`).

---

## Где хранить

- **`localStorage`** — простейший вариант, синхронно, доступно из WebView.
- **Tauri-стор (Rust)** — через существующий механизм конфига/хранилища проекта,
  если нужно сохранять между запусками надёжно и не упираться в лимиты localStorage.
- Модель данных: массив/словарь `{ word: string, count: number, lastUsed: timestamp }`.

> Учесть: в `PROBLEMS.md:86` заявлено, что «история фраз — только в сессии (не сохраняется
> между запусками)». Эта фича **расширяет** то поведение —persistent личный словарь.
> Решить с пользователем: сохранять между запусками или нет.

---

## Алгоритм автодополнения слов

**Основа — Trie (префиксное дерево) или взвешенный Trie:**
- Вставка слова по символам в дерево; на узле храним `count`/частоту.
- Поиск: по введённому префиксу обходим поддерево, собираем кандидатов, сортируем по
  `count desc` → первыми идут самые частые.
- Trie эффективнее хеш-таблиц по префиксному поиску, нет коллизий, легко добавить fuzzy
  (через Levenshtein) если понадобится.

Альтернатива попроще: массив слов + `filter(w => w.startsWith(prefix))` — для небольшого
личного словаря этого достаточно и проще в поддержке.

**Поведение «как в IDE»** (а не как браузерный `datalist`):
- Искать не только по префиксу, но и по **подстроке** (`indexOf`) — IDE предлагает слова,
  содержащие введённое.
- Debounce на `input` (чтобы не дёргать поиск на каждый символ).
- Навигация клавишами (↑/↓, Tab/Enter — принять, Esc — закрыть).
- ARIA-атрибуты для доступности.

---

## Что не подходит
- Браузерный `<datalist>` — работает **только с `<input type="text">`**, с `<textarea>`/редактором
  не сочетается. Нужен свой выпадающий попап.
- `autocomplete="on/off"` у `<textarea>` — управляется браузером, не годится для личного словаря.

---

## Библиотеки
- **autoComplete.js** (Tarek Raafat) — vanilla, без зависимостей, встроенная поддержка
  истории/частых слов, WAI-ARIA, debouncing. Ближе всего к задаче как готовое решение.
- Если выбран CodeMirror 6 (см. `01-...`) — нативное расширение `@codemirror/autocomplete`
  + свой источник данных (Trie/массив из localStorage). Это даст единый каркас автокомплита
  внутри редактора без отдельной библиотеки.

---

## Связь со словарём орфографии (русский)
- Если параллельно вешаем **`hunspell-spellchecker` + `dictionary-ru`/`dictionary-en`**
  (оффлайн, движок Hunspell как в Firefox/LibreOffice) — это даёт проверку и `suggest()`.
- История ввода и словарь орфографии — **разные источники**, но могут объединяться в одном
  autocomplete-попапе: сначала частые слова из истории, потом исправления из hunspell.

---

## Открытые вопросы
1. ~~Persistent или сессия?~~ → **Persistent** (между запусками). Расширяет `PROBLEMS.md:86`.
2. ~~Где хранить~~ → **Tauri-стор** (Rust): файл `%APPDATA%\ttsbard\input_history.json`,
   через новую serde-структуру + `RwLock`-кэш + `#[tauri::command]` (по образцу
   `save_replacements` / `SettingsManager`).
3. ~~Trie vs массив~~ → **взвешенный Trie** (по `count`), кандидаты сортируются по частоте.
4. ~~Префикс или подстрока~~ → **по подстроке** (`indexOf`), как в IDE.

## Точки интеграции (Tauri-стор)
- Файл: `%APPDATA%\ttsbard\input_history.json` (рядом с `settings.json`).
- Регистрация команд: `src-tauri/src/lib.rs` `invoke_handler` (строка ~238).
- Образец команды с сохранением файла: `commands/preprocessor.rs::save_replacements`.
- State/кэш: новый `Arc<RwLock<InputHistory>>` в `AppState` (`state.rs:68`) или отдельный
  `HistoryManager` через `.manage()` (по образцу `SettingsManager`).
- Ошибки: `AppError` (`error.rs`), в команду — `Result<T, String>`.

---

## Источники
- [autoComplete.js](https://tarekraafat.github.io/autoComplete.js/)
- [Autocomplete using Trie (GeeksforGeeks)](https://www.geeksforgeeks.org/dsa/auto-complete-feature-using-trie/)
- [Autocomplete feature using Trie (dev.to)](https://dev.to/c6z3h/autocomplete-feature-using-trie-data-structure-313g)
- [Автокомплит с нуля на vanilla JS](https://dev.to/alexpechkarev/how-to-build-an-autocomplete-component-from-scratch-in-vanilla-js-45g0)
- [wooorm/dictionaries (dictionary-ru / dictionary-en)](https://github.com/wooorm/dictionaries)
- [hunspell-spellchecker (jsDelivr)](https://www.jsdelivr.com/package/npm/hunspell-spellchecker)
