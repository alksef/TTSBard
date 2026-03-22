# План: Применение AI для коррекции текста

**Дата:** 2026-03-22
**Номер:** 55
**Задача:** Реализовать применение AI для коррекции текста перед TTS

## Обзор

Добавить функциональность AI коррекции текста, которая применяется перед отправкой на TTS. Включает:
- **UI:** Чекбокс "Применять AI" в настройках Редактор
- **UI:** Индикатор "AI" в панели Текст
- **Backend:** AI клиент для OpenAI и Z.ai
- **Backend:** Интеграция в TTS pipeline

---

## Требования пользователя

| Параметр | Значение |
|----------|----------|
| OpenAI модель | `gpt-4o-mini` (настраивается в `settings.json`) |
| Z.ai модель | `gpt-4.5` (настраивается в `settings.json`) |
| Таймаут | 20 секунд (настраивается `ai.timeout`) |
| При ошибке AI | Логировать + применять преобразование чисел + отправлять на TTS |

---

## Визуальный дизайн UI

### 1. Настройки → Редактор

Добавить чекбокс "Применять AI" ниже "Быстрый редактор":

```
┌─────────────────────────────────────────────────────────────┐
│  Настройки                                                  │
├─────────────────────────────────────────────────────────────┤
│  ⚙ Общие  |  📝 Редактор  |  🌐 Сеть  |  ✨ AI              │
│  ═══════════════════════════════════════════════════════════  │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ ☐ Быстрый редактор                                   │   │
│  │   При включении скрывает окно по нажатию Enter...    │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ ☐ Применять AI                    [disabled state]   │   │
│  │   Корректировать текст через AI перед TTS            │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
│  Если API ключ не настроен:                                │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ ☐ Применять AI  (disabled)                           │   │
│  │   Сначала настройте API ключ выбранного AI провайдера │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

**Стили чекбокса:**
- Использовать существующий `.setting-row` и `.checkbox-label`
- Disabled состояние: `opacity: 0.5`, `cursor: not-allowed`
- Подсказка меняется в зависимости от состояния `aiKeyConfigured`

### 2. Панель Текст

Добавить индикатор "AI" и кнопку "✨ Корректировать" ниже textarea:

```
┌─────────────────────────────────────────────────────────────┐
│  Текст                                                      │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │                                                     │   │
│  │  [текстовое поле ввода]                            │   │
│  │                                                     │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
│  ┌──────────────┐  ┌──────────────┐                        │
│  │ ✨ Корректировать │  │ Отправить на TTS │                    │
│  └──────────────┘  └──────────────┘                        │
│                                                             │
│  Режим быстрого редактора                                    │
│  AI                                                         │
└─────────────────────────────────────────────────────────────┘
```

**Поведение кнопки "✨ Корректировать":**
- Доступна только когда `ai_enabled = true` и есть текст
- При нажатии: отправить запрос к AI
- При успехе: заменить текст в поле ввода на исправленный
- При ошибке: показать уведомление об ошибке

**Стили индикатора AI:**
```css
.ai-status-hint {
  margin-top: 0.25rem;
  font-size: 0.75rem;
  color: var(--color-accent);
  opacity: 0.8;
  text-align: center;
  font-weight: 600;
  letter-spacing: 0.05em;
}
```

**Кнопка коррекции:**
- Стиль как secondary кнопка (серая/нейтральная)
- Иконка Sparkles (✨)
- Loading state во время запроса

---

## Архитектура

### 1. Потоки данных

**Два режима использования AI:**

```
┌─────────────────────────────────────────────────────────────┐
│  АВТОМАТИЧЕСКИЙ (при отправке на TTS)                        │
├─────────────────────────────────────────────────────────────┤
│  User Input → Replacements → AI → Numbers? → TTS           │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│  РУЧНОЙ (кнопка "✨ Корректировать")                        │
├─────────────────────────────────────────────────────────────┤
│  User Input → invoke(correct_text_only) → AI → Update field│
└─────────────────────────────────────────────────────────────┘
```

### 2. Поток данных TTS с AI коррекцией

```
┌──────────────┐
│ User Input   │
└──────┬───────┘
       │
       ▼
