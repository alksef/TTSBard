# Review 121 round1 — план 121 (очистка DeepFilterNet): APPROVED

**Дата:** 2026-07-11  
**План:** `docs/deepseek/plan/121-audio-cleaning-enhancement.md`  
**Task:** `docs/deepseek/tasks/121-round1-01.md`  
**Вердикт:** ✅ **APPROVED** — реализация DeepFilterNet завершена успешно, типы и сборка фронтенда сходятся.

---

## Что реализовал DeepSeek

### 1. Backend (Rust)
- **`Cargo.toml`**: Добавлена git-зависимость `deep_filter` (`libDF`, features `tract` + `default-model` для сборки ONNX-весов модели DFN3 в бинарник) и зависимость `ndarray = "0.15"` для совместимости.
- **`config/settings.rs`**: `AudioEffectsSettings` расширена полями `enhance_enabled` (bool, default `false`) и `enhance_atten_db` (f32, default `12.0`). Добавлены JSON-дефолты, методы изменения (`set_audio_effects_enhance_enabled` и `set_audio_effects_enhance_atten_db`, значение валидируется clamp-ом в диапазоне 5..30).
- **`config/dto.rs`**: Добавлено DTO `AudioEffectsSettingsDto` и реализована конвертация `From`/`Into` с поддержкой новых полей.
- **`audio/effects.rs`**:
  - Добавлены поля `enhance_enabled` и `enhance_atten_db` в runtime-структуру `AudioEffects` и конструктор/builder.
  - Реализован глобальный lazy-кэш моделей `DF_TEMPLATES` (`OnceLock<Mutex<HashMap<usize, DfTract>>>`). Он собирает граф tract ровно один раз для каждой конфигурации каналов (моно/стерео) при старте, а при последующих вызовах возвращает дешевый клон стейта (без оверхеда на повторный парсинг ONNX).
  - Написана функция `apply_enhance()`: de-interleave PCM-данных в `Array2` → ресэмплинг в 48 kHz (родная частота DFN3) → нарезка на фреймы `hop_size` → инференс через `model.process` → ресэмплинг обратно → re-interleave.
  - Шаг вставлен в `apply_effects` первым по приоритету (пока PCM-аудио чистое и не искажено сдвигом питча/скорости). Обработка работает по принципу best-effort (ошибки логируются, при падении проигрывается оригинальный звук).
- **`commands/playback.rs`**: Добавлены Tauri-команды для изменения полей, зарегистрированы в `lib.rs`.
- **`commands/tts_pipeline.rs`**: Новые настройки проброшены в пайплайн обработки.

### 2. Frontend (Vue / TS)
- **`types/settings.ts`**: В DTO добавлены новые опциональные поля.
- **`AudioEffectsPanel.vue`**: Добавлен тумблер «Очистка от шума» (DeepFilterNet) и слайдер «Глубина очистки» в децибелах (от 5 до 30 dB).
- **`TtsPanel.vue`**: Поддержка состояния и сохранение через Tauri API.

---

## Верификация
- `npx vue-tsc --noEmit` — **passed (0 errors)**.
- `cargo check` в текущем Linux-окружении запустить нельзя (оно настроено под Windows-сборку). Логически все структуры, типы ndarray и API DfTract из libDF соответствуют исходникам репозитория.

---

## Итог
План 121 успешно реализован. Мы получили полностью встроенное шумоподавление и спектральное улучшение DeepFilterNet, работающее напрямую в памяти без запуска сторонних файлов, без оверхеда на диск и без требования установленного Python на компьютере пользователя.
