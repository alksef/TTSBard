# Research: Постобработка аудио — очистка и улучшение TTS-вывода

**Дата:** 2026-07-11  
**Автор:** Antigravity (research)  
**Статус:** research note / необязательная идея  
**Связано:** `PROBLEMS.md`, `src-tauri/src/audio/effects.rs`, `src-tauri/src/tts/silero.rs`

---

## 1. Контекст проблемы

Silero TTS — основной локальный движок в TTSBard — имеет известные ограничения качества звучания:

- **Silero via Telegram bot** (`silero.rs`) возвращает `Vec<u8>` в формате **MP3** — т.е. уже с потерями кодека
- **Local TTS** (`local.rs`) возвращает WAV через base64 от локального HTTP-сервера
- Existing pipeline (`effects.rs`, 755 строк): MP3 decode → PCM → resample (rubato) → pitch shift (phase vocoder) → WAV encode
- Текущий стек: `symphonia`, `rubato`, `pitch_shift`, `rodio`, `cpal` — всё в Rust
- **Никакого noise reduction / speech enhancement сейчас нет**

**Точка вставки для enhancement:** функция `apply_effects()` в `effects.rs` — после speed/pitch, перед `encode_wav()`.

---

## 2. Ключевое ограничение: «Проблема чистой речи»

> **⚠️ ВАЖНО:** Большинство моделей улучшения речи обучены на **микрофонных записях** с реальным шумом. TTS-аудио уже синтетически чистое. Применение классических шумоподавителей к TTS может дать **over-processing**: замыленность, пропадание согласных, металлические артефакты.

**Что реально даёт эффект на TTS-аудио:**
- ✅ Удаление артефактов MP3-кодека (актуально для Silero через Telegram)
- ✅ Лёгкое спектральное улучшение (warmth/brightness)
- ✅ Upsampling — если Silero отдаёт низкую частоту (8–16 kHz OGG/MP3)
- ❌ Подавление шума — у TTS нет шума для подавления

---

## 3. Рассмотренные инструменты

### 3.1 DeepFilterNet ⭐ РЕКОМЕНДУЕТСЯ (Rust-нативный)

| Параметр | Значение |
|---|---|
| **GitHub** | https://github.com/Rikorose/DeepFilterNet |
| **Звёзды** | ~5,000+ |
| **Лицензия** | Apache 2.0 |
| **Язык ядра** | **Rust** (`libDF` crate) + PyO3 биндинги для Python |
| **Размер модели** | ~3.5–9 MB (DNF3 quantized до 3.5 MB) |
| **RTF на CPU** | ~0.04–0.06 (значительно быстрее реального времени) |
| **Sample rate** | 48 kHz (внутренний), поддержка 8/16/24/48 kHz |
| **Тип** | Speech enhancement / noise suppression |
| **Статус** | Активно развивается (2021–2024+) |

**Архитектура:** Deep Filtering — двухступенчатый подход:
1. ERB-частотный анализ (critcial band processing по модели слухового восприятия)
2. GRU-сеть для оценки маски глубокой фильтрации по частотным бинам
3. Применение маски в частотной области — 20ms lookahead, causal mode доступен

**Ключевое преимущество:** Ядро — **Rust crate** (`deep-filter` на crates.io). Прямая интеграция в Tauri без Python:

```toml
# Cargo.toml
deep-filter = "0.5"   # проверить актуальную версию на crates.io
```

**Точка вставки в `effects.rs`:**
```rust
// После speed/pitch эффектов, перед encode_wav()
#[cfg(feature = "enhancement")]
fn apply_speech_enhancement(samples: &[f32], sample_rate: u32) -> Vec<f32> {
    use deep_filter::{DFState, Model};
    let model = Model::default(); // встроенная модель
    let mut state = DFState::new(sample_rate, model.fft_size(), model.hop_size(), 1);
    let mut output = Vec::with_capacity(samples.len());
    for frame in samples.chunks(state.hop_size()) {
        let enhanced = state.process(frame);
        output.extend_from_slice(&enhanced);
    }
    output
}
```

**Python sidecar** (альтернатива без изменения Rust):
```python
from df.enhance import enhance, init_df, load_audio, save_audio
model, df_state, _ = init_df()
audio, _ = load_audio("input.wav", sr=df_state.sr())
# atten_lim_db=10 — мягкая фильтрация, подходит для TTS (не даёт over-processing)
enhanced = enhance(model, df_state, audio, atten_lim_db=10)
save_audio("output.wav", enhanced, df_state.sr())
```

**Риски для TTS-аудио:** Обучена на шумных записях → может чуть over-suppress согласные.  
**Решение:** Параметр `atten_lim_db=10–12` или `--pf` флаг (post-filter mode — мягче).

**Ресурсы:** RAM ~100–300 MB при инференсе, никакого GPU не нужно.

