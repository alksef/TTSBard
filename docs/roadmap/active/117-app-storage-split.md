---
id: ROADMAP-117
status: planned
created: 2026-10-01
updated: 2026-10-01
related_tasks: []
---

# ROADMAP-117 — Разделение данных приложения: Roaming, Local и %TEMP%

## Контекст

Все данные приложения пишутся в единый корень `%APPDATA%\ttsbard` (Roaming-профиль).
Более десятка мест строят этот путь независимо: `dirs::config_dir().join("ttsbard")`
(`src-tauri/src/history.rs`, `src-tauri/src/config/settings.rs`,
`src-tauri/src/ocr/service.rs`, `src-tauri/src/state.rs`, `src-tauri/src/tabs.rs`,
`src-tauri/src/webview/server.rs`, `src-tauri/src/commands/mod.rs`) плюс прямое
чтение `APPDATA` в `src-tauri/src/telegram/bot.rs` и `src-tauri/src/lib.rs` (логи).

Фактические объёмы на рабочей машине: `models` ~940 МБ, `logs` ~107 МБ,
`audio_cache` ~82 МБ, `temp` ~3 МБ. Roaming-расположение нарушает Windows-семантику
Known Folders: в доменных средах эти данные синхронизируются с сервером профиля.

Подтверждённые дефекты:

- временные файлы Telegram (`get_temp_dir` в `telegram/bot.rs`) пишутся в
  `%APPDATA%\ttsbard\temp`; итоговые файлы `silero_tts_<msg_id>.mp3` никогда не
  удаляются — чистятся только `.part` при сбое скачивания;
- `audio_cache` растёт без лимита и эвикции; при этом ключ детерминирован
  (hash текста, провайдера, голоса и эффектов), а файлы регенерируемы повторным
  синтезом;
- модели (`models/piper`, `models/ocr`, `models/ruaccent`, accentors) ищутся
  только в config dir; приложение их не скачивает и не распаковывает (ручное
  размещение), у пользователей фактически не используются — миграция не нужна;
- в `%LOCALAPPDATA%` уже есть следы приложения: ONNX Runtime ведёт собственный
  кеш в `TTSBard\ort-cache` (создаётся библиотекой, а не кодом приложения; на
  NTFS это та же папка, что и `ttsbard`), Tauri использует отдельный
  `com.al.ttsbard` для WebView-данных.

`espeak-ng-data` установщик кладёт рядом с exe (Tauri resource, 19 МБ,
`tauri.conf.json` → `espeak-ng-data`), поэтому установленная и portable-копии
используют один механизм; цепочка поиска resource_dir → cwd → папка с exe
(`tts/piper/runtime.rs::init_espeak_data`) — уже корректна и не требует
изменений.

## Цель

Данные разделены по семантике ОС, путь каждого вида определяется в одном месте:

- настройки, история и пользовательские шаблоны остаются в Roaming (`%APPDATA%\ttsbard`);
- регенерируемый аудио-кеш и модели живут в Local (`%LOCALAPPDATA%\ttsbard`);
- транзиентные файлы Telegram живут в `%TEMP%\ttsbard` секунды и удаляются
  детерминированно после потребления;
- пользователь может задать путь аудио-кеша в настройках, с переносом существующих
  файлов и fallback на дефолт при недоступности.

## Этапы

### 1. Центральный модуль путей и temp в %TEMP%

Ввести один модуль разрешения путей (config root, local root, temp root, корни
моделей), перевести на него существующие конструкции `dirs::config_dir()`.
Временные файлы Telegram переместить в `%TEMP%\ttsbard`, удалить файл сразу после
чтения в память (включая error-ветки после успешной загрузки, например
unsupported OGG codec), зачистить содержимое `%TEMP%\ttsbard` при старте
(безопасно благодаря single-instance). Одноразово удалить legacy-папку
`%APPDATA%\ttsbard\temp` по точному пути, best-effort.

### 2. Аудио-кеш в %LOCALAPPDATA%

