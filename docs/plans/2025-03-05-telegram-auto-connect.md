# Implementation: Автоматическое подключение к Telegram для Silero TTS

## Статус: ✅ ЗАВЕРШЕНО

Дата завершения: 2025-03-05

---

## Описание проблемы

**Текущее поведение:**
1. Подключение к Telegram происходит только при инициализации `TtsPanel.vue` через `initTelegram()` → `telegram_auto_restore`
2. При переключении TTS-провайдера на Silero НЕ происходит:
   - Подключение к Telegram (если еще не подключено)
   - Загрузка информации (текущий голос, лимиты) из Telegram
3. Информация о голосе и лимитах загружается только вручную через кнопки 🔄

**Ожидаемое поведение:**
1. При запуске приложения: если выбран Silero → сразу подключение к Telegram
2. При переключении на Silero: подключение к Telegram **один раз**
3. Загрузка информации (текущий голос, лимиты) — **только по кнопке 🔄**

---

## План реализации

### 1. Бэкенд (Rust)

#### Файл: `src-tauri/src/commands/mod.rs`

**Изменить команду `set_tts_provider`:**

```rust
/// Set TTS provider type
#[tauri::command]
pub async fn set_tts_provider(  // <-- добавлено async
    state: State<'_, AppState>,
    telegram_state: State<'_, TelegramState>,  // <-- добавлено
    settings_manager: State<'_, SettingsManager>,
    provider: TtsProviderType,
) -> Result<(), String> {
    // Initialize provider based on type
    match provider {
        TtsProviderType::OpenAi => {
            // OpenAI initialized separately via set_openai_api_key
        }
        TtsProviderType::Silero => {
            eprintln!("[SET_PROVIDER] Initializing Silero TTS");
            state.init_silero_tts();

            // Автоматическое восстановление сессии Telegram
            eprintln!("[SET_PROVIDER] Auto-connecting to Telegram...");
            match telegram_auto_restore(state, telegram_state).await {
                Ok(connected) => {
                    if connected {
                        eprintln!("[SET_PROVIDER] Telegram auto-connected successfully");
                    } else {
                        eprintln!("[SET_PROVIDER] Telegram session exists but not authorized");
                    }
                }
                Err(e) => {
                    eprintln!("[SET_PROVIDER] Telegram auto-connect failed: {}", e);
                    // Не прерываем выполнение, просто логируем ошибку
                }
            }
        }
        TtsProviderType::Local => {
            eprintln!("[SET_PROVIDER] Initializing Local TTS");
            state.init_local_tts();
        }
    }

    state.set_tts_provider_type(provider);

    // Auto-save settings
    let settings = SettingsManager::load_from_state(&state);
    settings_manager.save(&settings)
        .map_err(|e| format!("Failed to save settings: {}", e))?;

    eprintln!("[SET_PROVIDER] Provider set to {:?}", provider);
    Ok(())
}
```

**Примечание:** Также нужно обновить вызов в `lib.rs` где регистрируются команды, чтобы передать `TelegramState`.

---

### 2. Frontend (Vue)

#### Файл: `src/components/TtsPanel.vue`

**Изменить функцию `setActiveProvider`:**

```typescript
async function setActiveProvider(provider: TtsProviderType) {
  try {
    await invoke('set_tts_provider', { provider });
    activeProvider.value = provider;
  } catch (error) {
    showError(error as string);
  }
}
```

> **Примечание:** Данные (голос, лимиты) загружаются только по кнопке 🔄, автоматически не загружаются.

---

#### Файл: `src/App.vue`

**Добавить инициализацию Telegram при запуске:**

```typescript
import { useTelegramAuth } from './composables/useTelegramAuth'

const {
  init: initTelegram,
} = useTelegramAuth()

// В onMounted:
onMounted(async () => {
  // Существующая логика...

  // Инициализируем Telegram (восстанавливаем сессию)
  await initTelegram()
})
```

> **Примечание:** Данные (голос, лимиты) НЕ загружаются автоматически, только по кнопке 🔄.

---

## Изменения в файлах

### Backend:
- `src-tauri/src/commands/mod.rs` - изменить `set_tts_provider` на async + добавить `TelegramState`
- `src-tauri/src/lib.rs` - обновить регистрацию команд (передать `TelegramState`)

### Frontend:
- `src/components/TtsPanel.vue` - изменить `setActiveProvider`
- `src/App.vue` - добавить инициализацию Telegram

---

## Обработка ошибок

- Ошибка подключения к Telegram логируется, но **не прерывает** выбор Silero провайдера
- Пользователь увидит статус "Не подключено" в UI и сможет нажать "Подключить Telegram"

---

## Проверка реализации

После реализации проверить:

1. ✅ При запуске с выбранным Silero → автоматическое подключение к Telegram
2. ✅ При переключении на Silero → автоматическое подключение к Telegram
3. ✅ Данные (голос, лимиты) загружаются **только** по кнопке 🔄
4. ✅ Сессия Telegram сохраняется между перезапусками приложения

---

## Заметки

- Функция `telegram_auto_restore` возвращает `Result<bool, String>` где `bool` — успешно ли авторизованы
- При неуспешном подключении Silero всё равно становится активным провайдером
- Пользователь может подключиться к Telegram позже через кнопку "Подключить Telegram"

---

## Выполненная реализация

### Backend (Rust)

**Файл:** `src-tauri/src/commands/mod.rs`

1. Добавлен импорт `TelegramState`:
   ```rust
   use crate::commands::telegram::TelegramState;
   ```

2. Функция `set_tts_provider` изменена на `async`:
   ```rust
   #[tauri::command]
   pub async fn set_tts_provider(
       state: State<'_, AppState>,
       telegram_state: State<'_, TelegramState>,  // добавлено
       settings_manager: State<'_, SettingsManager>,
       provider: TtsProviderType,
   ) -> Result<(), String>
   ```

3. При выборе Silero автоматически вызывается `telegram_auto_restore`:
   ```rust
   TtsProviderType::Silero => {
       state.init_silero_tts();
       match telegram::telegram_auto_restore(telegram_state).await {
           Ok(connected) => { /* логирование */ }
           Err(e) => { /* логирование без прерывания */ }
       }
   }
   ```

### Frontend (Vue)

**Файл:** `src/App.vue`

Добавлена инициализация Telegram при старте приложения:
```typescript
import { useTelegramAuth } from './composables/useTelegramAuth'

const { init: initTelegram } = useTelegramAuth()

onMounted(async () => {
  try {
    await initTelegram()
  } catch (error) {
    console.log('[APP] Telegram auto-init failed or no session:', error)
  }
})
```

**Файл:** `src/components/TtsPanel.vue`

`setActiveProvider` остался без изменений (уже был в нужном формате).

---

## Компиляция

✅ **Rust:** `cargo check` - успешно
✅ **Vue:** `npm run build` - успешно
