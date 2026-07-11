# Plan 121: Очистка и улучшение аудио через встроенный DeepFilterNet

**Дата:** 2026-07-11  
**Сложность:** Высокая — сборка libDF на Rust с tract и бесшовная интеграция в PCM-пайплайн.

---

## 1. Проблема

Silero TTS генерирует речь с металлическими артефактами и искажениями сжатия (выход бота в MP3/OGG).
Запуск внешнего `.exe` / Python подпроцесса на каждую фразу добавляет ~300-500 мс задержки перед воспроизведением из-за инициализации нейросети, что ломает UX.

---

## 2. Решение

Реализовать нативную интеграцию Rust-библиотеки `deep_filter` в один процесс (in-process) с использованием движка `tract`:
1. Подключить `deep_filter` через Git-зависимость (`libDF` subdirectory).
2. Загружать модель `DeepFilterNet3_onnx.tar.gz` (будет встроена через Cargo features) один раз при инициализации приложения или лениво при первом включении.
3. Интегрировать инференс модели в `apply_effects` (в `effects.rs`) для обработки PCM семплов.
4. Добавить настройки включения и уровня очистки (`enhance_enabled`, `enhance_atten_db`) в `AppSettings` и пробросить их во фронтенд Vue.

---

## 3. Шаги реализации

### 3.1 Backend (Rust)

1. **`src-tauri/Cargo.toml`**:
   - Добавить:
     ```toml
     deep_filter = { git = "https://github.com/Rikorose/DeepFilterNet", subdirectory = "libDF", features = ["tract", "default-model"] }
     ```
2. **`src-tauri/src/config/settings.rs`** и **`src-tauri/src/config/dto.rs`**:
   - Добавить поля `enhance_enabled: bool` и `enhance_atten_db: f32` в структуру `AudioEffects` / `AudioEffectsDto` в настройках.
   - Задать дефолтные значения: `enhance_enabled: false`, `enhance_atten_db: 12.0` (мягкая фильтрация).
3. **`src-tauri/src/audio/effects.rs`**:
   - Добавить поддержку `enhance_enabled` и `enhance_atten_db` в структуру `AudioEffects`.
   - Внедрить синглтон/lazy-инициализированный объект `DfTract` (мьютекс или OnceLock).
   - Внутри `apply_effects` при включенном `enhance_enabled` обрабатывать `samples` (f32-слайс) через DeepFilterNet frame-by-frame (используя `DfTract::process` или аналогичный API крейта `deep_filter`).
4. **`src-tauri/src/commands/audio.rs`**:
   - Tauri-команды для переключения очистки и изменения силы: `set_audio_effects_enhance_enabled` и `set_audio_effects_enhance_atten_db` (или обновить существующую `set_audio_effects`).

### 3.2 Frontend (Vue / TypeScript)

1. **`src/types/settings.ts`**:
   - Обновить `AudioEffects` типы.
2. **`src/components/tts/AudioEffectsPanel.vue`**:
   - Отрендерить новый блок настройки улучшения голоса:
     - Toggle-переключатель «Очистка аудио (DeepFilterNet)».
     - Range slider «Глубина очистки» (0-100%), где 0% = `atten_lim_db = 100` (без лимита), а 100% = `atten_lim_db = 0` (максимальное ограничение, хотя лучше ограничить разумным диапазоном, например 5-30 dB).

---

## 4. Критерии приемки

1. Проект собирается (`cargo check` + `npx vue-tsc --noEmit`).
2. Очистка аудио работает без внешних Python зависимостей и без дискового оверхеда.
3. Время задержки перед воспроизведением со включенным шумодавом не превышает 50 мс.
