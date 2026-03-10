# Code Review Fixes #004 - 2026-03-11

## Обзор
План исправления замечаний из код-ревью #004 (оптимизация и улучшения).
Всего 6 задач: 2 критичные, 2 высокие, 2 средние.

## Приоритет 1: Критичные проблемы (CRITICAL)

### 1.1. Неэффективность использования памяти в аудио плеере 🔴
**Проблема**: `(*mp3_data).clone()` в `audio/player.rs:125` клонирует весь Vec<u8> вместо использования zero-copy view через `as_ref()`.
**Файлы**: `src-tauri/src/audio/player.rs:125`
**Риск**: HIGH - лишние 1 МБ аллокаций на каждое воспроизведение с двойным выводом
**Задачи**:
- [ ] Заменить `(*mp3_data).clone()` на `mp3_data.as_ref()` в `play_to_device`
- [ ] Убедиться, что rodio::Decoder корректно работает с `&[u8]`
- [ ] Добавить комментарий про zero-copy работу с Arc
- [ ] Протестировать с различными размерами MP3 файлов
- [ ] Замерить использование памяти до/after

**Код для изменения:**
```rust
// ДО:
let cursor = Cursor::new((*mp3_data).clone());

// ПОСЛЕ:
let cursor = Cursor::new(mp3_data.as_ref());
```

---

### 1.2. Утечка потоков в звуковой панели 🔴
**Проблема**: `std::thread::spawn` в `soundpanel/state.rs:123` создаёт потоки, которые никогда не завершаются и не отслеживаются.
**Файлы**: `src-tauri/src/soundpanel/state.rs:116-126`
**Риск**: HIGH - утечка памяти (~100 МБ/час для активного пользователя)
**Задачи**:
- [ ] Добавить поле `active_playbacks: Arc<Mutex<Vec<JoinHandle<()>>>>` в `SoundPanelState`
- [ ] Изменить `play_sound` для сохранения `JoinHandle`
- [ ] Добавить очистку завершённых потоков через `retain(|h| !h.is_finished())`
- [ ] Добавить метод `stop_all_sounds()` для остановки всех воспроизведений
- [ ] Обновить конструктор `new()` для инициализации `active_playbacks`
- [ ] Протестировать утечку памяти при многократном воспроизведении

**Код для изменения:**
```rust
// Добавить в SoundPanelState:
pub struct SoundPanelState {
    // ... существующие поля
    active_playbacks: Arc<Mutex<Vec<JoinHandle<()>>>>,
}

// Изменить play_sound:
pub fn play_sound(&self, binding: &SoundBinding) {
    let appdata_path = self.appdata_path.lock().unwrap().clone();
    let sound_path = format!("{}\\soundpanel\\{}", appdata_path, binding.filename);

    let handle = std::thread::spawn(move || {
        super::audio::play_audio_file(&sound_path);
    });

    if let Ok(mut playbacks) = self.active_playbacks.lock() {
        playbacks.push(handle);
        playbacks.retain(|h| !h.is_finished());
    }
}

// Добавить метод:
pub fn stop_all_sounds(&self) {
    if let Ok(mut playbacks) = self.active_playbacks.lock() {
        playbacks.drain(..);
    }
}
```

---

## Приоритет 2: Высокие улучшения (HIGH)

### 2.1. Конкуренция за блокировки в управлении состоянием 🟡
**Проблема**: Сложная иерархия блокировок в `state.rs`, получение 3+ отдельных блокировок для одиночных операций.
**Файлы**: `src-tauri/src/state.rs:13-18, 296-322, 38-99`
**Риск**: MEDIUM - снижение производительности, сложность поддержки
**Задачи**:
- [ ] Создать структуру `TtsConfig` с унифицированной конфигурацией TTS
- [ ] Заменить отдельные `Arc<Mutex>` поля на `Arc<RwLock<TtsConfig>>`
- [ ] Обновить все методы получения/установки TTS настроек
- [ ] Удалить иерархию упорядочивания блокировок (больше не нужна)
- [ ] Обновить все места, где используются старые поля
- [ ] Добавить интеграционные тесты для конкурентного доступа

**Код для изменения:**
```rust
// Создать новую структуру:
#[derive(Clone, Debug)]
pub struct TtsConfig {
    pub provider_type: TtsProviderType,
    pub openai_key: Option<String>,
    pub openai_voice: String,
    pub openai_proxy_host: Option<String>,
    pub openai_proxy_port: Option<u16>,
    pub local_url: String,
}

impl Default for TtsConfig {
    fn default() -> Self {
        Self {
            provider_type: TtsProviderType::OpenAi,
            openai_key: None,
            openai_voice: "alloy".to_string(),
            openai_proxy_host: None,
            openai_proxy_port: None,
            local_url: "http://127.0.0.1:8124".to_string(),
        }
    }
}

// В AppState:
pub struct AppState {
    // ... другие поля
    pub tts_config: Arc<RwLock<TtsConfig>>,
    // Удалить: openai_api_key, openai_voice, openai_proxy_host,
    //         openai_proxy_port, tts_provider_type, tts_providers,
    //         local_tts_url
}
```

---