Дефолт кеша — `%LOCALAPPDATA%\ttsbard\audio_cache`. Идемпотентная миграция при
старте: если существует `%APPDATA%\ttsbard\audio_cache`, переносить `*.wav`
по одному `fs::rename` (занятые пропускать), пустую старую папку удалить, ретрай
на следующем старте. CacheMiss остаётся естественным fallback: пропавший файл
замещается повторным синтезом, fallback-чтение из старой папки не вводится.
Формат `phrase_history.json` не меняется — записи ссылаются на `cache_key`, а не
на путь.

### 3. Модели в %LOCALAPPDATA% и рядом с exe

Корни поиска всех семейств моделей (piper, ocr, ruaccent, accentors): папка
рядом с exe (portable-сценарий), затем `%LOCALAPPDATA%\ttsbard\models`. Побочные
`create_dir_all` выполняются только в Local-корне, не рядом с exe. Миграция и
legacy-чтение из `%APPDATA%\ttsbard\models` не выполняются — модели у
пользователей не используются, перенос ручной. `resource_dir` как read-only корень
ruaccent сохраняется.

### 4. UI: путь кеша и папка данных

Секция «Хранилище» в General-настройках: текущий путь аудио-кеша, «Изменить…»
(нативный folder picker через `tauri-plugin-dialog`), «По умолчанию», кнопка
открытия папки данных `%LOCALAPPDATA%\ttsbard` в проводнике. Смена пути
применяет тот же идемпотентный перенос файлов, что и миграция, и только затем
пишет настройку (`storage.audio_cache_dir`). Валидация пути: canonicalize,
`create_dir_all`, тестовая запись; при недоступности настроенного пути при старте —
fallback на дефолт с warning в лог. Подпись существующей строки `%APPDATA%\ttsbard`
меняется на «Папка конфигурации». Размер кеша и кнопка очистки не показываются.

## Затрагиваемые места

- `src-tauri/src/telegram/bot.rs` — temp dir, удаление после потребления;
- `src-tauri/src/tts/silero.rs` — точка чтения и удаления файла;
- `src-tauri/src/history.rs` — `cache_dir_path`, миграция;
- `src-tauri/src/state.rs`, `src-tauri/src/ocr/service.rs`,
  `src-tauri/src/commands/mod.rs` — корни моделей;
- новый модуль путей и `src-tauri/src/lib.rs` (подключение, стартовые миграции);
- `src/components/settings/SettingsGeneral.vue` и локали — секция «Хранилище»;
- `src-tauri/src/config/settings.rs` — `storage.audio_cache_dir`.

## Порядок реализации

Этапы выполняются отдельными task и независимо проверяются, строго по порядку:
этап 1 открывает остальным центральный модуль путей; этапы 2–3 зависят от него,
этап 4 зависит от 2. Между этапами 2 и 3 нет зависимости.

## Критерии результата

- После сессии синтеза через Telegram в `%TEMP%\ttsbard` нет файлов; legacy
  `%APPDATA%\ttsbard\temp` удалена и не создаётся вновь.
- `audio_cache` читается и пишется в `%LOCALAPPDATA%\ttsbard\audio_cache`;
  после первой миграции в `%APPDATA%\ttsbard` папки `audio_cache` нет, история
  фраз воспроизводится без повторного синтеза.
- Модели обнаруживаются рядом с exe и в `%LOCALAPPDATA%\ttsbard\models`;
  пустые каталоги рядом с exe не создаются.
- Смена пути кеша в UI переносит файлы и сохраняет настройку; недоступный путь
  при старте не блокирует запуск — используется дефолт и warning.
- Проходят целевые Rust-тесты через `scripts/cargo.ps1`, `npm test` и
  `npm run build` для затронутого frontend.

## Не входит

- Логи: ротация и перенос из Roaming — отдельное направление.
- LRU-эвикция и лимит размера кеша, отображение размера и кнопка очистки —
  осознанно исключены; возвращаются вместе с функцией, которой служат.
- Скачивание и установка моделей приложением, очистка orphan-каталогов
  (`models/pocket`, остаточные zip-архивы) на пользовательских машинах.
- Изменение расположения `espeak-ng-data` — установщик уже кладёт его рядом с
  exe, установленная и portable-копии используют один и тот же механизм.
- Перенос soundpanel-звуков и webview-шаблонов — пользовательские и конфигурационные
  данные Roaming-семантики.