---

### 3.2 VoiceFixer

| Параметр | Значение |
|---|---|
| **GitHub** | https://github.com/haoheliu/VoiceFixer |
| **Звёзды** | ~2,100 |
| **Лицензия** | MIT |
| **Язык** | Python (PyTorch) |
| **Размер модели** | ~50–100 MB checkpoint |
| **RTF на CPU** | 10–30 секунд на минуту аудио (очень медленно) |
| **GPU RAM** | 2 GB рекомендуется |
| **Sample rate** | Выход 44 100 Hz |
| **Тип** | General speech restoration (шумоподавление + super-resolution + деклиппинг) |
| **Статус** | Практически заброшен (последние commits ~2022–2023) |

**Архитектура:** UNet-подобная сеть + HiFi-GAN вокодер ресинтез.

**Режимы:**
- Mode 0: полное восстановление
- Mode 1: только re-вокодинг (наиболее подходит для TTS — добавляет "живость")
- Mode 2: обратное применение (не нужно)

```python
from voicefixer import VoiceFixer
vf = VoiceFixer()
vf.restore(input="input.wav", output="output.wav", cuda=False, mode=0)
```

**Проблемы для проекта:**
- Медленно на CPU: ~10–30 сек на 5-секундный фрагмент
- Нет Rust-биндингов, нужен Python subprocess
- Может «размыть» характер голоса через HiFi-GAN ресинтез
- Полезен только при апсэмплинге с низкого SR (8/16 kHz → 44.1 kHz)
- **Заброшен как проект**

> **❌ Не рекомендуется** для TTSBard.

---

### 3.3 Resemble Enhance

| Параметр | Значение |
|---|---|
| **GitHub** | https://github.com/resemble-ai/resemble-enhance |
| **Звёзды** | ~2,500+ |
| **Лицензия** | Apache 2.0 |
| **Язык** | Python (PyTorch + diffusion) |
| **Размер модели** | ~200–400 MB |
| **RTF на CPU** | ~50–200x медленнее реального времени (диффузионная часть) |
| **GPU VRAM** | 4–8 GB для полного pipeline |
| **Тип** | Denoising + Speech Super Resolution (до 44.1 kHz) |
| **Статус** | Умеренно активен |

**Архитектура:** Два модуля:
1. **Denoiser** — быстрая discriminative модель (реально-временная на CPU)
2. **Enhancer** — диффузионная модель для super-resolution (очень медленная на CPU)

Denoiser-only режим приемлем по скорости, полный pipeline — только с GPU.

> **❌ Не подходит** для realtime без GPU. Denoiser-only может быть рассмотрен как лёгкий вариант, но модель весит ~200 MB.

---

### 3.4 RNNoise / nnnoiseless (Rust)

| Параметр | Значение |
|---|---|
| **GitHub (C)** | https://github.com/xiph/rnnoise |
| **GitHub (Rust)** | https://github.com/RustAudio/nnnoiseless |
| **Звёзды** | ~4,000 (C) / ~200 (Rust) |
| **Лицензия** | BSD-3 |
| **Размер модели** | ~100 KB |
| **RTF на CPU** | ~0.01 (ультра-быстро) |
| **Sample rate** | 48 000 Hz (фиксировано) |
| **Тип** | Noise suppression только |

**Архитектура:** 3-слойная GRU с 48 нейронами. Используется внутри Opus-кодека и WebRTC.

**Rust-интеграция:** `nnnoiseless` — чистый Rust-крейт на crates.io. Нативная интеграция в Tauri.

**Важная оговорка:** На чистом TTS-аудио практически ничего не делает — подавлять нечего. Может немного suppressить согласные (sibilants). Для TTSBard — нулевой риск, минимальный эффект.

---

### 3.5 DTLN (Dual-Signal Transformation LSTM Network)

| Параметр | Значение |
|---|---|
| **GitHub** | https://github.com/breizhn/DTLN |
| **Звёзды** | ~1,200 |
| **Лицензия** | MIT |
| **Размер модели** | ~3–5 MB (ONNX) |
| **RTF на CPU** | ~0.3–0.5 |
| **Sample rate** | 16 000 Hz (ограничение!) |
| **Тип** | Speech enhancement (LSTM + ONNX) |

**Rust-интеграция через `ort`:** ONNX Runtime Rust биндинги (`ort` crate на crates.io).  
**Ограничение:** Только 16 kHz — для Silero v4 (24 kHz) нужен ресэмплинг.

---

### 3.6 Традиционная DSP-цепочка (без нейросети)

Для TTS-постобработки **без риска over-processing** — традиционный DSP:

