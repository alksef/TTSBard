# Fix TTS Provider Switching

**Дата:** 2025-03-05
**Статус:** ✅ Completed

## Описание проблемы

**Проблема:**
При переключении TTS провайдера (например, с Silero на OpenAI) провайдер не инициализировался. Для OpenAI в коде был комментарий "initialized separately" и ничего не происходило.

**Текущее поведение (было):**
```rust
match provider {
    TtsProviderType::OpenAi => {
        // OpenAI initialized separately via set_openai_api_key
        // НИЧЕГО НЕ ДЕЛАЕТСЯ!
    }
    ...
}
```

**Результат:**
- ❌ При переключении на OpenAI провайдер не инициализировался
- ❌ TTS не работал до тех пор, пока пользователь не вводил API ключ заново

## Решение

Инициализировать выбранный провайдер сразу при переключении, используя сохранённые настройки.

**Новое поведение:**
```rust
match provider {
    TtsProviderType::OpenAi => {
        // Получаем сохранённый API ключ
        let api_key = state.openai_api_key.lock()...
        if let Some(key) = api_key {
            state.init_openai_tts(key);  // Инициализируем!
        }
    }
    TtsProviderType::Silero => {
        // Проверяем Telegram сессию и инициализируем Silero
        let client_arc = Arc::clone(&telegram_state.client);
        telegram_auto_restore(telegram_state).await...  // Проверка
        state.init_silero_tts(client_arc);  // Инициализируем!
    }
    TtsProviderType::Local => {
        state.init_local_tts();  // Инициализируем!
    }
}
```

## Изменения

### Файл: `src-tauri/src/commands/mod.rs`

**Функция:** `set_tts_provider`

**Изменения:**

1. **Для OpenAI:**
   - Получаем сохранённый API ключ из `state.openai_api_key`
   - Если ключ есть → вызываем `init_openai_tts(key)`
   - Если ключа нет → warning (но переключение всё равно происходит)

2. **Для Silero:**
   - Упрощена логика: `telegram_auto_restore` только для проверки
   - Инициализация происходит в любом случае (даже если нет клиента - подключится позже)

3. **Для Local:**
   - Без изменений (уже работало)

## Проверка

После исправления:

1. ✅ Переключение на OpenAI → провайдер инициализируется с сохранённым API ключом
2. ✅ Переключение на Silero → провайдер инициализируется с Telegram клиентом
3. ✅ Переключение на Local → провайдер инициализируется
4. ✅ Telegram не выгружается при переключении с Silero
5. ✅ TTS работает сразу после переключения

## Примечания

- API ключ OpenAI должен быть предварительно сохранён (через UI)
- Silero требует авторизации в Telegram, но провайдер создаётся даже без неё
- Нет необходимости выгружать/отключать Telegram при смене провайдера
