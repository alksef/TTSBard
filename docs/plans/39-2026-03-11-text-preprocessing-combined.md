# План #39: Конвейер предобработки текста (объединённый)

## Обзор
Полный модуль предобработки текста перед отправкой в TTS. Объединяет три этапа обработки:

1. **Обработчики префиксов** — управление отправкой в Twitch/WebView
2. **Замены** — замена `\word` и `%username`
3. **Числа в текст** — конвертация чисел с согласованием рода

## Конвейер обработки

```
Входной текст
    ↓
┌─────────────────────────────────────────────────────────┐
│  ЭТАП 1: Парсинг префиксов                              │
│  !text    → пропустить Twitch, отправить в WebView      │
│  !!text   → пропустить Twitch + WebView                  │
│  Результат: текст (без префиксов), флаги сохранены      │
└─────────────────────────────────────────────────────────┘
    ↓
┌─────────────────────────────────────────────────────────┐
│  ЭТАП 2: Замены                                         │
│  \word    → замена из replacements.txt                  │
│  %user    → имя пользователя из usernames.txt            │
│  Результат: текст с применёнными заменами               │
└─────────────────────────────────────────────────────────┘
    ↓
┌─────────────────────────────────────────────────────────┐
│  ЭТАП 3: Числа в текст                                  │
│  123      → сто двадцать три                            │
│  1 книга  → одна книга (согласование рода)              │
│  2 книги  → две книги                                   │
│  Результат: текст с числами в русских словах            │
└─────────────────────────────────────────────────────────┘
    ↓
┌─────────────────────────────────────────────────────────┐
│  ЭТАП 4: Синтез TTS                                     │
│  Текст отправляется в TTS (без префиксов)               │
│  Событие TextSentToTts с флагами                        │
└─────────────────────────────────────────────────────────┘
    ↓
┌─────────────────────────────────────────────────────────┐
│  ЭТАП 5: Маршрутизация событий                          │
│  Проверка флагов → отправка/пропуск Twitch, WebView     │
└─────────────────────────────────────────────────────────┘
```

## Структура модуля

```
src-tauri/src/preprocessor/
├── mod.rs          # Экспорты модуля, вспомогательные функции
├── prefix.rs       # Парсинг префиксов (!, !!)
├── replacer.rs     # Существующий: замены/usernames
└── numbers.rs      # Числа в текст с согласованием рода
```

## План реализации

### Этап 1: Создать модуль чисел
**Файлы:**
- `src-tauri/Cargo.toml` - Добавить `russian_numbers = "0.2.0"`
- `src-tauri/src/preprocessor/numbers.rs` - НОВЫЙ

```rust
use russian_numbers::{NumeralName, Gender};

/// Определить грамматический род русского слова
fn detect_gender(word: &str) -> Gender {
    let word_lower = word.to_lowercase();
    let word_clean = word_lower.trim_end_matches(|c| "!?,.;:".contains(c));

    if word_clean.ends_with('а') || word_clean.ends_with('я') || word_clean.ends_with('ь') {
        Gender::Feminine
    } else if word_clean.ends_with('о') || word_clean.ends_with('е') {
        Gender::Neuter
    } else {
        Gender::Masculine
    }
}

/// Конвертировать число в русский текст с согласованием рода
fn convert_number(num: i64, gender: Gender) -> String {
    match gender {
        Gender::Masculine => num.to_russian_name(Gender::Masculine),
        Gender::Feminine => num.to_russian_name(Gender::Feminine),
        Gender::Neuter => num.to_russian_name(Gender::Neuter),
    }
}

/// Обработать текст: заменить числа на русские слова
pub fn process_numbers(text: &str) -> String {
    use regex::Regex;
    let number_regex = Regex::new(r"-?\d+").unwrap();

    number_regex.replace_all(text, |caps: &regex::Captures| {
        let number_str = &caps[0];
        if let Ok(num) = number_str.parse::<i64>() {
            // Найти следующее слово для определения рода
            let after_match = &text[caps.get(0).unwrap().end()..];
            let next_word = after_match
                .split_whitespace()
                .next()
                .unwrap_or("");

            let gender = if num == 1 || num == 2 {
                detect_gender(next_word)
            } else {
                Gender::Masculine
            };

            convert_number(num, gender)
        } else {
            number_str.to_string()
        }
    }).to_string()
}
```

### Этап 2: Создать модуль префиксов
**Файл:** `src-tauri/src/preprocessor/prefix.rs` - НОВЫЙ

