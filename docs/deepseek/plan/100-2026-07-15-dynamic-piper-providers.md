# План 100: Динамические Piper TTS-провайдеры

**Дата:** 2026-07-15  
**Тип:** feature / backend + settings + UI  
**Источник:** `docs/stage/36-dynamic-piper-tts-providers.md`

## Цель

Добавить Piper-compatible голосовые модели как отдельные TTS-провайдеры.
При старте приложение сканирует `%APPDATA%\\TTSBard\\models\\piper`, а при
выборе провайдера лениво загружает модель. Целевой пользовательский артефакт —
один `ttsbard.exe`, без отдельного Piper-процесса, HTTP-сервера, Python и DLL.

## Зафиксированные решения

- Текущий `LocalTts` переименовать в `LocalHttpServerTts`; его HTTP-контракт не
  менять.
- Новый runtime-провайдер называется `LocalModelTts`.
- Каждая валидная пара `.onnx` + `.onnx.json` — отдельный provider ID.
- Каталог моделей: `%APPDATA%\\TTSBard\\models\\piper`.
- Модели обнаруживаются при старте; добавление в UI не требуется.
- ONNX session создаётся при выборе модели, не при сканировании.
- Во время загрузки UI показывает индикатор; после успеха — готовность.
- Неполные/невалидные модели пропускаются и пишутся в лог.
- Если выбранный provider ID отсутствует, выбирается первый доступный провайдер,
  выбор сохраняется и показывается глобальное уведомление.
- Первый этап поддерживает только Piper-compatible ONNX с соседним JSON.
- Статическая линковка Piper/ONNX Runtime обязательна для целевого варианта;
  DLL допустима только как временный fallback при технической невозможности.

## Порядок реализации и коммиты

Этапы выполнять последовательно, каждый отдельным коммитом:

1. `refactor: rename local HTTP TTS provider` — только переименование классов,
   файлов/модулей, импортов и диагностических сообщений; поведение не менять.
2. `feat: discover Piper models from AppData` — каталог, описание модели,
   сканирование и стабильные IDs; без реального inference.
3. `feat: add embedded Piper model runtime` — статический runtime, открытие
   модели и синтез WAV через существующий audio pipeline.
4. `feat: add lazy Piper provider loading` — lifecycle `Discovered/Loading/Ready/
   Failed`, кэш загруженных sessions, async-safe доступ из TTS pipeline.
5. `feat: register dynamic Piper providers` — объединить встроенные и найденные
   провайдеры, persist выбранного provider ID и startup fallback.
6. `feat: show dynamic Piper providers in TTS panel` — динамические карточки,
   loading/ready/error индикаторы и выбор модели.
7. `test: validate Piper provider startup and fallback` — тесты каталогов,
   повреждённых JSON, отсутствующего выбранного ID, переключения и runtime
   ошибок; затем независимые `cargo check` и `vue-tsc --noEmit`.

## Декомпозиция DeepSeek-задач

Не запускать несколько build-агентов одновременно в одном worktree: задачи
пересекаются через `TtsProvider`, settings DTO, `AppState` и TTS panel. На каждом
этапе DeepSeek получает только текущую задачу, после чего Codex читает diff,
запускает проверки и создаёт следующую задачу по результатам ревью.

Параллельно допустимы только read-only исследования, не изменяющие рабочее
дерево: проверка native/static ONNX Runtime под Windows и анализ текущей модели
инициализации TTS.

## Критерии готовности

- Существующий Local HTTP provider работает без изменения API.
- Модель из `models/piper` появляется отдельным провайдером после перезапуска.
- Модель не загружается до выбора.
- При выборе виден loading, затем ready либо понятная ошибка.
- Удаление выбранной модели не ломает запуск: выбран fallback-провайдер.
- Произвольный `.onnx` без Piper JSON не появляется в списке.
- Один TTS-запрос использует выбранный Piper provider и проходит общий audio
  effects/playback pipeline.
- Сборка не требует отдельного Piper exe, Python или DLL рядом с приложением.