┌─────────────────────────────────────────────────────────────┐
│  TTS Pipeline (commands/mod.rs - speak_text_internal)       │
├─────────────────────────────────────────────────────────────┤
│  1. Prefix Parsing       (удалить !!/! префиксы)           │
│  2. Replacements         (\word, %username замены)         │
│  3. AI Correction        (NEW - если ai_enabled)           │
│     ├── Success → использовать исправленный текст           │
│     └── Error   → логировать, использовать оригинал        │
│  4. Numbers to Text      (пропускать если AI succeeded)    │
│  5. TTS Synthesis                                               │
└─────────────────────────────────────────────────────────────┘
```

### 3. AI Клиент (новый модуль)

```
src-tauri/src/ai/mod.rs
├── correct_text()      # главная функция
├── build_openai_request()
├── build_zai_request()
└── send_request()
```

**Зависимости:**
- `reqwest` для HTTP запросов
- Настройки из `AiSettings` (model, timeout, credentials)

### 3. Настройки в settings.json

```json
{
  "ai_enabled": true,
  "ai": {
    "provider": "openai",
    "timeout": 20,
    "prompt": "Correct grammar...",
    "openai": {
      "model": "gpt-4o-mini",
      "api_key": "sk-...",
      "use_proxy": false
    },
    "zai": {
      "model": "gpt-4.5",
      "url": "https://api.openai.com/v1/chat/completions",
      "token": "sk-..."
    }
  }
}
```

### 4. Tauri команда для ручной коррекции

Новая команда `correct_text_only` для кнопки "✨ Корректировать":

```rust
// src-tauri/src/commands/ai.rs

#[tauri::command]
pub async fn correct_text_only(
    text: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    // Проверить что AI включён
    if !state.get_settings()?.ai_enabled {
        return Err("AI коррекция отключена".into());
    }

    // Вызвать AI коррекцию
    ai::correct_text(&text, &state).await
        .map_err(|e| e.to_string())
}
```

**Frontend вызов:**

```typescript
// src/components/InputPanel.vue

const isCorrecting = ref(false)

async function correctWithAI() {
  if (!text.value || !aiEnabled.value) return

  isCorrecting.value = true
  try {
    const corrected = await invoke('correct_text_only', { text: text.value })
    text.value = corrected  // Обновить поле ввода
    showSuccess('Текст скорректирован')
  } catch (error) {
    showError(error as string)
  } finally {
    isCorrecting.value = false
  }
}
```

---

## Изменения в файлах

### Backend (Rust)

| Файл | Изменения |
|------|-----------|
| `src-tauri/src/config/settings.rs` | Добавить `ai_enabled`, `model`, `timeout` поля |
| `src-tauri/src/config/dto.rs` | Обновить DTOs с новыми полями |
| `src-tauri/src/commands/ai.rs` | Добавить `set_ai_enabled`, `get_ai_enabled`, `correct_text_only` |
| `src-tauri/src/commands/mod.rs` | Интегрировать AI в pipeline, обработка ошибок |
| `src-tauri/src/ai/mod.rs` | **НОВЫЙ** - AI клиент модуль |
| `src-tauri/src/lib.rs` | Зарегистрировать команды, добавить `mod ai` |
| `src-tauri/Cargo.toml` | Добавить `reqwest` зависимость |

### Frontend (TypeScript/Vue)

| Файл | Изменения |
|------|-----------|
| `src/types/settings.ts` | Добавить `ai_enabled` в `GeneralSettingsDto` |
| `src/composables/useAppSettings.ts` | Экспортировать `ai_enabled` через general |
| `src/components/SettingsPanel.vue` | Добавить чекбокс "Применять AI" |
| `src/components/InputPanel.vue` | Добавить кнопку "✨ Корректировать" + индикатор "AI" |

---

## 5. API запросы

### OpenAI (gpt-4o-mini)

```http
POST https://api.openai.com/v1/chat/completions
Authorization: Bearer sk-...
Content-Type: application/json

{
  "model": "gpt-4o-mini",
  "messages": [
    { "role": "system", "content": "<prompt из настроек>" },
    { "role": "user", "content": "<текст для коррекции>" }
  ],
  "temperature": 0.3,
  "max_tokens": 4096
}
```

### Z.ai (gpt-4.5)

```http
POST https://api.openai.com/v1/chat/completions
Authorization: Bearer sk-...
Content-Type: application/json

{
  "model": "gpt-4.5",
  "messages": [
    { "role": "system", "content": "<prompt из настроек>" },
    { "role": "user", "content": "<текст для коррекции>" }
  ],
  "temperature": 0.3,
  "max_tokens": 4096
}
```

---

## 6. Обработка ошибок

### Сценарий 1: AI успешно скорректировал

```
[DEBUG] AI correction enabled, processing text...
[DEBUG] Sending AI correction request provider=OpenAi model=gpt-4o-mini text_len=25
[DEBUG] AI request details url=... body={...}
[DEBUG] AI response details response={...}
[INFO] AI correction applied original="..." corrected="..."
[DEBUG] Skipping number-to-text conversion (AI succeeded)
→ Использовать исправленный текст, НЕ применять number-to-text
```

### Сценарий 2: AI ошибка

```
[DEBUG] AI correction enabled, processing text...
[ERROR] AI correction failed: AI request failed: 401 - Invalid API key
[DEBUG] Numbers to text
→ Использовать оригинальный текст, ПРИМЕНИТЬ number-to-text
```

### Сценарий 3: AI отключен

```
→ Не вызывать AI
→ Применить number-to-text как обычно
```

---

## 7. Логика выполнения

### 1. Проверка перед включением чекбокса

```typescript
const aiKeyConfigured = computed(() => {
  if (!aiSettings.value) return false
  const provider = aiSettings.value.provider
  if (provider === 'openai') {
    return !!aiSettings.value.openai?.api_key
  } else if (provider === 'zai') {
    return !!aiSettings.value.zai?.token
  }
  return false
})
```

**Результат:** Чекбокс disabled если нет ключа

### 2. Поток в speak_text_internal

```rust
// Stage 2: Replacements (existing)
let text = preprocessor.process(&text);

