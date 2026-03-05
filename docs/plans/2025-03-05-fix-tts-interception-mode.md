# Fix TTS in Text Interception Mode

**Дата:** 2025-03-05
**Статус:** 📝 Plan created

## Описание проблемы

**Текущее поведение:**

В режиме перехвата текста (когда пользователь нажимает Enter в плавающем окне), обработчик события `TextReady` **жёстко прописан** только для OpenAI TTS:

```rust
// lib.rs:505-592
AppEvent::TextReady(text) => {
    // Получает API ключ только для OpenAI
    let api_key = state.openai_api_key.lock()...

    if !api_key.is_empty() {
        let client = OpenAiTts::new(api_key);  // ЖЁСТКО OpenAI!
        client.synthesize(&text).await...
    }
}
```

**Проблемы:**
- ❌ Silero TTS - НЕ работает в режиме перехвата
- ❌ Local TTS - НЕ работает в режиме перехвата
- ❌ Код дублирует логику из `speak_text` команды
- ❌ Не использует `tts_providers` из состояния

**Ожидаемое поведение:**
- ✅ В режиме перехвата должен работать тот же TTS провайдер, что выбран в настройках
- ✅ Переключение провайдера должно влиять на TTS в перехвате

## Решение

Переписать обработчик `TextReady` для использования `tts_providers` из состояния, аналогично команде `speak_text`.

### Архитектура

**Сейчас (неправильно):**
```
TextReady event
    ↓
Жёстко создаёт OpenAiTts
    ↓
synthesize()
```

**Должно быть:**
```
TextReady event
    ↓
Берёт tts_providers из state
    ↓
provider.synthesize()  → работает для OpenAI, Silero, Local!
```

## Шаги реализации

### Шаг 1: Переписать `TextReady` обработчик

**Файл:** `src-tauri/src/lib.rs`

**Изменить обработчик `AppEvent::TextReady`:**

```rust
AppEvent::TextReady(text) => {
    eprintln!("[EVENT] Text ready for TTS: {}", text);

    // Preprocess text
    let text = if let Some(preprocessor) = state.get_preprocessor() {
        preprocessor.process(&text)
    } else {
        text
    };

    // Используем speak_text команду для единообразия
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        // Вызываем ту же логику, что и speak_text
        match crate::commands::speak_text_internal(&state, text).await {
            Ok(_) => {
                eprintln!("[EVENT] TTS started successfully");
            }
            Err(e) => {
                state.emit_event(AppEvent::TtsError(e));
            }
        }
    });
}
```

### Шаг 2: Создать внутреннюю функцию `speak_text_internal`

**Файл:** `src-tauri/src/commands/mod.rs`

**Вынести логику TTS из `speak_text` в отдельную функцию:**

```rust
/// Внутренняя функция для TTS (используется и из команды, и из событий)
pub async fn speak_text_internal(
    state: &AppState,
    text: String,
) -> Result<(), String> {
    // Логика из текущего speak_text
    // 1. Проверить provider
    // 2. provider.synthesize()
    // 3. Воспроизвести аудио
}
```

### Шаг 3: Обновить `speak_text` команду

```rust
#[tauri::command]
pub async fn speak_text(state: State<'_, AppState>, text: String) -> Result<(), String> {
    speak_text_internal(&state, text).await
}
```

## Проверка

После реализации:

1. ✅ Перехват текста работает с OpenAI TTS
2. ✅ Перехват текста работает с Silero TTS
3. ✅ Перехват текста работает с Local TTS
4. ✅ Переключение провайдера влияет на TTS в перехвате
5. ✅ Нет дублирования кода

## Файлы для изменения

1. `src-tauri/src/lib.rs` - обновить обработчик `TextReady`
2. `src-tauri/src/commands/mod.rs` - создать `speak_text_internal`, обновить `speak_text`

## Заметки

- `speak_text_internal` должна быть публичной, чтобы её мог вызывать `lib.rs`
- AudioSettingsManager загружается внутри функции (как сейчас)
- TTS провайдер берётся из `state.tts_providers`
