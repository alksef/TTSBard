# План: Рефакторинг TTSVoiceWizard Local

**Дата:** 2026-03-07
**Номер:** 28
**Статус:** Запланировано

## Описание

Обновление провайдера "TTSVoiceWizard Local" для повторения интеграции аналогичной режиму "Locally Hosted" в TTS Voice Wizard. Провайдер будет отправлять HTTP запросы к локальному серверу TTS по тому же API (например, TITTS.py на Python).

## Анализ

### TTS Voice Wizard - режим "Locally Hosted"

Проанализирован исходный код TTS Voice Wizard (C#):
- **Режим:** "Locally Hosted" в списке TTS методов
- **Настройки:** `LocalHostedTTSAddress` (default: "127.0.0.1"), `LocalHostedTTSPort` (default: "8124")
- **Endpoint:** `GET /synthesize/{text}` или `GET /synthesize/?text={text}`
- **Ответ:** base64 закодированные WAV данные как `text/plain`
- **Файл:** `OSCVRCWiz/Services/Speech/TextToSpeech/TTSEngines/GladosTTS.cs`

### TITTS.py - эталонная реализация сервера

Проанализирован Python скрипт TITTS.py - это эталонная реализация HTTP сервера для интеграции:
- **Эндпоинт:** `GET /synthesize/` или `GET /synthesize/<text>`
- **Параметры:** текст передается через URL path или query параметр `?text=...`
- **Обработка:** num2words (цифры → слова), Mystem (коррекция рода числительных)
- **Интеграция:** отправляет текст в Telegram бот @silero_voice_bot, получает голосовое OGG
- **Конвертация:** OGG → WAV (128kHz, 16-bit, mono) → base64
- **Параллельно:** отправляет текст в Twitch чат
- **Ответ:** base64 WAV как `text/plain`
- **По умолчанию:** `http://127.0.0.1:8124`

### Текущее состояние

- UI название: "TTSVoiceWizard (Local)" → **изменить на** "Local (TTSVoiceWizard - Locally Hosted)"
- Реализация в `local.rs`: заглушка (не реализован синтез)
- Default URL: `http://localhost:5002` → **изменить на** `http://127.0.0.1:8124`
- Описание в UI: отсутствует информация о назначении провайдера

## Задачи

### 1. UI: Переименование провайдера

**Файл:** `src/components/TtsPanel.vue`

- [ ] Изменить `card-title`: "TTSVoiceWizard (Local)" → "Local (TTSVoiceWizard - Locally Hosted)"
- [ ] Добавить subtitle/описание: "Обратная совместимость с TTSVoiceWizard. Запросы к {url}"
- [ ] Отобразить текущий URL в описании карточки

### 2. Backend: Реализация LocalTts

**Файл:** `src-tauri/src/tts/local.rs`

- [ ] Добавить поле `server_url: String` в структуру `LocalTts`
- [ ] Реализовать метод `synthesize()`:
  - [ ] HTTP GET запрос к `{server_url}/synthesize/{url_encoded_text}`
  - [ ] Парсинг ответа (base64 WAV)
  - [ ] Декодирование base64 в `Vec<u8>`
  - [ ] Обработка ошибок (timeout, connection refused, invalid response)
- [ ] Добавить метод `set_url(&mut self, url: String)`

### 3. State: Инициализация с URL

**Файл:** `src-tauri/src/state.rs`

- [ ] Изменить `init_local_tts()` для принятия URL параметра
- [ ] Добавить поле `local_tts_url: Arc<Mutex<String>>` в `AppState`
- [ ] Добавить методы `get_local_tts_url()` и `set_local_tts_url()`

### 4. Настройки: Default URL

**Файл:** `src-tauri/src/config/settings.rs`

- [ ] Изменить default URL: `"http://127.0.0.1:8124"`
- [ ] Обновить документацию

### 5. Commands: Связь State и Settings

**Файл:** `src-tauri/src/commands/mod.rs`

- [ ] Обновить `set_local_tts_url` для обновления State
- [ ] При изменении URL переинициализировать LocalTts если он активен

### 6. UI: Отображение описания

**Файл:** `src/components/TtsPanel.vue`

- [ ] Добавить computed свойство для отображения описания
- [ ] Показывать URL в карточке: "Запросы к http://127.0.0.1:8124"
- [ ] Добавить small text под заголовком

## Зависимости

Нет

## Риски

1. **Backward compatibility:** Изменение default URL может сломать существующие установки
   - **Митигация:** Оставить `http://localhost:5002` как fallback или добавить миграцию

2. **Error handling:** Сервер может быть недоступен
   - **Митигация:** Понятные сообщения об ошибках в UI

## Тестирование

- [ ] Проверить работу с запущенным сервером TTSVoiceWizard
- [ ] Проверить поведение при недоступном сервере
- [ ] Проверить сохранение/загрузку URL
- [ ] Проверить переключение провайдеров