// Stage 2.5: AI Correction (NEW)
let (text, ai_failed) = if ai_enabled {
    match ai::correct_text(&text, &state).await {
        Ok(corrected) => (corrected, false),
        Err(e) => {
            error!(error = %e, "AI correction failed");
            (text, true)  // mark as failed
        }
    }
} else {
    (text, false)
};

// Stage 3: Numbers to text (conditional)
let text = if !ai_enabled || ai_failed {
    process_numbers(&text)  // Apply number conversion
} else {
    text  // Skip number conversion
};

// Stage 4: TTS synthesis (existing)
tts.synthesize(&text).await?;
```

---

## 8. Debug логирование

### Уровни логов

| Уровень | Событие |
|---------|---------|
| DEBUG | Все детали запроса/ответа AI |
| INFO | Успешная коррекция (original → corrected) |
| ERROR | Ошибки AI API |
| WARN | Пропуск number-to-text (при успехе AI) |

### Пример лога

```
[DEBUG] [AI] Sending correction request
  provider=OpenAi
  model=gpt-4o-mini
  text_len=42
  timeout=20s

[DEBUG] [AI] Request body
  {
    "model": "gpt-4o-mini",
    "messages": [
      {"role": "system", "content": "Correct grammar..."},
      {"role": "user", "content": "privet kak dela"}
    ],
    "temperature": 0.3
  }

[DEBUG] [AI] Response received
  status=200
  body={
    "choices": [{
      "message": {"content": "Привет, как дела?"}
    }]
  }

[INFO] [AI] Correction applied
  original="privet kak dela"
  corrected="Привет, как дела?"
```

---

## 9. Проверка (Verification)

### 1. UI тестирование

- [ ] Чекбокс появляется на вкладке Редактор
- [ ] Чекбокс disabled когда нет API ключа
- [ ] Чекбокс enabled когда API ключ настроен
- [ ] Сохранение состояния после перезагрузки приложения
- [ ] Индикатор "AI" появляется в панели Текст при включении
- [ ] Кнопка "✨ Корректировать" появляется при `ai_enabled = true`
- [ ] Кнопка скрыта когда `ai_enabled = false`
- [ ] Кнопка disabled когда поле ввода пустое
- [ ] Loading state на кнопке во время запроса
- [ ] Текст в поле обновляется после успешной коррекции

### 2. Функциональное тестирование

**Автоматическая коррекция (при отправке на TTS):**
- [ ] Успешная коррекция через OpenAI
- [ ] Успешная коррекция через Z.ai
- [ ] При ошибке API — логируется, числа конвертируются
- [ ] При успехе AI — числа НЕ конвертируются
- [ ] Debug логи содержат детали запроса/ответа

**Ручная коррекция (кнопка "✨ Корректировать"):**
- [ ] Успешная коррекция обновляет поле ввода
- [ ] При ошибке API — показывается уведомление
- [ ] При ошибке текст в поле НЕ изменяется
- [ ] Кнопка не работает когда AI отключён

### 3. Конфигурация

- [ ] `settings.json` содержит `ai_enabled`
- [ ] `settings.json` содержит `timeout`, `model`
- [ ] Смена модели в конфиге применяется

### 4. Сборка

```bash
npm run check    # TypeScript
npm run build    # Frontend
cargo check      # Rust
cargo build      # Full build
```

---

## Открытые вопросы

**Все вопросы решены:**

| Вопрос | Ответ |
|--------|-------|
| Название вкладки AI | ✨ AI (как есть) |
| Позиция чекбокса | Сверху (после "Быстрый редактор") |
| Кнопка коррекции | Да, нужна — обновляет текст в поле ввода |

---

## Phase 3: Автоматическая AI коррекция

**Новая функциональность:**
- Чекбокс "Применять AI автоматически" в настройках
- Автоматическая коррекция текста перед TTS
- Индикация что текст был скорректирован

### Backend (Rust)

#### 7. Добавить `enabled` поле в `AiSettings`

**Файл:** `src-tauri/src/config/settings.rs`

```rust
pub struct AiSettings {
    #[serde(default)]
    pub enabled: bool,  // <-- NEW
    #[serde(default)]
    pub provider: AiProviderType,
    // ... rest of fields
}
```

**Файл:** `src-tauri/src/config/dto.rs`

```rust
pub struct AiSettingsDto {
    pub enabled: bool,  // <-- NEW
    pub provider: AiProviderTypeDto,
    // ... rest of fields
}
```

#### 8. Добавить команды для управления enabled

**Файл:** `src-tauri/src/commands/ai.rs`

```rust
#[tauri::command]
pub fn set_ai_enabled(
    settings_manager: State<'_, SettingsManager>,
    enabled: bool,
) -> Result<(), String> {
    settings_manager.set_ai_enabled(enabled)
        .map_err(|e| format!("Failed to save: {}", e))
}

