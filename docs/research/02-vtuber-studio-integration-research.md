# Research: Интеграция с VTube Studio для автоматического липсинка (Lip-Sync)

**Дата:** 2026-07-10  
**Автор:** Antigravity (research)  
**Статус:** research note  
**Связано:** `PROBLEMS.md`, `docs/research/01-tts-improvement-opportunities-2026-06-28.md`, `src-tauri/src/playback.rs`

---

## 1. Введение и цель

Для стримеров, общающихся исключительно с помощью синтеза речи (TTS), критически важно, чтобы их виртуальный 2D/3D аватар шевелил ртом синхронно с произносимым текстом. Поскольку стример не использует физический микрофон для разговора, обычный трекинг лица по камере не может зафиксировать движение губ.

Цель данного исследования — рассмотреть архитектуру интеграции **TTSBard** с **VTube Studio (VTS)** через локальный API для обеспечения плавного автоматического открытия рта модели во время озвучки сообщений.

---

## 2. Контекст проблемы и превосходство API-решения

В стандартном сценарии VTube Studio получает данные о движении рта двумя путями:
1. **Через веб-камеру**: Камера отслеживает мимику стримера (поскольку TTS-стример молчит, рот персонажа остается закрытым).
2. **Через микрофон (Audio Lip-Sync)**: VTube Studio захватывает звук с микрофона и открывает рот модели на основе громкости входящего сигнала.

### Проблема традиционного подхода (виртуальные кабели)
Чтобы заставить модель шевелиться во время работы TTS, стримерам приходится выстраивать хрупкую цепочку виртуального аудио:
* Устанавливать виртуальные аудиокабели (VB-Cable).
* Направлять звук из TTS-программы в виртуальный кабель.
* Настраивать VTube Studio на прослушивание этого виртуального кабеля как «микрофона».
* Настраивать OBS на захват этого кабеля для трансляции.

*Недостатки:* Windows часто сбрасывает настройки виртуальных кабелей при обновлениях, возникают задержки звука (desync), а также повышается сложность настройки для конечного пользователя.

### Решение через прямое API управление (TTSBard)
Поскольку TTSBard сама генерирует и воспроизводит аудио, она обладает полной информацией о громкости звука в каждый момент времени. Мы можем полностью отказаться от виртуальной аудио-маршрутизации:

```
[TTS-Движок] ──(аудио)──> [TTSBard (Плеер)] ──(вывод на динамики/стрим)
                                 │
                     (анализ амплитуды в Rust)
                                 │
                                 ▼ (значение MouthOpen: 0.0 .. 1.0)
                         [WebSocket API]
                                 │
                                 ▼
                         [VTube Studio] ──(рот модели двигается)
```

1. **Анализ на лету**: Бэкенд TTSBard рассчитывает громкость сэмпла (RMS) в процессе воспроизведения.
2. **Прямая отправка**: Значение громкости транслируется напрямую в WebSocket API VTube Studio в параметр `MouthOpen` (частота 30-50 FPS).
3. **Результат**: Пользователю достаточно включить одну кнопку в настройках TTSBard. Липсинк работает мгновенно, без задержек и без необходимости настраивать виртуальные кабели в Windows и VTube Studio.

---


## 3. API Протокол VTube Studio

VTube Studio запускает локальный WebSocket-сервер (по умолчанию на порту `8001`). Общение происходит в формате JSON.

### Этап 1. Авторизация плагина (Plugin Authentication)

При первом подключении приложение должно получить авторизационный токен от VTube Studio. Для этого отправляется запрос:

```json
{
  "apiName": "VTubeStudioPublicAPI",
  "apiVersion": "1.0",
  "requestID": "TTSBardAuthToken",
  "messageType": "AuthenticationTokenRequest",
  "data": {
    "pluginName": "TTSBard",
    "pluginDeveloper": "alksef"
  }
}
```

VTube Studio покажет пользователю всплывающее окно с вопросом: *«Разрешить плагину TTSBard доступ к VTube Studio?»*.
При одобрении сервер вернет токен в `AuthenticationTokenResponse`:

```json
{
  "apiName": "VTubeStudioPublicAPI",
  "apiVersion": "1.0",
  "messageType": "AuthenticationTokenResponse",
  "requestID": "TTSBardAuthToken",
  "data": {
    "authenticationToken": "a3b9f...8c2"
  }
}
```

Этот токен необходимо сохранить в файле конфигурации приложения (`settings.json`). В последующие разы авторизация выполняется мгновенно с использованием токена:

