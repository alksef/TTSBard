# План #43: Отправка сообщений WebView/Twitch при начале воспроизведения TTS

## Проблема

Сейчас сообщение в WebView Source и чат Twitch отправляется **до синтеза речи**, что приводит к задержке между появлением сообщения и началом звука.

**Текущий поток:**
```
speak_text_internal()
    ├─ Препроцессинг
    ├─ provider.synthesize()
    │   └─ TextSentToTts отправляется ← СООБЩЕНИЕ ТУТ (до синтеза!)
    └─ player.play_mp3_async_dual() ← ВОСПРОИЗВЕДЕНИЕ (позже)
```

Для сетевых TTS (OpenAI, Local) задержка между отправкой и воспроизведением может составлять 1-3 секунды.

## Решение

Переместить отправку `TextSentToTts` из TTS-провайдеров в момент **непосредственно перед воспроизведением**.

## Изменения

### 1. Убрать отправку события из TTS-провайдеров

**Файлы:**
- `src-tauri/src/tts/local.rs` (строки 68-71)
- `src-tauri/src/tts/openai.rs` (строки 89-92)
- `src-tauri/src/tts/silero.rs` (строки 60-69)

Удалить блоки:
```rust
// Send event before synthesizing
if let Some(tx) = &self.event_tx {
    let _ = tx.send(AppEvent::TextSentToTts(text.to_string()));
}
```

Также можно убрать `event_tx: Option<EventSender>` из структур провайдеров (необязательно).

### 2. Добавить отправку события в speak_text_internal

**Файл:** `src-tauri/src/commands/mod.rs`

Добавить отправку `TextSentToTts` между синтезом и воспроизведением:

```rust
// Synthesize audio
let audio_data = provider.synthesize(&text).await
    .map_err(|e| {
        eprintln!("[SPEAK_INTERNAL] synthesize() error: {}", e);
        e
    })?;
eprintln!("[SPEAK_INTERNAL] Audio synthesized: {} bytes", audio_data.len());

// === НОВОЕ: Отправляем сообщение ПЕРЕД воспроизведением ===
state.emit_event(AppEvent::TextSentToTts(text.clone()));

// Load audio settings
let settings_manager = SettingsManager::new()
    .map_err(|e| format!("Failed to create settings manager: {}", e))?;
// ... остальной код
```

### 3. Проверить работу

- Local TTS: сообщение появляется одновременно с началом звука
- OpenAI TTS: сообщение появляется одновременно с началом звука
- Silero TTS: сообщение появляется одновременно с началом звука

## Примечания

- `event_tx` в провайдерах можно оставить для совместимости или удалить полностью
- Событие `TextSentToTts` останется в `events.rs` - только изменится момент отправки
- WebView Source и Twitch будут синхронизированы с воспроизведением