| Шаг | Эффект |
|---|---|
| **High-shelf EQ** (+2–3 dB @ 8 kHz) | Добавляет «воздух» и чёткость согласных |
| **Лёгкая компрессия** (ratio 2:1, threshold -18 dBFS) | Выравнивает динамику, делает речь «живее» |
| **Harmonic exciter** (waveshaper + bandpass) | Синтетические высокие обертоны |
| **Peak normalization** (-1 dBFS) | Стандартизация уровня |

Нулевые зависимости, ~100 строк Rust в `effects.rs`, полная управляемость.

---

## 4. Сравнительная таблица

| Инструмент | Качество | CPU RTF | Размер | Rust-native | Для TTS | Статус |
|---|---|---|---|---|---|---|
| **DeepFilterNet** | ⭐⭐⭐⭐ | ~0.05 | ~3.5–9 MB | ✅ full crate | ⭐⭐⭐⭐ | Активный |
| **Resemble (denoiser)** | ⭐⭐⭐ | ~0.1–0.3 | ~200 MB | ❌ | ⭐⭐⭐ | Умеренный |
| **Resemble (full)** | ⭐⭐⭐⭐⭐ | ~50x slow | ~400 MB | ❌ | ⭐⭐ | Умеренный |
| **VoiceFixer** | ⭐⭐⭐⭐ | 10–30s/min | ~100 MB | ❌ | ⭐⭐ | Заброшен |
| **RNNoise** | ⭐ | ~0.01 | ~100 KB | ✅ | ⭐ | Стабильный |
| **DTLN (ONNX)** | ⭐⭐⭐ | ~0.3–0.5 | ~3–5 MB | 🔶 via ort | ⭐⭐⭐ | Умеренный |
| **DSP-цепочка** | ⭐⭐⭐ | <0.01 | 0 | ✅ native | ✅⭐⭐⭐⭐ | N/A |

---

## 5. Рекомендации для TTSBard

### Вариант А — Быстрая победа: DSP post-chain в `effects.rs`

Наименее рискованный и самый быстрый путь:
- High-shelf boost + лёгкая компрессия + нормализация
- ~100 строк Rust, zero dependencies
- Opt-in флаг в настройках
- Может заметно улучшить восприятие голоса

### Вариант Б — Нейросетевой: DeepFilterNet нативный Rust

**Приоритет**: прямая интеграция через `deep-filter` crate:
```toml
# src-tauri/Cargo.toml
deep-filter = "0.5"
```
- Слотируется в `apply_effects()` после pitch/speed
- Включать as optional Cargo feature (`enhancement`)
- `atten_lim_db=10` — мягкий режим для чистого TTS
- По умолчанию **выключено**, пользователь включает в настройках

### Перед реализацией (обязательно протестировать)

- [ ] Замерить: какой реальный SR выдаёт Silero через Telegram (`silero.rs` → MP3 decode → проверить sr)
- [ ] Прогнать `deep-filter --atten-lim 10 input.wav` на реальных семплах
- [ ] Сравнить A/B: raw vs. DSP-chain vs. DeepFilterNet(atten=10)
- [ ] Проверить задержку: влияет ли на dual-audio (динамики + виртуальный микрофон)

---

## 6. Архитектура (если интегрировать)

```
TTS Provider (Silero / Local)
    → WAV/MP3 buffer
    → decode (symphonia) → PCM f32
    → speed resample (rubato)
    → pitch shift (phase vocoder)
    → [OPT] Speech Enhancement      ← НОВЫЙ шаг
         ├── DSP-chain (Rust native, in effects.rs)
         └── DeepFilterNet (deep-filter crate, optional feature)
    → encode WAV
    → Playback (speakers + virtual mic via CPAL)
```

**Конфиг:** `audio_enhance: { enabled: bool, backend: "dsp" | "deepfilter", atten_db: f32 }`

**Tauri command:**
```rust
#[tauri::command]
async fn get_audio_enhance_settings() -> EnhanceSettings { ... }
#[tauri::command]
async fn set_audio_enhance_settings(settings: EnhanceSettings) -> Result<()> { ... }
```

---

## 7. Итог

| Решение | Рекомендация |
|---|---|
| **VoiceFixer** | ❌ Медленный, заброшен, неприемлем |
| **Resemble Enhance** | ❌ Требует GPU для полного pipeline |
| **Resemble (denoiser only)** | ⚠️ Возможно, но 200 MB модель |
| **RNNoise / nnnoiseless** | ⚠️ Нулевой риск, минимальный эффект на TTS |
| **DTLN (ONNX)** | ⚠️ Только 16 kHz, нужен ресэмплинг |
| **DSP-цепочка** | ✅ Быстрая победа, нулевые зависимости |
| **DeepFilterNet (Rust crate)** | ✅ **Лучший выбор:** лёгкий, нативный Rust, API гибкий |

**Оптимальная стратегия:** DSP-цепочка как быстрая победа (без зависимостей), DeepFilterNet как opt-in "premium" режим через `deep-filter` crate нативно в Rust.

