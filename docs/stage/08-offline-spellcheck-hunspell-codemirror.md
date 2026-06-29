# Stage: Офлайн-проверка орфографии (CodeMirror + Hunspell, без сети)

**Дата:** 2026-06-30
**Статус:** research / оценка (план — отдельным файлом)
**Решение (предварительное):** офлайн-движок **`spellbook`** (нативный Rust-порт Nuspell, от
helix-editor) + словари Hunspell `ru.aff`/`ru.dic` (бандл) на бэкенде Tauri; подсветка ошибок и
варианты исправления во фронте через **`@codemirror/lint`**.
**Возможно ли:** **ДА.** Подсветка + варианты замены — штатная фича CodeMirror linter.
**Связано:** `01-monaco-vs-codemirror-editor-research.md` (CM6 «хорошо ложится на hunspell»),
`02-local-history-autocomplete.md:73` (идея hunspell + словарь), `07-editor-menu-ai-history-spellcheck.md`
(онлайн-вариант — здесь противоположный, офлайн)

## Цель (запрос пользователя)
- **Офлайн** проверка орфографии (без сети, без отправки текста наружу → приватность).
- CodeMirror должен **подсвечивать** слова с ошибками и **давать варианты исправления**.
- Оценка возможности + варианты решения.

## Возможно ли это? — ДА
- **Бэкенд:** проверка/`suggest` офлайн в Rust — решено через `spellbook` (нативный порт Nuspell,
  читает `.aff`/`.dic` Hunspell — тот же движок, что в Firefox/LibreOffice). Поддерживает русский.
- **Фронт:** CodeMirror 6 имеет первоклассный `@codemirror/lint` — `linter()` возвращает массив
  `Diagnostic { from, to, message, severity, actions[] }`, где `actions` — это и есть «варианты
  исправления» (quick-fix, клик = применить замену). Подчёркивание + меню замен — из коробки.
- Сеть/внешний API **не нужен**. Приватность сохранена (текст не покидает приложение).

> Ранее (stage `01:44`, `02:73-77`) упоминался npm-пакет `hunspell-spellchecker` (JS-порт) и
> объединение со словарём в autocomplete-попапе. Здесь выбираем **Rust-сторону** (`spellbook`)
> вместо JS-пакета — нативнее, быстрее, не тащит WASM/FFI в WebView, единый движок с бэкендом.

---

## Контекст кода (точки интеграции)
- `TtsEditor.vue` (CodeMirror 6) — расширения собираются в `createState()` (строки 275-300).
  Linter добавляется как ещё один extension: `linter(spellSource)`.
- `@codemirror/lint` — **пока НЕ в зависимостях** (`package.json:13-18` есть view/state/commands/
  language/autocomplete/search, но нет lint). Нужно `npm i @codemirror/lint`.
- Команды регистрируются в `src-tauri/src/lib.rs` `invoke_handler`.
- State/кэш — по образцу `HistoryManager` / `SettingsManager` (`src-tauri/src/state.rs`).
- Ресурсы (словари): `src-tauri/resources/` + `tauri.conf.json` `bundle.resources` (см. как уже
  бандлятся ассеты).
- Тема/приватность: настройка вкл/выкл → новое поле в `settings.json` ⇒ **обязательно
  `#[serde(default)]`** на сущности (иначе те же грабли миграции, что с `playback_pause`).

---

## Варианты движка (что рассматривали)

| Вариант | Где | Плюсы | Минусы / вердикт |
|---|---|---|---|
| **A. `spellbook` (Rust, Nuspell)** ⭐ | бэкенд Tauri | нативный Rust, без FFI/WASM, быстрый, `check`+`suggest`, Hunspell-совместимые словари | alpha/молодой крейт; проверить API-стабильность `suggest` |
| **B. `nuspell` (C++) через FFI** | бэкенд | «взрослый» движок | сложность сборки C++ под Windows/Tauri; риск крашей — **отвергаем** |
| **C. `nuspell-wasm`** | фронт (WASM) | без бэкенда | медленнее из-за бриджа; надо грузить WASM+словари в WebView — **отвергаем** |
| **D. `hunspell-spellchecker` (npm, JS-порт)** | фронт | простота (упомянут в `02:74`) | медленнее, JS-порт менее точный; отдельный движок от бэкенда — **отвергаем** в пользу A |
| **E. `zspell` (Rust)** | бэкенд | ещё один Rust-порт | `suggest` API не стабилен — **отвергаем** |

