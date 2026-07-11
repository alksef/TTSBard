# Stage 19: Кастомный OpenAI-совместимый ИИ провайдер

**Дата:** 2026-07-11  
**Статус:** Запланировано к реализации  
**Источник:** Запрос пользователя  

---

## 1. Цель и требования

Добавить поддержку кастомного ИИ провайдера (совместимого с OpenAI API), чтобы пользователь мог использовать локальный прокси (`self-ai-proxy`) или любую другую модель (например, DeepSeek через прокси-сервер), задав API URL, API-ключ/токен и имя модели.

### Требования к реализации:
1. Возможность задать кастомный `base_url` (API URL) и `api_key` (API Токен) наряду с моделью и прокси-настройками.
2. Поддержка в бэкенде через `async-openai` (аналогично OpenAI / DeepSeek).
3. Интеграция в настройки (`src/components/SettingsAiPanel.vue`) с удобным UI для ввода полей.
4. Обновление TypeScript/Rust DTO и структур конфигурации.

---

## 2. Предлагаемые изменения (план)

### 2.1 Backend (Rust)

1. **`src-tauri/src/config/settings.rs`**
   - Добавить `Custom` в `AiProviderType` (`custom` при сериализации).
   - Создать структуру `AiCustomSettings` с полями:
     - `url: Option<String>` (API URL, по умолчанию `Some("https://api.openai.com/v1".to_string())` или `None`)
     - `api_key: Option<String>`
     - `use_proxy: bool`
     - `model: String` (по умолчанию `"deepseek-chat"`)
   - Добавить `custom: AiCustomSettings` в `AiSettings` и обновить `Default` реализацию.
   - Добавить методы для мутации настроек в `SettingsManager`:
     - `set_ai_custom_url`
     - `set_ai_custom_api_key`
     - `set_ai_custom_model`
     - `set_ai_custom_use_proxy`

2. **`src-tauri/src/config/dto.rs`**
   - Добавить `Custom` в `AiProviderTypeDto`.
   - Создать `AiCustomSettingsDto`.
   - Реализовать конвертацию `From`/`Into` для новых типов.

3. **`src-tauri/src/ai/mod.rs`**
   - Добавить `custom` модуль.
   - Добавить `Custom(custom::CustomClient)` в enum `AiProvider`.
   - Обновить `create_ai_client` фабрику.
   - Обновить `hash_ai_settings` для хэширования настроек `custom` провайдера.

4. **`src-tauri/src/ai/custom.rs`**
   - Создать файл клиента (на основе `openai.rs`), инициализируя `OpenAIConfig` с кастомным `url` через `with_api_base`.

5. **`src-tauri/src/commands/ai.rs`**
   - Реализовать Tauri-команды для сохранения полей `custom` настроек:
     - `set_ai_custom_url`
     - `set_ai_custom_api_key`
     - `set_ai_custom_model`
     - `set_ai_custom_use_proxy`
   - Зарегистрировать их в `src-tauri/src/lib.rs`.

---

## 2.2 Frontend (Vue & TypeScript)

1. **`src/types/settings.ts`**
   - Обновить `AiProviderType` (добавить `Custom: 'custom'`).
   - Создать интерфейс `AiCustomSettingsDto` с полями `url`, `api_key`, `use_proxy`, `model`.
   - Обновить `AiSettingsDto` (добавить `custom: AiCustomSettingsDto`).

2. **`src/components/SettingsAiPanel.vue`**
   - Добавить блок UI для кастомного провайдера.
   - Поля ввода: API URL, API Токен, Модель, Чекбокс использования прокси.
   - Реализовать функции сохранения каждого поля / группы полей.

3. **`src/components/InputPanel.vue`**
   - Добавить проверку наличия ключа для `custom` провайдера.

---

## 3. Критерии приемки

1. Проект собирается (`cargo check` + `npx vue-tsc --noEmit`).
2. В интерфейсе настроек AI доступен "Кастомный провайдер" (Custom Provider).
3. Настройки корректно сохраняются в файл конфигурации приложения.
4. При переключении на Custom провайдер запросы на исправление текста отправляются на указанный API URL с заданным токеном и моделью.