```json
{
  "apiName": "VTubeStudioPublicAPI",
  "apiVersion": "1.0",
  "requestID": "TTSBardLogin",
  "messageType": "AuthenticationRequest",
  "data": {
    "pluginName": "TTSBard",
    "pluginDeveloper": "alksef",
    "authenticationToken": "a3b9f...8c2"
  }
}
```

Ответ подтверждает успешный вход: `{"authenticated": true}`.

---

## 4. Сценарии реализации Липсинка (Lip-Sync)

Как заставить модель шевелить ртом на основе активности TTS?

### Сценарий A. Логический переключатель (Binary Gate)
При старте озвучки посылаем `MouthOpen = 1.0`, по окончании — `MouthOpen = 0.0`.
* **Плюсы**: Реализуется в 10 строк кода.
* **Минусы**: Модель выглядит как деревянная кукла с постоянно открытым ртом во время фразы. Стримеры обычно избегают этого режима.

### Сценарий B. Процедурный липсинк (Procedural Jaw Flap)
Пока воспроизводится аудио, клиент генерирует изменяющийся во времени параметр `MouthOpen` с частотой ~30 FPS на основе тригонометрических волн и шума (например, `0.4 + 0.6 * sin(t * 12) + random_offset`).
* **Плюсы**: Живой вид, рот активно двигается на частотах человеческой речи (5–12 Гц). Не требует анализа звука.
* **Минусы**: Движения губ не совпадают с интонацией и реальными паузами внутри предложения.

### Сценарий C. Липсинк по амплитуде звука (Audio Amplitude-Based)
Вычисление реального уровня громкости (RMS — Root Mean Square) воспроизводимого аудиопотока в реальном времени. Значение RMS масштабируется в диапазон `0.0..1.0` и отправляется в VTS каждые 30–50 мс.
* **Плюсы**: Максимальный реализм. Модель полностью повторяет громкость и паузы речи. При затухании голоса рот закрывается.
* **Минусы**: Требует доступа к сырым аудиоданным (PCM-сэмплам) во время проигрывания.

### Сценарий D. Фонетический липсинк (Phoneme-Based Vowel Shapes)
Анализ текста или аудио для выделения фонем (гласных звуков A, I, U, E, O) и управление формой рта модели (`MouthSmile` / `MouthOpen` / `MouthX`).
* **Плюсы**: Идеальная артикуляция.
* **Минусы**: Высокая вычислительная сложность, задержка (latency) и сложность интеграции.

> [!IMPORTANT]
> **Рекомендация по приоритетам:**
> Начинать нужно с комбинации **Сценария B** (как резервного) и **Сценария C** (как основного качественного режима).

---

## 5. Архитектура интеграции в TTSBard: Выбор в пользу Бэкенда (Rust)

Хотя изначально рассматривался гибридный подход с WebSocket на фронтенде, для десктопного приложения **TTSBard** наиболее правильной, производительной и стабильной является **полностью бэкенд-ориентированная архитектура (Rust)**.

### Почему бэкенд (Rust) предпочтительнее для WebSocket:

1. **Автономность процесса**: TTSBard сворачивается в трей и продолжает работать в фоне. Если GUI-окно (фронтенд) скрыто, закрыто или переведено в спящий режим операционной системой, WebSocket-соединение с VTube Studio на бэкенде гарантированно останется активным.
2. **Нулевая нагрузка на IPC (Tauri Bridge)**: Передача амплитуды звука на частоте 30–60 FPS через мост Tauri IPC (`emit` событий) требует сериализации JSON в Rust, передачи в V8, парсинга в TypeScript и отправки в WebSocket. Прямая отправка из Rust-потока в TCP-сокет VTS убирает эти накладные расходы полностью.
3. **Прямой доступ к аудио-буферу**: Только на бэкенде в процессе декодирования звука (`rodio`) мы имеем доступ к PCM-сэмплам для расчета RMS в реальном времени.

---

## 6. Техническая реализация на бэкенде

### А. Добавление WebSocket-клиента
В `src-tauri/Cargo.toml` добавляется стандартная библиотека для работы с веб-сокетами в асинхронном рантайме `tokio`:
```toml
tokio-tungstenite = { version = "0.21", features = ["rustls-tls-native-roots"] }
```

### Б. Расчет амплитуды через rodio Source Decorator (Паттерн Декоратор)
Чтобы измерять громкость воспроизводимого TTS-звука без изменения декодеров (`symphonia`, `openai.rs` и т.д.), мы можем написать кастомный декоратор для трейта `rodio::Source`.

Этот декоратор перехватывает сэмплы "на лету" перед отправкой в аудиокарту:

```rust
use rodio::Source;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

pub struct AmplitudeInterceptor<I> {
    inner: I,
    // Шарится с модулем VTS через Atomic (f32 кодируется как u32 bits)
    shared_rms: Arc<AtomicU32>,
    window_size: usize,
    sample_count: usize,
    sum_squares: f32,
}

impl<I> AmplitudeInterceptor<I>
where
    I: Source,
    I::Item: rodio::Sample + Into<f32>,
{
    pub fn new(inner: I, shared_rms: Arc<AtomicU32>, window_ms: u32) -> Self {
        let sample_rate = inner.sample_rate() as f32;
        let window_size = ((sample_rate * (window_ms as f32 / 1000.0)) * inner.channels() as f32) as usize;
        Self {
            inner,
            shared_rms,
            window_size: window_size.max(1),
            sample_count: 0,
            sum_squares: 0.0,
        }
    }
}

impl<I> Iterator for AmplitudeInterceptor<I>
where
    I: Source,
    I::Item: rodio::Sample + Into<f32>,
{
    type Item = I::Item;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let sample = self.inner.next()?;
        let val: f32 = sample.into();
        
        self.sum_squares += val * val;
        self.sample_count += 1;

        if self.sample_count >= self.window_size {
            let rms = (self.sum_squares / self.sample_count as f32).sqrt();
            // Записываем RMS в атомик для асинхронного чтения VTS-клиентом
            self.shared_rms.store(rms.to_bits(), Ordering::Relaxed);
            
            // Сброс окна
            self.sum_squares = 0.0;
            self.sample_count = 0;
        }

        Some(sample)
    }
}

impl<I> Source for AmplitudeInterceptor<I>
where
    I: Source,
    I::Item: rodio::Sample + Into<f32>,
{
    fn current_frame_len(&self) -> Option<usize> {
        self.inner.current_frame_len()
    }

    fn channels(&self) -> u16 {
        self.inner.channels()
    }

    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        self.inner.total_duration()
    }
}
```

**Интеграция в плеер (`player.rs` / `playback.rs`):**
При добавлении источника в `rodio::Sink`:
```rust
let shared_rms = Arc::new(AtomicU32::new(0));
let interceptor = AmplitudeInterceptor::new(source, Arc::clone(&shared_rms), 30); // 30ms окно
sink.append(interceptor);
```

### В. VtsManager на бэкенде
Создается модуль `src-tauri/src/vts/mod.rs`, который работает в отдельном tokio-таске:
1. Читает `settings.vtube_studio` из конфига.
2. При активации пытается установить соединение с `ws://127.0.0.1:{port}`.
3. Выполняет хэндшейк и авторизацию по сохраненному токену.
4. Если статус воспроизведения (`PlaybackStatus`) равен `Playing`:
   * Раз в 30 мс считывает значение громкости из `shared_rms` атомика.
   * Формирует и шлёт в сокет JSON-запрос `InjectParameterDataRequest`.
5. Если соединение падает — пытается переподключиться в фоновом режиме (Backoff reconnection).

---

## 7. Схема конфигурации (`settings.json`)

В настройки бэкенда добавляется структура:

```json
{
  "vtube_studio": {
    "enabled": false,
    "port": 8001,
    "auth_token": "",
    "lip_sync": {
      "mode": "Amplitude", 
      "multiplier": 1.5,
      "parameter_mouth_open": "MouthOpen",
      "parameter_mouth_smile": "MouthSmile"
    }
  }
}
```

---

## 8. План внедрения

1. **Этап I (Зависимости и настройки бэкенда)**:
   * Добавить `tokio-tungstenite` в `Cargo.toml`.
   * Добавить структуру `VtubeStudioSettings` в `src-tauri/src/config/settings.rs`.
2. **Этап II (Анализатор звука)**:
   * Реализовать `AmplitudeInterceptor` в `src-tauri/src/audio/effects.rs` (или новом файле).
   * Подключить его к декодерам в `PlaybackManager` (в `playback.rs` при вызове `open_sink_on_device`).
3. **Этап III (Менеджер VTube Studio)**:
   * Создать `src-tauri/src/vts/manager.rs` с логикой WebSocket подключения, авторизации по токену и цикла отправки параметров.
   * Запустить его в `lib.rs` как часть глобального Tauri-стейта.
4. **Этап IV (Tauri-команды и UI)**:
   * Написать Tauri команды `vts_request_token`, `vts_test_connection`, `vts_save_settings`.
   * Добавить панель управления VTube Studio в Vue UI (раздел «Настройки» / «Интеграции»).