**Рекомендация: A (`spellbook` + Hunspell-словари `ru`), бэкенд.**

> Запасной путь, если `spellbook` окажется нестабилен на русском `suggest`: переключиться на
> C (`nuspell-wasm`) во фронте — но это шаг назад по производительности. Сначала A.

---

## Архитектура (решение)

### Бэкенд (Rust, Tauri)
1. **Зависимости:** `cargo add spellbook`.
2. **Словари:** `ru.aff` + `ru.dic` (пакет `hunspell-ru`) → `src-tauri/resources/dict/ru.{aff,dic}`,
   прописать в `tauri.conf.json` `bundle.resources`.
3. **Менеджер `SpellcheckManager`** (по образцу `HistoryManager`):
   - Грузит словарь один раз в `Arc<RwLock<Dictionary>>` при старте.
   - Хранит набор слов-исключений (пользовательский словарь) — optional, этап 2.
4. **Команды** `#[tauri::command]`:
   - `spellcheck(words: Vec<String>) -> Vec<SpellResult { word, correct, suggestions: Vec<String> }>`
     — проверяет **токены**, возвращает для неверных варианты. Бэкенд не парсит текст на слова —
     фронт разбивает на токены (`/[\wа-яё-]+/gi`) и шлёт массив (меньше работы, меньше трафика).
   - `spellcheck_add_word(word)` / `spellcheck_user_dictionary()` — пользовательский словарь (этап 2).
5. **Кэш проверенных слов** в памяти (`HashMap<String, SpellResult>`) — не дёргать движок на одно
   слово повторно (важно при дебаунсе).
6. **Настройка:** `SettingsManager` → `spelling: { enabled: bool, lang: String }`, с `#[serde(default)]`.

### Фронт (CodeMirror)
1. **`npm i @codemirror/lint`** + `@codemirror/language` (для токенизации через `DefaultBufferSpaces`/
   `Tree` или простой regex-сплиттер).
2. **Linter-расширение** в `TtsEditor.vue:createState()`:
   ```ts
   import { linter, type Diagnostic } from '@codemirror/lint'
   const spellLinter = linter(async (view): Promise<Diagnostic[]> => {
     const doc = view.state.doc.toString()
     const tokens = [...doc.matchAll(/[\wа-яёА-ЯЁ-]+/g)]   // слова с позициями
     const unknown = tokens.filter(t => !userDictKnown.has(t[0].toLowerCase()))
     const res = await invoke<SpellResult[]>('spellcheck', { words: unknown.map(t=>t[0]) })
     return res.filter(r => !r.correct).map(r => {
       const m = tokens.find(t => t[0] === r.word)!
       return {
         from: m.index!, to: m.index! + r.word.length,
         severity: 'warning', message: `«${r.word}» — нет в словаре`,
         actions: r.suggestions.slice(0, 5).map(s => ({
           name: s,
           apply: (v, from, to) => v.dispatch({ changes: { from, to, insert: s } }),
         })),
       }
     })
   }, { delay: 400 })   // дебаунс — не на каждое нажатие
   ```
3. **Quick-fix:** CodeMirror сам покажет лампочки/меню замен (bulb) при наведении/курсоре на
   подчёркнутом слове — `actions[]` = варианты исправления. Подчёркивание — волнистая линия (CSS
   linter-темы, переопределить через CSS-vars для light/dark).
4. **Тема:** стилизовать `.cm-diagnosticText`, `.cm-lintRange` через существующие CSS-vars
   (`--color-border-strong`, `--color-accent`) для light/dark.

---

## Риски / нюансы (для будущего плана)
1. **`spellbook` — молодой крейт (alpha).** Перед планом — **spike**: `cargo`-демка с `ru.aff`/`ru.dic`,
   проверить `check` и особенно `suggest` на русском. Если `suggest` слабый/багует → fallback на C.
