# Implementation: Telegram Авторизация для Silero TTS

## Статус: ✅ ЗАВЕРШЕНО

Дата завершения: 2025-03-05

---

## Реализованная функциональность

### Backend (Rust)

#### 1. Зависимости в `src-tauri/Cargo.toml`
```toml
grammers-client = "0.8.0"
grammers-session = "0.8.0"
grammers-mtsender = "0.8.0"
grammers-tl-types = "0.8.0"
chrono = "0.4"
```

#### 2. Модуль `src-tauri/src/telegram/`

**client.rs** - TelegramClient
- `init()` - инициализация с api_id, api_hash, phone
- `request_code()` - запрос кода подтверждения
- `sign_in()` - вход с кодом (30s timeout)
- `get_user_info()` - получение данных пользователя
- `disconnect()` - отключение клиента
- `sign_out()` - выход и удаление сессии

**bot.rs** - SileroTtsBot (синтез речи)
- `synthesize()` - основной метод синтеза
- `send_text_to_bot()` - отправка текста @silero_voice_bot
- `wait_for_voice_message()` - ожидание аудио
- `download_voice_to_temp()` - сохранение в %TEMP%

**types.rs** - Типы данных
```rust
pub enum AuthState { Idle, CodeRequired, Connected, Error }
pub struct UserInfo { id, first_name, last_name, username, phone }
pub struct TtsResult { success, audio_path, duration, error }
```

#### 3. Tauri Commands в `src-tauri/src/commands/telegram.rs`

| Команда | Описание |
|---------|----------|
| `telegram_init` | Инициализация клиента |
| `telegram_request_code` | Запрос кода |
| `telegram_sign_in` | Вход с кодом |
| `telegram_sign_out` | Выход |
| `telegram_get_status` | Статус подключения |
| `telegram_get_user` | Данные пользователя |
| `speak_text_silero` | Синтез речи |

---

### Frontend (Vue 3 + TypeScript)

#### 1. `src/composables/useTelegramAuth.ts`

**Состояния:** `idle`, `loading`, `code_required`, `connected`, `error`

**Методы:**
- `init()` - проверка статуса
- `requestCode(credentials)` - запрос кода
- `signIn(code)` - вход
- `signOut()` - выход
- `getStatus()` - статус
- `speak(text)` - синтез речи

#### 2. `src/components/TelegramAuthModal.vue`

**Состояние 1 - Форма входа:**
- Поля: телефон, API ID, API Hash
- Ссылка: https://my.telegram.org/apps
- Кнопка: "Получить код"

**Состояние 2 - Ввод кода:**
- Текст: "Код отправлен на {номер}"
- Поле кода
- Кнопка: "Подтвердить"

**Состояние 3 - Подключено:**
- Аватар и имя пользователя
- Текст: "Подключено как @username"
- Кнопка: "Отключить"

**Состояние ошибки:**
- Иконка ⚠️ + текст ошибки
- Кнопки: "Попробовать снова", "Отключить"

#### 3. `src/components/TtsPanel.vue`

**Добавлено в Silero Bot секцию:**
- Индикатор статуса (зелёный/красный)
- Информация о подключённом пользователе
- Кнопка "Подключить Telegram"
- Инфо-текст: "Для работы Silero TTS убедитесь, что в боте включены формат голосовых"
- Красная плашка при ошибках

---

## Хранение данных

**Сессия Telegram:**
- Windows: `%APPDATA%\app-tts-v2\telegram.session`
- SQLite формат (grammers-session)
- Автосохранение авторизации

---

## Обработка ошибок

### Красная плашка Silero TTS
- Фон: `#fef2f2` (светло-красный)
- Рамка: `#ef4444` (красный)
- Кнопка "Исправить" → открывает модал

### Модальное окно ошибки
- Детальное описание проблемы
- Кнопки: "Попробовать снова", "Отключить"

---

## Поток синтеза речи

```
Frontend: useTelegramAuth.speak(text)
    ↓
Tauri: speak_text_silero(text)
    ↓
SileroTtsBot::synthesize()
    ↓
1. send_text_to_bot(@silero_voice_bot)
2. wait_for_voice_message()
3. download_voice_to_temp()
    ↓
Return TtsResult { audio_path }
```

---

## Компиляция

✅ **Rust:** `cargo check` - успешено
✅ **Vue:** `npm run build` - успешно

---

## Файлы

### Созданные:
- `src-tauri/src/telegram/client.rs`
- `src-tauri/src/telegram/bot.rs`
- `src-tauri/src/telegram/types.rs`
- `src-tauri/src/telegram/mod.rs`
- `src-tauri/src/commands/telegram.rs`
- `src/composables/useTelegramAuth.ts`
- `src/components/TelegramAuthModal.vue`

### Обновлённые:
- `src-tauri/Cargo.toml`
- `src-tauri/src/lib.rs`
- `src-tauri/src/commands/mod.rs`
- `src-tauri/src/tts/silero.rs`
- `src/components/TtsPanel.vue`

---

## Следующие шаги (опционально)

1. Тестирование с реальным Telegram API
2. Добавление настроек голоса (speaker selection)
3. Кэширование аудио файлов
4. Индикация прогресса синтеза
5. Обработка 2FA (two-factor authentication)