#[tauri::command]
pub fn get_ai_enabled(
    settings_manager: State<'_, SettingsManager>,
) -> bool {
    settings_manager.get_ai_enabled()
}
```

**Файл:** `src-tauri/src/config/manager.rs`

```rust
pub fn set_ai_enabled(&self, enabled: bool) -> Result<()> {
    self.update_field("/ai/enabled", &enabled)
}

pub fn get_ai_enabled(&self) -> bool {
    self.cache.read().ai.enabled
}
```

#### 9. Интегрировать AI коррекцию в TTS pipeline

**Файл:** `src-tauri/src/commands/mod.rs`

В `speak_text_internal()` функции, **после Stage 2 (replacements)**:

```rust
// === STAGE 2.5: AI Text Correction (if enabled) ===
let text = {
    let settings = settings_manager.load()
        .map_err(|e| format!("Failed to load settings: {}", e))?;

    if settings.ai.enabled {
        match crate::ai::create_ai_client(&settings.ai, &settings.tts.network).await {
            Ok(client) => {
                match client.correct(&text, &settings.ai.prompt).await {
                    Ok(corrected) => {
                        tracing::info!("AI correction applied");
                        corrected
                    }
                    Err(e) => {
                        tracing::warn!("AI correction failed, using original: {}", e);
                        text  // Fallback to original on error
                    }
                }
            }
            Err(e) => {
                tracing::warn!("AI client not available, skipping: {}", e);
                text
            }
        }
    } else {
        text
    }
};

// === STAGE 3: Numbers to text ===
let text = crate::preprocessor::process_numbers(&text);
```

**Важно:** AI коррекция должна быть **fault-tolerant** - при ошибке fallback на оригинальный текст.

### Frontend (Vue)

#### 10. Обновить `src/types/settings.ts`

```typescript
export interface AiSettingsDto {
  enabled: boolean  // <-- NEW
  provider: AiProviderType
  prompt: string
  openai: AiOpenAiSettingsDto
  zai: AiZAiSettingsDto
}
```

#### 11. Добавить чекбокс в `src/components/SettingsAiPanel.vue`

```vue
<div class="setting-row">
  <label>
    <input
      type="checkbox"
      v-model="localSettings.ai.enabled"
      @change="saveAiEnabled"
    />
    <span>Применять AI коррекцию автоматически</span>
  </label>
  <small class="hint">
    Текст будет корректироваться перед озвучиванием
  </small>
</div>
```

```typescript
async function saveAiEnabled() {
  try {
    await invoke('set_ai_enabled', { enabled: localSettings.ai.enabled })
    showSuccess('AI коррекция: ' + (localSettings.ai.enabled ? 'включена' : 'выключена'))
  } catch (e) {
    showError('Не удалось сохранить настройку')
  }
}
```

### Решения по вопросам

1. **Fault tolerance:** Fallback на оригинальный текст с warn логом ✅
2. **Индикация:** Прозрачно (без уведомлений) ✅
3. **Timeout:** 30 секунд (как в Phase 2) ✅

---

### Файлы для изменения (Phase 3)

| Файл | Действие |
|------|----------|
| `src-tauri/src/config/settings.rs` | EDIT - Add `enabled: bool` |
| `src-tauri/src/config/dto.rs` | EDIT - Add `enabled: bool` |
| `src-tauri/src/config/manager.rs` | EDIT - Add `set_ai_enabled()`, `get_ai_enabled()` |
| `src-tauri/src/commands/ai.rs` | EDIT - Add commands |
| `src-tauri/src/lib.rs` | EDIT - Register commands |
| `src-tauri/src/commands/mod.rs` | EDIT - Integrate into `speak_text_internal()` |
| `src/types/settings.ts` | EDIT - Add `enabled: boolean` |
| `src/components/SettingsAiPanel.vue` | EDIT - Add checkbox |

---