2. **Словари — вес.** `ru.dic` ~1-3 МБ; бандл растёт. Учесть в размере инсталлятора. Можно
   догружать словарь лениво, но офлайн-цель подразумевает бандл.
3. **Токенизация рус+лат.** Regex `/[\wа-яёА-ЯЁ-]+/g` — `\w` не ловит кириллицу в JS без флага `u`,
   поэтому явно добавляем `а-яё`. Проверить в spike.
4. **Дебаунс linter.** `linter(fn, { delay: 400 })` — не запускать проверку на каждое нажатие
   (иначе дёргаем бэкенд/движок постоянно). + кэш слов.
5. **Приватность vs. AI-коррекция.** Орфография офлайн = безопасно; но не путать с `correct_text`
   (AI, онлайн). В меню (stage 07) — два разных пункта: «орфография (офлайн)» и «AI (онлайн)».
6. **Миграция настроек.** Новое поле `settings.json` → `#[serde(default)]` на сущности, иначе
  повторяются грабли `playback_pause` (panic на старых конфигах).
7. **Ложные срабатывания.** Имена, никнейменты (`%username`), замены (`\replace`) — не проверять
   как обычный текст; исключать токены после препроцессорных подстановок или игнорировать по
   паттерну.
8. **Конфликт с autocomplete (план 73).** Подсветка ошибок и попап автокомплита — не мешают
   (разные расширения), но visually не должны конфликтовать (цвета). Зафиксировать.

---

## Оценка трудозатрат

| Часть | Объём | Зависимости |
|---|---|---|
| Spike: `spellbook` + `ru.aff/dic`, проверка `check`/`suggest` | малый (но **блокирующий**) | новая зависимость |
| Бэкенд: `SpellcheckManager` + команда `spellcheck` + кэш | средний | spike ✓ |
| Словари в бандле (`resources` + `tauri.conf.json`) | малий | нет |
| Настройка `settings.json` (`enabled`, `lang`) + `#[serde(default)]` | малий | нет |
| Фронт: `@codemirror/lint` + linter-источник + токенизация + debounce | средний | бэкенд ✓ |
| Quick-fix (варианты замен) + тема (light/dark) | малий-средний | linter ✓ |
| Пользовательский словарь («добавить слово») | средний (этап 2) | база ✓ |

**Итого:** средняя задача, **двухслойная** (бэкенд Rust + фронт CodeMirror). Чуть больше, чем
меню-каркас (07), сравнимо с планом 74 по объёму. **Блокирующий риск — spike `spellbook`
(если движок не взлетит на русском, весь подход A меняется на C).**

## KEY_DECISIONS (предварительные)
- **Движок: `spellbook` (Rust-порт Nuspell) + Hunspell-словари `ru`**, бэкенд — НЕ JS/WASM/FFI.
- **Подсветка + варианты исправления: через `@codemirror/lint`** (Diagnostic + `actions[]`).
- **Офлайн = приватность:** текст не покидает приложение. Противоположность онлайн-варианту (07).
- **Токенизация во фронте, проверка на бэкенде** (фронт шлёт массив слов, бэкенд отвечает
  `correct`+`suggestions`).
- **Обязательно `#[serde(default)]`** на новой настройке (урок `playback_pause`).
- **Блокирующий spike** перед планом: проверить `spellbook` на русском `suggest`.

## Источники
- [spellbook (helix-editor, Rust-порт Nuspell)](https://github.com/helix-editor/spellbook)
- [docs.rs/spellbook](https://docs.rs/spellbook)
- [Nuspell (родитель движка)](https://nuspell.github.io)
- [hunspell-ru — русский словарь (Debian)](https://packages.debian.org/sid/hunspell-ru)
- [CodeMirror 6 lint — официальный пример (Diagnostic + actions)](https://codemirror.net/examples/lint/)
- [zspell (запасной Rust-порт)](https://lib.rs/crates/zspell)
- [nuspell-wasm (fallback, фронт)](https://www.npmjs.com/search?q=keywords%3A%22spell+checker%22)