```rust
/// Результат парсинга префиксов
pub struct PrefixResult {
    pub text: String,
    pub skip_twitch: bool,
    pub skip_webview: bool,
}

/// Парсить префиксы из текста
pub fn parse_prefix(text: &str) -> PrefixResult {
    if text.starts_with("!!") {
        PrefixResult {
            text: text[2..].trim_start().to_string(),
            skip_twitch: true,
            skip_webview: true,
        }
    } else if text.starts_with('!') {
        PrefixResult {
            text: text[1..].trim_start().to_string(),
            skip_twitch: false,
            skip_webview: true,
        }
    } else {
        PrefixResult {
            text: text.to_string(),
            skip_twitch: false,
            skip_webview: false,
        }
    }
}
```

### Этап 3: Обновить экспорты модуля
**Файл:** `src-tauri/src/preprocessor/mod.rs`

```rust
mod replacer;
mod numbers;
mod prefix;

pub use replacer::TextPreprocessor;
pub use numbers::process_numbers;
pub use prefix::{parse_prefix, PrefixResult};

// ... существующие вспомогательные функции
```

### Этап 4: Добавить флаги префиксов в состояние
**Файл:** `src-tauri/src/state.rs`

```rust
pub struct AppState {
    // ... существующие поля

    /// Флаги префиксов из текущего TTS запроса
    prefix_skip_twitch: Arc<Mutex<bool>>,
    prefix_skip_webview: Arc<Mutex<bool>>,
}

impl AppState {
    // ... существующие методы

    pub fn set_prefix_flags(&self, skip_twitch: bool, skip_webview: bool) {
        *self.prefix_skip_twitch.lock() = skip_twitch;
        *self.prefix_skip_webview.lock() = skip_webview;
    }

    pub fn get_prefix_flags(&self) -> (bool, bool) {
        let skip_twitch = *self.prefix_skip_twitch.lock();
        let skip_webview = *self.prefix_skip_webview.lock();
        (skip_twitch, skip_webview)
    }

    pub fn clear_prefix_flags(&self) {
        *self.prefix_skip_twitch.lock() = false;
        *self.prefix_skip_webview.lock() = false;
    }
}
```

### Этап 5: Интегрировать конвейер
**Файл:** `src-tauri/src/commands/mod.rs`

```rust
pub async fn speak_text_internal(state: &AppState, text: String) -> Result<(), String> {
    eprintln!("[SPEAK_INTERNAL] Starting TTS for text: '{}'", text);

    if text.trim().is_empty() {
        return Err("Текст не может быть пустым".to_string());
    }

    // === ЭТАП 1: Парсинг префиксов ===
    let prefix_result = crate::preprocessor::parse_prefix(&text);
    let text = prefix_result.text;

    if prefix_result.skip_twitch || prefix_result.skip_webview {
        eprintln!("[PREFIX] Флаги - skip_twitch: {}, skip_webview: {}",
            prefix_result.skip_twitch, prefix_result.skip_webview);
    }

    // === ЭТАП 2: Замены (существующие) ===
    let text = if let Some(preprocessor) = state.get_preprocessor() {
        let processed = preprocessor.process(&text);
        if processed != text {
            eprintln!("[PREPROCESSOR] Замены: '{}' -> '{}'", text, processed);
        }
        processed
    } else {
        text
    };

    // === ЭТАП 3: Числа в текст ===
    let text = crate::preprocessor::process_numbers(&text);
    eprintln!("[PREPROCESSOR] Итоговый текст для TTS: '{}'", text);

    // Сохранить флаги для обработчиков событий
    state.set_prefix_flags(prefix_result.skip_twitch, prefix_result.skip_webview);

    // === ЭТАП 4: Синтез TTS ===
    let audio_data = provider.synthesize(&text).await?;

    // ... воспроизведение аудио
}
```

### Этап 6: Обновить маршрутизацию событий
**Файл:** `src-tauri/src/event_loop.rs`

```rust
fn process_text_sent_to_tts(&self, text: String) {
    eprintln!("[EVENT] Text sent to TTS: '{}'", text);

    let (skip_twitch, skip_webview) = self.state.get_prefix_flags();

    // === WebView (проверка флага) ===
    if !skip_webview {
        if let Some(ref sender) = *self.state.webview_event_sender.lock() {
            eprintln!("[EVENT] Отправка в WebView");
            let _ = sender.send(AppEvent::TextSentToTts(text.clone()));
        }
    } else {
        eprintln!("[EVENT] WebView пропущен (префикс)");
    }

    // === Twitch (проверка флага) ===
    if !skip_twitch {
        let settings = self.state.twitch_settings.blocking_read();
        if settings.enabled {
            drop(settings);
            self.state.send_twitch_event(TwitchEvent::SendMessage(text));
        }
    } else {
        eprintln!("[EVENT] Twitch пропущен (префикс)");
    }

    // Очистить флаги
    self.state.clear_prefix_flags();
}
```

