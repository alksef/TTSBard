# План: Рефакторинг AI клиентов (вынос общей логики + configurable model для OpenAI)

**Дата:** 2026-03-22
**Номер:** 56
**Задача:** Рефакторинг AI клиентов для уменьшения дублирования кода и добавления configurable model для OpenAI

---

## Обзор

Текущая реализация AI клиентов (`openai.rs` и `zai.rs`) содержит значительное дублирование кода (~70 строк). Также OpenAI клиент использует hardcoded константу для модели вместо настройки из конфига.

**Проблемы:**
1. OpenAI client использует `const DEFAULT_MODEL` вместо поля `model` из настроек
2. Дублирование логики в `AiClient::correct()` реализации
3. Дублирование параметров запроса (`temperature`, `max_tokens`)
4. Дублирование логики извлечения контента из ответа
5. Дублирование константы `DEFAULT_TIMEOUT`

**Цели:**
1. Добавить поле `model` для OpenAI (как у Z.ai)
2. Вынести общую логику в модуль `common.rs`
3. Уменьшить дублирование кода на ~70 строк
4. Обеспечить обратную совместимость
5. **БЕЗ изменений в UI** (только backend)

---

## Текущее состояние

### Дублирование кода

| Метод/Логика | openai.rs | zai.rs | Дублирование |
|-------------|-----------|--------|--------------|
| `AiClient::correct()` | 24 строки | 24 строки | Идентично (кроме логов) |
| Параметры запроса | `temperature(0.7)`, `max_tokens(4096)` | `temperature(0.7)`, `max_tokens(4096)` | Идентично |
| Извлечение контента | 8 строк | 8 строк | Идентично |
| `DEFAULT_TIMEOUT` | `30` | `30` | Идентично |
| **Итого дублирования** | | | **~70 строк** |

### Структуры клиентов

**OpenAI client** (`openai.rs:26-28`):
```rust
pub struct OpenAiClient {
    client: Client<OpenAIConfig>,
    // ❌ Нет поля model (использует константу DEFAULT_MODEL)
}
```

**Z.ai client** (`zai.rs:27-30`):
```rust
pub struct ZAiClient {
    client: Client<OpenAIConfig>,
    model: String,  // ✅ Правильно - модель из настроек
}
```

---

## Файлы для изменения

### Backend (Rust)

| Файл | Изменения |
|------|-----------|
| `src-tauri/src/ai/common.rs` | **НОВЫЙ** - общие константы и функции |
| `src-tauri/src/ai/mod.rs` | Добавить `pub mod common` |
| `src-tauri/src/ai/openai.rs` | Добавить поле `model`, использовать `common` |
| `src-tauri/src/ai/zai.rs` | Использовать `common` |
| `src-tauri/src/config/settings.rs` | Добавить `model` в `AiOpenAiSettings`, getter/setter |
| `src-tauri/src/config/dto.rs` | Добавить `model` в `AiOpenAiSettingsDto` |
| `src-tauri/src/commands/ai.rs` | Добавить команды `set_ai_openai_model`, `set_ai_zai_model` |
| `src-tauri/src/lib.rs` | Зарегистрировать новые команды |

### Frontend (TypeScript) - минимальные изменения

| Файл | Изменения |
|------|-----------|
| `src/types/settings.ts` | Добавить `model?: string` в `AiOpenAiSettingsDto` |

**Примечание:** UI изменения НЕ требуются.

---

## Детальные изменения

### 1. Создать `src-tauri/src/ai/common.rs`

```rust
//! Common functionality for AI clients

use async_openai::types::chat::CreateChatCompletionResponse;
use tracing::error;
use super::AiError;

// Constants
pub const DEFAULT_AI_TIMEOUT: u64 = 30;
pub const DEFAULT_TEMPERATURE: f32 = 0.7;
pub const DEFAULT_MAX_TOKENS: u32 = 4096;

// Utility functions

/// Validate input parameters for text correction
pub fn validate_correction_input(text: &str, prompt: &str) -> Result<(), AiError> {
    if text.trim().is_empty() {
        return Err(AiError::InvalidInput("Text cannot be empty".to_string()));
    }
    if prompt.trim().is_empty() {
        return Err(AiError::InvalidInput("Prompt cannot be empty".to_string()));
    }
    Ok(())
}

/// Validate correction result and log success
pub fn validate_correction_result(
    corrected: &str,
    original: &str,
    provider_name: &str,
) -> Result<(), AiError> {
    if corrected.trim().is_empty() {
        error!("{} returned empty corrected text", provider_name);
        return Err(AiError::InvalidResponse("AI returned empty corrected text".to_string()));
    }

    tracing::info!(
        original_length = original.len(),
        corrected_length = corrected.len(),
        "{} correction applied successfully",
        provider_name
    );

    Ok(())
}

/// Extract content from chat completion response
pub fn extract_response_content(
    response: &CreateChatCompletionResponse,
    provider_name: &str,
) -> Result<String, AiError> {
    response
        .choices
        .first()
        .and_then(|c| c.message.content.as_deref())
        .ok_or_else(|| {
            error!("{} response missing choices or content", provider_name);
            AiError::InvalidResponse("Response missing choices or content".to_string())
        })
        .map(|s| s.to_string())
}

/// Log response preview
pub fn log_response_preview(content: &str, provider_name: &str) {
    tracing::info!(
        content_length = content.len(),
        content_preview = &content[..content.len().min(200)],
        "{} correction completed",
        provider_name
    );
}
```

### 2. `src-tauri/src/ai/mod.rs`

Добавить:
```rust
pub mod common;
```

### 3. `src-tauri/src/ai/openai.rs`

