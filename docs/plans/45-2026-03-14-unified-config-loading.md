# План: Унифицированная загрузка конфигурации (get_all_app_settings)

**Дата:** 2026-03-14
**Номер:** 45
**Статус:** 🔄 Planned
**Приоритет:** High

## Описание

Реализовать единую точку входа для загрузки всех настроек приложения вместо множества отдельных `invoke` вызовов. Это устранит race conditions при загрузке конфигурации в release режиме и упростит архитектуру.

## Проблема

### Текущая ситуация

**Множественные источники данных:**
- `get_webview_settings` → AppState.webview_settings (память)
- `get_twitch_settings` → AppState.twitch_settings (память)
- `get_floating_appearance` → WindowsManager (файл)
- `sp_get_floating_appearance` → WindowsManager (файл)
- `get_audio_settings` → config::AudioSettings (файл)
- `get_logging_settings` → config::LoggingSettings (файл)
- ...и еще 15+ команд

**Race condition в release:**
```
Timeline release:
1. Tauri создает окна (быстро, без devtools)
2. Frontend mount'ится (быстро)
3. invoke() команды → данные ЕЩЕ НЕ загружены ❌
4. setup.rs завершается → данные загружены (поздно)
```

**Каждый компонент делает свои invoke:**
- SettingsPanel.vue: 3 invoke
- AudioPanel.vue: 1 invoke
- FloatingPanel.vue: 2 invoke
- TtsPanel.vue: 4+ invoke
- WebViewPanel.vue: 1 invoke
- TwitchPanel.vue: 1 invoke
- InputPanel.vue: 2 invoke
- **ИТОГО: 14+ invoke вызовов при старте**

## Решение

Единая команда `get_all_app_settings` которая:
1. Загружает все настройки одним вызовом
2. Гарантирует консистентность данных (из одного момента времени)
3. Устраняет race conditions
4. Упрощает отладку и тестирование

