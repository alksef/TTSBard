# Plan 81: Офлайн-проверка орфографии (Stage 08 — spellbook + Hunspell)

**Дата:** 2026-06-30
**Статус:** draft (для реализации через DeepSeek по WORKFLOW)
**Связано:** stage `docs/stage/08-offline-spellcheck-hunspell-codemirror.md`,
spike 80 (ПРОЙДЕН — отчёт ниже), план 78 (общий spell-linter-слой — фронт-потребитель),
арх-ревью `docs/reviews/review-019-2026-06-30.md`.

## Контекст
Spike 80 **ПРОЙДЕН**: `spellbook v0.4.2` отлично работает на русском (`suggest()` — 4/4 точных
исправлений, 5µs/слово, 3.5MB словарь, без паников). Fallback (nuspell-wasm) **не нужен**.
Архитектура = подход A (нативный Rust на бэкенде). Фронт-интеграция — через общий spell-linter-
слой (план 78): этот план реализует **бэкенд-источник** `spellcheck`, который composable
`useSpellcheck` уже вызывает при `source = 'offline'`.

## Spike 80 — подтверждённые факты (использовать в плане)
- **Крейт:** `spellbook = "0.4"` (точнее — последняя 0.4.x).
- **API:**
  ```rust
  let aff = std::fs::read_to_string("...ru.aff")?;
  let dic = std::fs::read_to_string("...ru.dic")?;
  let dict = spellbook::Dictionary::new(&aff, &dic)?;  // Result<_, ParseDictionaryError>
  dict.check("привет");   // bool
  let mut sugg = Vec::new();
  dict.suggest("првиет", &mut sugg);  // заполняет Vec<String>
  ```
- **Словари:** `ru.aff` (70 KB) + `ru.dic` (3.4 MB), источник — wooorm/dictionaries
  (Hunspell UTF-8). В бандл: ~3.5 MB.
- **Производительность:** загрузка ~437ms (разовая при старте), проверка ~5µs/слово.

## Цели
1. Бэкенд `SpellcheckManager` (по образцу `HistoryManager`): грузит словарь один раз в
   `Arc<RwLock<Dictionary>>` при старте.
2. Tauri-команда `spellcheck(words: Vec<String>) -> Vec<SpellResult>` (тот же тип, что в
   плане 78).
3. Словари в бандле (`tauri.conf.json bundle.resources`).
4. Кэш проверенных слов в памяти (не дёргать движок повторно).
5. Настройка уже есть из плана 78 (`spellcheck_enabled`, `spellcheck_source`).

## Точные точки интеграции (из research + spike)
- `src-tauri/src/state.rs:136-138` — `history_manager` в `AppState` (образец поля).
- `src-tauri/src/setup.rs:234-236` — инициализация `history_manager` (образец).
- `src-tauri/src/history.rs` — `HistoryManager` (образец менеджера: `Arc<RwLock<...>>`,
  `spawn_save`-паттерн НЕ нужен — словарь read-only, не персистим).
- `src-tauri/src/lib.rs:248-431` — `invoke_handler` (зарегистрировать `spellcheck`).
- `src-tauri/src/commands/` — образец команд (модуль + `mod.rs`).
- `src-tauri/tauri.conf.json:56-66` — секция `bundle` (НЕТ `resources` → добавить).
- Фронт: `useSpellcheck.ts` (план 78) уже вызывает `invoke('spellcheck', { words })` при
  `source='offline'`. Этот план делает так, чтобы команда **существовала и работала**.

## Архитектура

### 1. Зависимость
`cargo add spellbook` (в `src-tauri/Cargo.toml`).

### 2. Словари в бандле
- Файлы: `src-tauri/resources/dict/ru.aff`, `src-tauri/resources/dict/ru.dic`
  (из spike — wooorm/dictionaries, UTF-8).
- `tauri.conf.json` → `bundle.resources`: `"resources": ["resources/dict/*"]`.
- Путь к ресурсу в рантайме: `app.path().resource_dir()` (Tauri API) →
  `<resource_dir>/resources/dict/ru.{aff,dic}`. См. как Tauri резолвит resources.

