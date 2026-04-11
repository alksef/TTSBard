# План: Рефакторинг MINOR замечаний

## Обзор
Рефакторинг 8 некритичных замечаний.

## Шаги

### 1. Убрать бесполезную загрузку настроек в get_proxy_settings
- **Файл**: `src-tauri/src/commands/proxy.rs:189`
- **Действие**: Убрать `_settings.load()`, если `get_proxy_url()` и `get_proxy_type()` работают с кэшем без необходимости явной загрузки.

### 2. Заменить reqwest::get на переиспользуемый Client
- **Файл**: `src-tauri/src/commands/webview.rs:332`
- **Действие**: Использовать переиспользуемый `reqwest::Client` вместо `reqwest::get`, который создаёт новый клиент на каждый вызов.

### 3. Убрать `as any` в SoundPanel main.ts
- **Файл**: `src-soundpanel/main.ts:11`
- **Действие**: Определить интерфейс с методом `showNoBinding` и заменить `as any` на типизированный assertion.

### 4. Добавить cleanup setTimeout в useErrorHandler
- **Файл**: `src/composables/useErrorHandler.ts:68`
- **Действие**: Хранить ID таймаутов для автоудаления ошибок и очищать их при необходимости. Низкий приоритет — singleton, но для чистоты кода стоит сделать.

### 5. Добавить обработку ошибок в FishAudioModelPicker.loadImages
- **Файл**: `src/components/tts/FishAudioModelPicker.vue:33-40`
- **Действие**: Добавить `.catch()` для промисов загрузки изображений.

### 6. Убрать отладочный watch в InputPanel
- **Файл**: `src/components/InputPanel.vue:82-89`
- **Действие**: Удалить watch на `editorSettings`, который только логирует значение и не используется.

### 7. Добавить проверку unlisten в SoundPanelTab
- **Файл**: `src/components/SoundPanelTab.vue:220-237`
- **Действие**: Добавить проверку `if (unlisten)` перед вызовом в `onUnmounted`, чтобы избежать ошибки если инициализация не завершилась.

### 8. Обработать env_filter parse error
- **Файл**: `src-tauri/src/lib.rs:112`
- **Действие**: Заменить `.parse().expect("Invalid log directive")` на обработку ошибки с warning логом и пропуском невалидной директивы.