**Изменения:**
- Добавить импорт: `use super::{AiClient, AiError, common as ai_common};`
- Добавить поле `model: String` в структуру
- Удалить константу `DEFAULT_MODEL`
- В `new()`: добавить `let model = settings.openai.model.clone();`
- В `send_request()`: заменить `Self::DEFAULT_MODEL` на `&self.model`, использовать `ai_common::*`
- В `AiClient::correct()`: использовать `ai_common::validate_correction_input()` и `ai_common::validate_correction_result()`

### 4. `src-tauri/src/ai/zai.rs`

**Изменения:**
- Добавить импорт: `use super::{AiClient, AiError, common as ai_common};`
- Удалить константу `DEFAULT_TIMEOUT`
- В `send_request()`: использовать `ai_common::*`
- В `AiClient::correct()`: использовать `ai_common::validate_correction_input()` и `ai_common::validate_correction_result()`

### 5. `src-tauri/src/config/settings.rs`

**Добавить в `AiOpenAiSettings`:**
```rust
#[serde(default = "default_openai_model")]
pub model: String,

fn default_openai_model() -> String {
    "gpt-4o-mini".to_string()
}
```

**Добавить методы в `SettingsManager`:**
```rust
pub fn set_ai_openai_model(&self, model: String) -> Result<()> {
    self.update_field("/ai/openai/model", &model)
}

pub fn get_ai_openai_model(&self) -> String {
    self.cache.read().ai.openai.model.clone()
}

pub fn set_ai_zai_model(&self, model: String) -> Result<()> {
    self.update_field("/ai/zai/model", &model)
}
```

### 6. `src-tauri/src/config/dto.rs`

**Добавить в `AiOpenAiSettingsDto`:**
```rust
#[serde(default = "default_openai_model_dto")]
pub model: String,

fn default_openai_model_dto() -> String {
    "gpt-4o-mini".to_string()
}
```

**Обновить `From` implementations:**
```rust
impl From<crate::config::AiOpenAiSettings> for AiOpenAiSettingsDto {
    fn from(s: crate::config::AiOpenAiSettings) -> Self {
        Self {
            api_key: s.api_key,
            use_proxy: s.use_proxy,
            model: s.model,  // <-- NEW
        }
    }
}

impl From<AiOpenAiSettingsDto> for crate::config::AiOpenAiSettings {
    fn from(dto: AiOpenAiSettingsDto) -> Self {
        Self {
            api_key: dto.api_key,
            use_proxy: dto.use_proxy,
            model: dto.model,  // <-- NEW
        }
    }
}
```

### 7. `src-tauri/src/commands/ai.rs`

Добавить команды:
```rust
#[tauri::command]
pub fn set_ai_openai_model(
    settings_manager: State<'_, SettingsManager>,
    model: String,
) -> Result<(), String> {
    settings_manager
        .set_ai_openai_model(model)
        .map_err(|e| format!("Failed to save OpenAI model: {}", e))
}

#[tauri::command]
pub fn get_ai_openai_model(
    settings_manager: State<'_, SettingsManager>,
) -> String {
    settings_manager.get_ai_openai_model()
}

#[tauri::command]
pub fn set_ai_zai_model(
    settings_manager: State<'_, SettingsManager>,
    model: String,
) -> Result<(), String> {
    settings_manager
        .set_ai_zai_model(model)
        .map_err(|e| format!("Failed to save Z.ai model: {}", e))
}
```

### 8. `src-tauri/src/lib.rs`

Добавить в `generate_handler!`:
```rust
set_ai_openai_model,
get_ai_openai_model,
set_ai_zai_model,
```

### 9. `src/types/settings.ts`

```typescript
export interface AiOpenAiSettingsDto {
  api_key?: string
  use_proxy?: boolean
  model?: string  // <-- NEW
}
```

---

## Порядок выполнения

1. **Settings Core:** `settings.rs` → добавить `model` в `AiOpenAiSettings`
2. **DTOs:** `dto.rs` → добавить `model` в DTO
3. **Common Module:** создать `common.rs`
4. **AI Module:** обновить `mod.rs` → добавить `pub mod common`
5. **OpenAI Client:** `openai.rs` → рефакторинг использовать `common`
6. **Z.ai Client:** `zai.rs` → рефакторинг использовать `common`
7. **Commands:** `ai.rs` → добавить команды
8. **Lib:** `lib.rs` → зарегистрировать команды
9. **Types:** `settings.ts` → добавить `model` в интерфейс

---

## Обратная совместимость

**Migration path:** Благодаря `#[serde(default = "default_openai_model")]` существующие настройки автоматически получат значение `"gpt-4o-mini"` при первой загрузке.

Старый формат JSON:
```json
{"ai": {"openai": {"api_key": "sk-...", "use_proxy": false}}}
```

Новый формат (auto-migrated):
```json
{"ai": {"openai": {"api_key": "sk-...", "use_proxy": false, "model": "gpt-4o-mini"}}}
```

---

## Проверка

```bash
# Компиляция
cargo check

# Юнит-тесты
cargo test --package ttsbard-lib --lib ai::common::tests
cargo test --package ttsbard-lib --lib ai::openai::tests
cargo test --package ttsbard-lib --lib ai::zai::tests

# Полные тесты
cargo test
```

**Functional checklist:**
- [ ] OpenAI клиент использует модель из настроек
- [ ] Z.ai клиент использует модель из настроек
- [ ] Оба клиента используют общие константы (temperature, max_tokens, timeout)
- [ ] Оба клиента используют общие функции (validate, extract, log)
- [ ] Дублирование кода удалено
- [ ] Существующие настройки мигрируют автоматически