### 2.2. Неэффективное перечисление устройств 🟡
**Проблема**: Каждое воспроизведение заново перечисляет все аудио устройства (O(n) lookup).
**Файлы**: `src-tauri/src/audio/player.rs:101-110`
**Риск**: MEDIUM - избыточная работа при каждом воспроизведении
**Задачи**:
- [ ] Добавить `cached_devices: Arc<RwLock<HashMap<String, Device>>>` в `AppState`
- [ ] Создать метод `refresh_devices()` для обновления кэша
- [ ] Вызывать `refresh_devices()` при инициализации и изменении устройств
- [ ] Изменить `play_to_device` для использования кэша
- [ ] Добавить обработку ошибок при отсутствии устройства в кэше
- [ ] Рассмотреть добавление callback для событий изменения устройств

**Код для изменения:**
```rust
// В AppState:
pub struct AppState {
    // ... существующие поля
    pub cached_devices: Arc<RwLock<HashMap<String, cpal::Device>>>,
}

// Добавить метод:
impl AppState {
    pub async fn refresh_devices(&self) -> Result<(), String> {
        let host = cpal::default_host();
        let mut cache = self.cached_devices.write().await;
        cache.clear();

        for (index, device) in host.output_devices()
            .map_err(|e| format!("Failed to get devices: {}", e))?
            .enumerate()
        {
            cache.insert(index.to_string(), device);
        }
        Ok(())
    }
}

// В play_to_device:
let cached = state.cached_devices.read().await;
let device = cached.get(&device_id)
    .ok_or_else(|| format!("Device not found: {}", device_id))?;
drop(cached); // Явно освобождаем read lock перед использованием
```

---

## Приоритет 3: Средние улучшения (MEDIUM)

### 3.1. Несогласованная обработка ошибок 🟢
**Проблема**: Смешивание `Result<(), String>`, `anyhow::Result` и `.unwrap()`, 42 случая `unwrap()`/`expect()` в 9 файлах.
**Файлы**: множественные файлы в `src-tauri/src/`
**Риск**: LOW - качество кода, отладка
**Задачи**:
- [ ] Создать `error.rs` с `AppError` enum используя `thiserror`
- [ ] Определить варианты ошибок: Io, Network, TtsFailed, Audio, Config
- [ ] Создать алиас `type Result<T> = std::result::Result<T, AppError>`
- [ ] Заменить `Result<(), String>` на `Result<()>` в критичных путях
- [ ] Заменить `unwrap()`/`expect()` на `?` с контекстом
- [ ] Добавить `.context()` через anyhow для chained errors
- [ ] Обновить все вызовы с `String` ошибками на `AppError`

**Код для добавления:**
```rust
// src-tauri/src/error.rs:
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Network error: {0}")]
    Network(String),

    #[error("TTS synthesis failed: {0}")]
    TtsFailed(String),

    #[error("Audio playback error: {0}")]
    Audio(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, AppError>;
```

---

### 3.2. Мёртвый код 🟢
**Проблема**: Аннотации `#[allow(dead_code)]` в нескольких файлах, TODO комментарии без действий.
**Файлы**: множественные файлы
**Риск**: LOW - качество кода
**Задачи**:
- [ ] Включить `rust.dead_code = "warn"` в Cargo.toml
- [ ] Найти весь мёртвый код через `cargo clippy`
- [ ] Удалить неиспользуемые функции и методы
- [ ] Заменить `#[allow(dead_code)]` на `#[must_use]` где применимо
- [ ] Разобрать TODO комментарии - либо выполнить, либо удалить
- [ ] Добавить `#[must_use]` для методов, возвращающих важные значения

**Изменения в Cargo.toml:**
```toml
[lints.rust]
dead_code = "warn"
unused = "warn"

[lints.clippy]
todo = "warn"
```

---

## Зависимости

### Блокирующие зависимости:
- Задача 2.1 зависит от 1.1 (обе изменяют audio/player.rs) ⚠️
- Задача 2.2 зависит от 2.1 (оба используют AppState) ⚠️

### Параллельное выполнение:
- Задачи 1.1 и 1.2 независимы ✅
- Задачи 3.1 и 3.2 независимы от остальных ✅

### Связанные планы:
- План #38: Рефакторинг lib.rs (независим) ✅

## Порядок выполнения

### Фаза 1: Критические исправления (День 1-2)
1. **1.1** - Исправить клонирование в audio/player.rs (30 мин)
2. **1.2** - Добавить отслеживание потоков в soundpanel (2 часа)

### Фаза 2: Высокие улучшения (День 3-5)
3. **2.1** - Рефакторинг блокировок в state.rs (4 часа)
4. **2.2** - Кэширование устройств (2 часа)

### Фаза 3: Средние улучшения (День 6-8)
5. **3.1** - Унификация обработки ошибок (4 часа)
6. **3.2** - Очистка мёртвого кода (2 часа)

## Критерии завершения

- [ ] Все задачи выполнены
- [ ] `cargo build` успешно
- [ ] `cargo clippy` без предупреждений
- [ ] `cargo test` проходит
- [ ] Нет утечек памяти при тестировании звуковой панели
- [ ] Использование памяти снижено (замеры до/after)

## Риски

- **Breaking changes**: Рефакторинг state.rs может сломать существующий код (~25-30 мест)
- **Тестирование**: Трудно протестировать снижение памяти без бенчмарков