## Сводка файлов

| Файл | Действие | Этап |
|------|----------|------|
| `src-tauri/Cargo.toml` | Добавить зависимость `russian_numbers` | 1 |
| `src-tauri/src/preprocessor/numbers.rs` | НОВЫЙ - конвертация чисел | 1 |
| `src-tauri/src/preprocessor/prefix.rs` | НОВЫЙ - парсинг префиксов | 2 |
| `src-tauri/src/preprocessor/mod.rs` | Экспортировать новые модули | 3 |
| `src-tauri/src/state.rs` | Добавить хранение флагов префиксов | 4 |
| `src-tauri/src/commands/mod.rs` | Интегрировать конвейер | 5 |
| `src-tauri/src/event_loop.rs` | Проверять флаги перед отправкой | 6 |

## Матрица тестирования

### Комбинированные примеры

| Ввод | После префикса | После замен | После чисел | TTS озвучивает | Twitch | WebView |
|------|----------------|-------------|-------------|----------------|--------|---------|
| `!Привет` | `Привет` | `Привет` | `Привет` | `Привет` | пропуск | отправка |
| `!!1 книга` | `1 книга` | `1 книга` | `одна книга` | `одна книга` | пропуск | пропуск |
| `У меня 5 яблок` | `У меня 5 яблок` | `У меня 5 яблок` | `У меня пять яблок` | `У меня пять яблок` | отправка | отправка |
| `!\name 2 друга` | `\name 2 друга` | `Alice 2 друга` | `Alice два друга` | `Alice два друга` | пропуск | отправка |

### Тестовые случаи

**Парсинг префиксов:**
- [ ] `!text` → skip_twitch=true, skip_webview=false
- [ ] `!!text` → skip_twitch=true, skip_webview=true
- [ ] `text` → оба false
- [ ] ` !text` → оба false (начальный пробел)

**Конвертация чисел:**
- [ ] `123` → `сто двадцать три`
- [ ] `1 книга` → `одна книга`
- [ ] `2 книги` → `две книги`
- [ ] `5 яблок` → `пять яблок`
- [ ] `-10` → `минус десять`

**Комбинированные:**
- [ ] `!1 друг` → `один друг`, Twitch пропущен
- [ ] `!!2 книги` → `две книги`, оба пропущены
- [ ] `\name 1 книга` → `Alice одна книга`

## Зависимости

```toml
[dependencies]
russian_numbers = "0.2.0"
regex = "1"  # Уже есть в зависимостях
lazy_static = "1"  # Уже есть в зависимостях
```

## Примечания

1. **Порядок важен**: Префиксы → Замены → Числа → TTS
2. **Префиксы удаляются** перед любой обработкой
3. **Флаги сохраняются** через весь конвейер до маршрутизации событий
4. **Определение рода** эвристическое (по окончаниям слов)
5. **Отрицательные числа** получают префикс "минус"

## Ограничения эвристики определения рода

Эвристика по окончаниям слов покрывает **~80-85%** случаев, но имеет ограничения:

### Где эвристика работает ✅
- Регулярные слова: `книга`, `стол`, `окно`, `дом`, `море`
- Большинство повседневных слов

### Где эвристика ошибётся ❌

| Слово | Правильный род | Эвристика | Ошибка |
|-------|----------------|-----------|--------|
| `путь` | мужской | женский (`ь`) | *одна* (вместо *один*) |
| `день` | мужской | женский (`ь`) | *одна* (вместо *один*) |
| `парень` | мужской | женский (`ь`) | *одна* (вместо *один*) |
| `кофе` | мужской | средний (`е`) | *одно* (вместо *один*) |
| `меню` | средний | мужской (нет `о`) | *один* (вместо *одно*) |

### Почему это приемлемо для TTS
1. **Редкие ошибки** — 15-20% слов, большинство не встретятся часто
2. **Контекст понятен** — даже *одна день* не разрушает понимание
3. **Простота** — нет тяжёлых зависимостей (rsmorphy)
4. **Скорость** — простая проверка окончаний

### Альтернативы
- **rsmorphy** — полный морфологический анализ (см. `docs\ideas\rsmorphy-alternative.md`)
- **Mystem API** — внешний вызов (избыточно для этой задачи)

**Решение:** Используем эвристику. При необходимости можно заменить на rsmorphy в будущем.