## Структура AppSettingsDto

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettingsDto {
    // TTS настройки
    tts: TtsSettingsDto,

    // WebView настройки
    webview: WebViewSettingsDto,

    // Twitch настройки
    twitch: TwitchSettingsDto,

    // Окна и внешний вид
    windows: WindowsSettingsDto,

    // Аудио
    audio: AudioSettingsDto,

    // Прочее
    general: GeneralSettingsDto,

    // Логирование
    logging: LoggingSettingsDto,

    // Препроцессор
    preprocessor: PreprocessorSettingsDto,
}
```

## План реализации

### Phase 1: Backend (Rust)

#### Step 1.1: Создать DTO структуры
**Файл:** `src-tauri/src/config/dto.rs`

- [ ] Создать модуль `dto.rs`
- [ ] Определить `AppSettingsDto` и все под-структуры
- [ ] Добавить `Serialize, Deserialize` derives
- [ ] Добавить conversion функции (`From`, `Into`)

#### Step 1.2: Реализовать команду get_all_app_settings
**Файл:** `src-tauri/src/commands/mod.rs`

```rust
#[tauri::command]
pub async fn get_all_app_settings(
    app_state: State<'_, AppState>,
    windows_manager: State<'_, WindowsManager>,
    settings_manager: State<'_, SettingsManager>,
) -> Result<AppSettingsDto, String> {
    // Загрузить все настройки
}
```

- [ ] Создать команду `get_all_app_settings`
- [ ] Собрать данные из всех источников
- [ ] Добавить подробное логирование
- [ ] Обработать ошибки gracefully

#### Step 1.3: Зарегистрировать команду
**Файл:** `src-tauri/src/lib.rs`

- [ ] Добавить `get_all_app_settings` в `invoke_handler!`
- [ ] Убедиться что порядок правильный

#### Step 1.4: Добавить флаг готовности
**Файл:** `src-tauri/src/state.rs`

- [ ] Добавить `backend_ready: Arc<AtomicBool>` в `AppState`
- [ ] Инициализировать в `false` при создании

#### Step 1.5: Установить флаг в конце setup
**Файл:** `src-tauri/src/setup.rs`

- [ ] В самом конце `init_app` установить `backend_ready = true`
- [ ] Добавить логирование "Backend ready"

#### Step 1.6: Команда для проверки готовности
**Файл:** `src-tauri/src/commands/mod.rs`

```rust
#[tauri::command]
pub async fn is_backend_ready(state: State<'_, AppState>) -> bool {
    state.backend_ready.load(Ordering::SeqCst)
}
```

- [ ] Создать команду `is_backend_ready`
- [ ] Вернуть состояние флага

#### Step 1.7: Команда подтверждения готовности
**Файл:** `src-tauri/src/commands/mod.rs`

```rust
#[tauri::command]
pub async fn confirm_backend_ready(app_handle: AppHandle) -> Result<(), String> {
    if is_backend_ready() {
        let _ = app_handle.emit("backend-ready", &());
    }
    Ok(())
}
```

- [ ] Создать команду `confirm_backend_ready`
- [ ] Отправить событие если уже готово

### Phase 2: Frontend (TypeScript/Vue)

#### Step 2.1: Создать TypeScript типы
**Файл:** `src/types/settings.ts`

- [ ] Создать интерфейс `AppSettingsDto`
- [ ] Создать все под-интерфейсы
- [ ] Экспортировать для использования в компонентах

#### Step 2.2: Создать composable для настроек
**Файл:** `src/composables/useAppSettings.ts`

```typescript
export function useAppSettings() {
  const settings = ref<AppSettingsDto | null>(null)
  const isLoading = ref(false)
  const error = ref<string | null>(null)

  async function load() { ... }
  async function waitForReady() { ... }

  return { settings, isLoading, error, load, waitForReady }
}
```

- [ ] Создать composable
- [ ] Добавить реактивность
- [ ] Добавить обработку ошибок
- [ ] Добавить логирование

#### Step 2.3: Обновить App.vue
**Файл:** `src/App.vue`

- [ ] Импортировать `useAppSettings`
- [ ] Загрузить настройки при старте
- [ ] Provide через inject для компонентов
- [ ] Показать loading state

#### Step 2.4: Создать Settings Store (Pinia)
**Файл:** `src/stores/settings.ts`

```typescript
export const useSettingsStore = defineStore('settings', () => {
  const allSettings = ref<AppSettingsDto | null>(null)

  function setAllSettings(settings: AppSettingsDto) {
    allSettings.value = settings
  }

  // Getters для различных секций
  const tts = computed(() => allSettings.value?.tts)
  const webview = computed(() => allSettings.value?.webview)
  // ...и т.д.
})
```

- [ ] Создать Pinia store
- [ ] Добавить computed getters
- [ ] Добавить actions для обновления

#### Step 2.5: Обновить компоненты (по одному)
**Файлы:** Все компоненты в `src/components/`

**Компоненты для обновления:**
- [ ] SettingsPanel.vue
- [ ] AudioPanel.vue
- [ ] FloatingPanel.vue
- [ ] TtsPanel.vue
- [ ] WebViewPanel.vue
- [ ] TwitchPanel.vue
- [ ] InputPanel.vue
- [ ] PreprocessorPanel.vue

**Для каждого компонента:**
- [ ] Удалить локальные `loadSettings()` функции
- [ ] Использовать store или inject
- [ ] Убрать `invoke` вызовы для чтения
- [ ] Оставить `invoke` вызовы для записи
- [ ] Протестировать

#### Step 2.6: Обновить SoundPanel
**Файл:** `src-soundpanel/SoundPanelApp.vue`

- [ ] Использовать `waitForBackendReady()` перед загрузкой
- [ ] Загружать данные через store если применимо
- [ ] Протестировать отдельно

### Phase 3: События обновления

#### Step 3.1: Определить события
**Файл:** `src-tauri/src/events.rs`

События уже существуют, нужно использовать:
- `TtsProviderChanged`
- `FloatingAppearanceChanged`
- `WebViewServerRestarted` (если нужно)
- `TwitchStatusChanged`

#### Step 3.2: Обновить store при событиях
**Файл:** `src/stores/settings.ts`

- [ ] Слушать события обновления
- [ ] Обновлять соответствующие секции в store
- [ ] Реактивность Vue автоматически обновит UI

### Phase 4: Обратная совместимость

#### Step 4.1: Оставить старые команды
- [ ] НЕ удалять старые `get_*` команды
- [ ] Они используются для частичных обновлений
- [ ] Добавить deprecation warning в документацию

#### Step 4.2: Добавить миграцию
- [ ] Если настройки в старом формате - мигрировать
- [ ] Логировать миграцию

### Phase 5: Тестирование

#### Step 5.1: Unit тесты
**Файл:** `src-tauri/tests/settings_test.rs`

- [ ] Тест `get_all_app_settings` возвращает все данные
- [ ] Тест консистентности данных
- [ ] Тест ошибок

#### Step 5.2: Интеграционные тесты
- [ ] Тест загрузки в dev режиме
- [ ] Тест загрузки в release режиме
- [ ] Тест race conditions

#### Step 5.3: Ручное тестирование
**Чек-лист:**
- [ ] Все настройки загружаются при старте
- [ ] Белый экран не появляется
- [ ] Все галки в правильном состоянии
- [ ] Цвета загружаются корректно
- [ ] Биндинги звуковой панели загружаются
- [ ] События обновления работают
- [ ] Сохранение настроек работает

### Phase 6: Документация

#### Step 6.1: Обновить CLAUDE.md
- [ ] Добавить описание новой архитектуры
- [ ] Обновить guidelines для работы с конфигом

#### Step 6.2: Создать гайд для контрибьюторов
**Файл:** `docs/SETTINGS_ARCHITECTURE.md`

- [ ] Описать как добавлять новые настройки
- [ ] Описать как обновлять DTO
- [ ] Примеры использования

#### Step 6.3: Обновить CHANGELOG
- [ ] Добавить запись о новых возможностях
- [ ] Описать breaking changes

## Приемка

### Критерии успеха

1. ✅ Все настройки загружаются одним invoke вызовом
2. ✅ Нет race conditions в release режиме
3. ✅ Все компоненты используют единый источник данных
4. ✅ События обновления работают корректно
5. ✅ Обратная совместимость сохранена
6. ✅ Производительность не ухудшена
7. ✅ Покрыты тестами основные сценарии

### Метрики

- Количество invoke вызовов при старте: **14+ → 1**
- Время загрузки UI: **не должно увеличиться**
- Размер передаваемых данных: **измерить**
- Количество race condition багов: **0**

## Риски

### Высокий приоритет
- **Сложность миграции:** Много компонентов нужно обновить
- **Тестирование:** Нужно тщательно протестировать все сценарии

### Средний приоритет
- **Производительность:** Большой DTO может тормонить на медленных системах
- **Совместимость:** Старые плагины/скрипты могут сломаться

### Низкий приоритет
- **Поддержка:** Новая архитектура сложнее для понимания новичками

## Альтернативы

### Альтернатива A: Pinia store без unified DTO
- Меньше изменений
- Остаются race conditions

### Альтернатива B: Только флаг backend_ready
- Минимальные изменения
- Не устраняет множественные invoke

### Альтернатива C: Hybrid подход
- Главная окно использует `get_all_app_settings`
- SoundPanel использует старые команды
- **РЕКОМЕНДУЕТСЯ** как промежуточный этап

## Зависимости

- [ ] Нет блокирующих зависимостей

## Сроки

- **Phase 1 (Backend):** 2-3 часа
- **Phase 2 (Frontend):** 4-6 часов
- **Phase 3 (Events):** 1-2 часа
- **Phase 4 (Compatibility):** 1 час
- **Phase 5 (Testing):** 2-3 часа
- **Phase 6 (Docs):** 1-2 часа

**ИТОГО:** 11-17 часов разработки

## Примечания

- Начать с Phase 1, протестировать backend отдельно
- Можно делать поэтапно, оставляя старые команды
- SoundPanel можно сделать последним этапом
- Добавить подробное логирование для отладки