### 3. Менеджер `src-tauri/src/spellcheck.rs` (НОВЫЙ, по образцу HistoryManager)
```rust
use anyhow::{Context, Result};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

pub struct SpellcheckManager {
    dict: RwLock<Option<spellbook::Dictionary>>,
    // Кэш: слово → (correct, suggestions). Не дёргать движок повторно.
    cache: RwLock<HashMap<String, SpellResult>>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SpellResult {
    pub word: String,
    pub correct: bool,
    pub suggestions: Vec<String>,
}

impl SpellcheckManager {
    /// Грузит словарь один раз при старте. Ошибка загрузки — НЕ падаем приложение:
    /// dict остаётся None, spellcheck возвращает всем correct=true (фича молча выключена).
    pub fn new(aff_path: PathBuf, dic_path: PathBuf) -> Self {
        let dict = (|| -> Result<spellbook::Dictionary> {
            let aff = std::fs::read_to_string(&aff_path)
                .context("read ru.aff")?;
            let dic = std::fs::read_to_string(&dic_path)
                .context("read ru.dic")?;
            spellbook::Dictionary::new(&aff, &dic).context("parse hunspell dict")
        })();
        if let Err(e) = &dict {
            eprintln!("[spellcheck] dictionary load failed: {e:?} (spellcheck disabled)");
        }
        Self {
            dict: RwLock::new(dict.ok()),
            cache: RwLock::new(HashMap::new()),
        }
    }

    pub fn check_words(&self, words: &[String]) -> Vec<SpellResult> {
        let dict_guard = self.dict.read();
        let Some(dict) = dict_guard.as_ref() else {
            // Словарь не загрузился — все слова считаем корректными (фича выключена).
            return words.iter().map(|w| SpellResult {
                word: w.clone(), correct: true, suggestions: vec![],
            }).collect();
        };
        drop(dict_guard); // borrow issue — см. реализацию, возможно без drop

        let mut cache = self.cache.write();
        let mut results = Vec::with_capacity(words.len());
        for w in words {
            if let Some(r) = cache.get(w) {
                results.push(r.clone());
                continue;
            }
            let correct = dict.check(w);
            let mut suggestions = Vec::new();
            if !correct {
                dict.suggest(w, &mut suggestions);
            }
            let r = SpellResult { word: w.clone(), correct, suggestions };
            cache.insert(w.clone(), r.clone());
            results.push(r);
        }
        results
    }
}
```
> **Внимание к borrow-checker:** `dict_guard` (read) и запись в `cache` (write) — разные
> RwLock, но `dict` заимствуется из `dict_guard`. DeepSeek: реализовать так, чтобы не держать
> read-lock словаря во время write-lock кэша (например: проверить слово под read-lock,
> собрать результат, отпустить, потом писать в кэш). Точная реализация — на усмотрение с
> соблюдением `parking_lot::RwLock` паттерна как в `history.rs`.

### 4. Команда `src-tauri/src/commands/spellcheck.rs` (НОВЫЙ)
```rust
use crate::spellcheck::{SpellcheckManager, SpellResult};
use std::sync::Arc;
use tauri::State;

pub struct SpellcheckState(pub Arc<SpellcheckManager>);

#[tauri::command]
pub fn spellcheck(
    words: Vec<String>,
    state: State<'_, SpellcheckState>,
) -> Result<Vec<SpellResult>, String> {
    Ok(state.0.check_words(&words))
}
```
Регистрация: `commands/mod.rs` (pub mod spellcheck), `lib.rs invoke_handler` (+ spellcheck),
`state.rs` (+ `spellcheck_manager` поле), `setup.rs` (инициализация с путями к resource_dir).

### 5. Настройка
Уже из плана 78: `spellcheck_enabled` + `spellcheck_source` (default Offline). Здесь — без
новых настроек (только если нужен `lang` — отложим, этап 2).

## Риски и решения
1. **borrow-checker** dict read + cache write — реализовать аккуратно (см. выше).
2. **Вес бандла +3.5MB** — принято (spike подтвердил приемлемость).
3. **Ложные срабатывания** (имена, `%username`, `\replace`) — этап 2 / отдельная задача:
   исключать токены после препроцессорных подстановок или по паттерну. В этот план НЕ входит.
4. **Resource path resolution** — Tauri `resource_dir()`; проверить в setup, что файлы
  существуют (если нет — лог + фича молча выключена, не краш).
5. **`spellbook` alpha** — spike подтвердил стабильность 0.4.2 на русском. Зафиксировать
  версию в Cargo.toml (`=0.4.2` или диапазон 0.4), чтобы избежать сюрпризов.

## Критерии готовности
1. `spellbook` в Cargo.toml, словари в `resources/dict/`, `bundle.resources` прописан.
2. `SpellcheckManager` грузит словарь при старте (ошибка — не краш, фича выключена).
3. Команда `spellcheck(words) -> Vec<SpellResult>` с кэшем.
4. Фронт (план 78) при `source='offline'` получает реальные результаты (подсветка ошибок +
   варианты исправления в CodeMirror).
5. `cargo check` + `npx vue-tsc --noEmit` — 0 ошибок, 0 warnings.

## Объём
Средний, **бэкенд-Rust** + конфиг бандла. По WORKFLOW — через DeepSeek.
Зависит от плана 78 (фронт-слой уже вызывает команду). Может идти после 78.

## После реализации
Code-review + арх-ревью через сабагентов. Проверить especially: borrow-checker, resource path
на Windows, кэш (не течёт ли память), что при отсутствии словаря нет краша.
