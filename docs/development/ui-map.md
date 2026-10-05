# Карта UI

Навигация по интерфейсу для исследования и постановки локальных задач.
Карта описывает исходники, а не подтверждает прохождение runtime-проверок.
Правила проверки и статус визуальных ориентиров — в [UI guidelines](./ui-guidelines.md).
Владельцы CSS, исключения и группы излишней кастомизации — в
[реестре CSS](./ui-css-ownership.md).

## Основное окно

Вход: `src/main.ts` → `src/App.vue`. Навигация: `src/components/Sidebar.vue`.
Идентификаторы панелей определены типом `Panel` в этих компонентах.
Пути в таблице относительно `src/components/`.

| ID | Экран | Компонент | Вложенные поверхности |
|---|---|---|---|
| `input` | Ввод | `InputPanel.vue` | `editor/TtsEditor.vue`, `EditorTabs.vue`, `EditorMenu.vue`, `RouteSelector.vue`, `IncomingTextsTab.vue`, `IncomingRouteSelector.vue`, `SpellContextMenu.vue`, `PhraseHistoryList.vue` |
| `tts` | Синтез речи | `TtsPanel.vue` | `tts/TtsOpenAICard.vue`, `TtsSileroCard.vue`, `TtsLocalCard.vue`, `TtsFishAudioCard.vue`, `TtsElevenLabsCard.vue`, `VoiceSelector.vue`, `FishAudioModelPicker.vue`, `TelegramConnectionStatus.vue`, `TelegramAuthModal.vue` |
| `audio` | Аудио | `AudioPanel.vue` | `audio/AudioDevicesTab.vue`, `AudioEffectsTab.vue`, `AudioPreviewBar.vue`, `DspSettings.vue`, `EqSettings.vue`, `CompressorSettings.vue`, `LimiterSettings.vue`, `EffectsSettings.vue` |
| `preprocessor` | Препроцессор | `PreprocessorPanel.vue` | Правила обработки и их условные настройки |
| `webview` | WebView | `WebViewPanel.vue` | Сервер, адреса, отображение текста, UPnP |
| `twitch` | Twitch | `TwitchPanel.vue` | Подключение и настройки отправки |
| `vtube-studio` | VTube Studio | `VTubeStudioPanel.vue` | Адрес/порт, параметры и действия при наборе, тестирование |
| `ocr` | OCR | `OcrPanel.vue` | Настройки распознавания, выбор экрана захвата и выбор области |
| `input-server` | Входящий сервер | `InputServerPanel.vue` | Подключение и маршрутизация входящего текста |
| `hotkeys` | Горячие клавиши | `HotkeysPanel.vue` | Назначение сочетаний |
| `intercept` | Перехват | `InterceptPanel.vue` | Настройки перехвата |
| `settings` | Настройки | `SettingsPanel.vue` | Вкладки ниже |

## Настройки

| ID вкладки | Компонент | Связанные поверхности |
|---|---|---|
| `general` | `src/components/settings/SettingsGeneral.vue` | `DataTransferModal.vue` в том же каталоге |
| `interface` | `src/components/settings/SettingsInterface.vue` | Тема и внешний вид окон |
| `editor` | `src/components/settings/SettingsEditor.vue` | `EditorFontSettings.vue` в том же каталоге |
| `network` | `src/components/settings/SettingsNetwork.vue` | Сетевые формы |
| `ai` | `src/components/SettingsAiPanel.vue` | Настройки AI |

## Общие поверхности и отдельные окна

| Поверхность | Источник |
|---|---|
| Titlebar и переключение дополнительных окон | `src/App.vue` |
| Статусы интеграций | `src/components/titlebar/IntegrationStatusCluster.vue` |
| Уведомления | `src/components/ErrorToasts.vue`, `shared/StatusMessage.vue`, `shared/TestResult.vue` в `src/components/` |
| Минимальный режим | `src/components/MinimalModeButton.vue`, `src/composables/compactModeState.ts`, `src/components/InputPanel.vue` |
| Звуковая панель | `src-soundpanel/main.ts` → `SoundPanelApp.vue` в том же каталоге |
| Управление воспроизведением | `src-playback/main.ts` → `PlaybackControlApp.vue` в том же каталоге |
| Выбор области OCR | `src-ocr-selection/main.ts` → `SelectionApp.vue` в том же каталоге |
| Браузерный overlay | `src-tauri/src/input_server/overlay.html` |

## Где искать оформление и поведение

- Главное окно: `src/style.css` и `src/styles/index.css`; цвета и переменные — `src/styles/variables.css`; базовые стили — `base.css`; общий фокус — `focus.css` в `src/styles/`.
- Согласованные размеры и отступы основных форм — `src/styles/ui-tokens.css`; общий слой явно подключаемых классов `ui-*` — `src/styles/ui-controls.css`. Переведены на слой: вкладки настроек и аудио, все вкладки настроек, TTS-карточки и диалоги, OCR, препроцессор, перехват и горячие клавиши (обычные роли), аудио-устройства/эффекты (обычные роли), интеграции, шрифтовые пикеры. Компактные поверхности редактора/истории, титлбар и отдельные окна сохраняют собственную плотность (предложение шкалы — в ROADMAP-121); глобальный `<style>` в `AudioPanel.vue` остаётся владельцем стилей плотных DSP-подсекций.
- Общие контролы: `src/components/shared/InputWithToggle.vue`, `ProviderCard.vue`, `StatusMessage.vue`, `TestResult.vue` в том же каталоге.
- DSP: `src/components/audio/dsp-shared.css`. В `AudioPanel.vue` есть также **глобальный** блок `<style>`: проверять его влияние на совпадающие классы других панелей.
- Отдельные окна имеют собственные entrypoints: наличие стиля в главном окне не доказывает его наличие в них.
- Владельцы настроек и runtime-состояний: `src/composables/`; контракты — `src/types/settings.ts`, `src/ipc/`; переводы — `src/i18n/`.

При добавлении/переносе экрана обновлять соответствующую строку. Перед правкой
найти фактический класс и условия показа в исходнике; номера строк в карте не
фиксируются, поскольку быстро устаревают. Ограничить разрешённые файлы в task
по [AI workflow](./ai-workflow.md).

## Восстановление настроек

| Поверхность | Исходники | Условие открытия |
|---|---|---|
| Окно восстановления настроек | `src-settings-recovery/main.ts` → `src-settings-recovery/SettingsRecoveryApp.vue`; backend: `src-tauri/src/settings_recovery.rs` | Ошибка загрузки `settings.json` (ROADMAP-123) |

Окно описано в `src-tauri/tauri.conf.json`, доступы — в
`src-tauri/capabilities/settings-recovery.json`.
