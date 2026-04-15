# План: Выбор спикера для Telegram/Silero TTS

## Что сделано

### Backend (Rust)

1. **Типы и конфигурация**
   - Добавлен тип `VoiceCode` в `src-tauri/src/telegram/types.rs` с полями `id` и `description`
   - Обновлена структура `TelegramTtsSettings` в `src-tauri/src/config/settings.rs` с полями `voices` и `current_voice_id`
   - Добавлена поддержка в DTO (`src-tauri/src/config/dto.rs`) с `#[serde(default)]` для обратной совместимости

2. **Логика работы с ботами**
   - Реализована функция `set_speaker()` в `src-tauri/src/telegram/bot.rs` для отправки команды `/speaker {code}`
   - Добавлена функция `send_speaker_command_with_code()` для отправки команды с кодом голоса
   - Реализован парсинг ответа бота `parse_set_speaker_response()` с поддержкой украинского/русского/английского
   - Добавлена функция `extract_set_speaker_response_from_update()` для обработки ответов
   - Функция `get_current_voice()` теперь возвращает ID отправленного сообщения для точной идентификации ответа
   - Удалены неиспользуемые параметры в `parse_message_text_with_validation()`

3. **Tauri команды**
   - `telegram_set_speaker` - установить голос через бота
   - `telegram_add_voice_code` - добавить голос в сохраненные
   - `telegram_remove_voice_code` - удалить голос из сохраненных
   - `telegram_select_voice` - выбрать голос из списка и обновить current_voice_id
   - Все команды зарегистрированы в `src-tauri/src/lib.rs`

### Frontend (TypeScript/Vue)

1. **Типы**
   - Добавлен интерфейс `VoiceCode` в `src/types/settings.ts`
   - Обновлен интерфейс `TelegramTtsSettingsDto` с полями `voices` и `current_voice_id`

2. **Composable**
   - Добавлены функции в `src/composables/useTelegramAuth.ts`:
     - `loadSavedVoices()` - загрузка сохраненных голосов
     - `addVoiceCode()` - добавление нового голоса с валидацией через бота
     - `removeVoiceCode()` - удаление голоса
     - `selectVoice()` - выбор голоса из списка
     - `autoRefreshVoice()` - автообновление с авто-добавлением новых голосов
   - Удалён мёртвый код с ненужным `get_all_app_settings`

3. **UI Компоненты**
   - Обновлён `src/components/tts/TtsSileroCard.vue`:
     - Добавлена секция управления голосами
     - Кнопка "Обновить текущий голос" с иконкой RefreshCw
     - Кнопка "Добавить" для добавления новых голосов
     - Список сохраненных голосов с подсветкой активного
     - Диалог добавления голоса с валидацией дубликатов
     - Поддержка loading states и ошибок
     - Удалён дублирующийся интерфейс `CurrentVoice`
   - Обновлён `src/components/TtsPanel.vue`:
     - Добавлены обработчики событий для управления голосами
     - Автозагрузка голосов при подключении Telegram

### Code Review Fixes

1. Удалены неиспользуемые параметры в `parse_message_text_with_validation()`
2. Удалён мёртвый код в `useTelegramAuth.ts`
3. Удалён дублирующийся интерфейс в `TtsSileroCard.vue`
4. Исправлен тип параметра в `handleAddVoice()`

### UI Улучшения

- Кнопка "Обновить текущий голос" выполнена в едином стиле с кнопкой "Добавить"
- Иконка RefreshCw отображается слева от текста
- При загрузке показывается спиннер вместо иконки
- Поддержка disabled состояния для кнопки обновления

## Обзор

Доработка UI для TTS провайдера Telegram/Silero с возможностью выбора голоса (спикера). В карточке провайдера отображается текущий голос, есть список сохраненных голосов с возможностью добавления/удаления.

## Критические файлы

### Backend (Rust):
- `src-tauri/src/config/settings.rs` - структура `TelegramTtsSettings`
- `src-tauri/src/telegram/types.rs` - тип `VoiceCode`
- `src-tauri/src/telegram/bot.rs` - функция `set_speaker()`
- `src-tauri/src/commands/telegram.rs` - Tauri команды

### Frontend (TypeScript/Vue):
- `src/types/settings.ts` - интерфейс `TelegramTtsSettingsDto`
- `src/components/tts/TtsSileroCard.vue` - карточка провайдера
- `src/composables/useTelegramAuth.ts` - composable для Telegram
- `src/components/tts/TelegramConnectionStatus.vue` - статус подключения

## Этап 1: Backend - Типы и конфигурация

### 1.1 Добавить тип VoiceCode

**Файл:** `src-tauri/src/telegram/types.rs`

```rust
/// Сохраненный код голоса для Telegram TTS
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VoiceCode {
    pub id: String,  // e.g., "rene", "hamster_clerk"
    pub name: String, // e.g., "Rene", "Хомяки"
}
```

### 1.2 Обновить TelegramTtsSettings

**Файл:** `src-tauri/src/config/settings.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TelegramTtsSettings {
    pub api_id: Option<i64>,
    #[serde(default)]
    pub proxy_mode: ProxyMode,
    /// Список сохраненных кодов голосов
    #[serde(default)]
    pub voices: Vec<crate::telegram::VoiceCode>,
    /// Текущий выбранный ID голоса
    #[serde(default)]
    pub current_voice_id: String,
}
```

## Этап 2: Backend - Логика работы с ботами

### 2.1 Добавить функцию set_speaker()

**Файл:** `src-tauri/src/telegram/bot.rs`

```rust
/// Установить голос, отправив "/speaker {code}" боту
/// Возвращает true если успешно, иначе ошибку
pub async fn set_speaker(client: &TelegramClient, voice_code: &str) -> Result<bool, String> {
    // 1. Отправить "/speaker {code}"
    // 2. Дождаться текстового ответа
    // 3. Парснуть ответ:
    //    - "Успешно выбран спикер: {code}" → Ok(true)
    //    - "Успешно выбран тот же самый спикер" → Ok(true)
    //    - "Указан неверный голос." → Err("Invalid voice code")
}
```

**Функция парсинга ответа:**

```rust
fn parse_set_speaker_response(text: &str) -> Result<bool, String> {
    // Проверить варианты успешного ответа
    if text.contains("Успешно выбран спикер")
        || text.contains("Успішно обрано спікера")
        || text.contains("Successfully selected speaker") {
        return Ok(true);
    }

    if text.contains("Успешно выбран тот же самый спикер")
        || text.contains("Успішно обрано того самого спікера") {
        return Ok(true);
    }

    if text.contains("Указан неверный голос")
        || text.contains("Вказано невірний голос") {
        return Err("Invalid voice code".to_string());
    }

    Err("Unknown response format".to_string())
}
```

### 2.2 Временное логирование протокола (для отладки)

**ВАЖНО:** Это временная мера для отладки. После завершения реализации нужно удалить логирование отдельным таском.

**Файл:** `src-tauri/src/telegram/bot.rs`

Добавить детальное логирование в функции работы с ботом:

```rust
// В функции set_speaker():
info!("[TG_DEBUG] Sending /speaker {{voice_code}} to bot, voice_code = {{voice_code}}");
// ... отправка сообщения ...

// В цикле ожидания ответа:
info!("[TG_DEBUG] Received update: {{update_like:?}}");
// ... обработка ...

info!("[TG_DEBUG] Parsed response text: {{text}}");
```

Также добавить логирование в `get_current_voice()` для отслеживания:

```rust
info!("[TG_DEBUG] Sending /speaker command");
info!("[TG_DEBUG] Received text message: {{text}}");
info!("[TG_DEBUG] Parsed voice: id={{voice_id}}, name={{voice_name}}");
```

Это поможет увидеть:
- Что именно отправляется боту
- Какие ответы приходят (в оригинальном виде)
- Как парсятся ответы

**Удаление:** После завершения реализации и тестирования нужно создать отдельный план для удаления отладочного логирования.

## Этап 3: Backend - Tauri команды

**Файл:** `src-tauri/src/commands/telegram.rs`

### 3.1 Команда telegram_set_speaker

```rust
#[tauri::command]
pub async fn telegram_set_speaker(
    state: State<'_, TelegramState>,
    voice_code: String,
) -> Result<bool, String> {
    // 1. Валидация voice_code (не пустой)
    // 2. Получить клиент из state
    // 3. Вызвать bot::set_speaker()
    // 4. Вернуть результат
}
```

### 3.2 Команда telegram_add_voice_code

```rust
#[tauri::command]
pub fn telegram_add_voice_code(
    settings_manager: State<'_, SettingsManager>,
    voice: VoiceCode,
) -> Result<(), String> {
    // 1. Проверить что voice.id не пустой
    // 2. Проверить что нет дубликатов
    // 3. Добавить в telegram.voices
    // 4. Сохранить настройки
}
```

### 3.3 Команда telegram_remove_voice_code

```rust
#[tauri::command]
pub fn telegram_remove_voice_code(
    settings_manager: State<'_, SettingsManager>,
    voice_id: String,
) -> Result<(), String> {
    // 1. Удалить голос из telegram.voices
    // 2. Если это был current_voice_id - очистить
    // 3. Сохранить настройки
}
```

### 3.4 Команда telegram_select_voice

```rust
#[tauri::command]
pub async fn telegram_select_voice(
    state: State<'_, TelegramState>,
    settings_manager: State<'_, SettingsManager>,
    voice_id: String,
) -> Result<bool, String> {
    // 1. Отправить "/speaker {voice_id}" боту
    // 2. Если успешно - обновить current_voice_id
    // 3. Вернуть результат
}
```

### 3.5 Зарегистрировать команды

**Файл:** `src-tauri/src/commands/mod.rs`

```rust
.invoke_handler(tauri::generate_handler![
    // ... существующие команды
    telegram_set_speaker,
    telegram_add_voice_code,
    telegram_remove_voice_code,
    telegram_select_voice,
])
```

## Этап 4: Frontend - Типы

**Файл:** `src/types/settings.ts`

```typescript
export interface VoiceCode {
  id: string
  name: string
}

export interface TelegramTtsSettingsDto {
  api_id?: number
  proxy_mode?: string
  voices?: VoiceCode[]
  current_voice_id?: string
}
```

## Этап 5: Frontend - UI Компоненты

### 5.1 Обновить TtsSileroCard.vue

Добавить пропсы и emits:

```typescript
interface Props {
  // ... существующие
  currentVoice?: CurrentVoice | null
  savedVoices?: VoiceCode[]
  voiceLoading?: boolean
  voiceError?: string | null
}

interface Emits {
  // ... существующие
  (e: 'refresh-voice'): void
  (e: 'add-voice', code: string): void
  (e: 'remove-voice', id: string): void
  (e: 'select-voice', id: string): void
}
```

Добавить секцию с голосами (стиль как Fish Audio):

```vue
<div class="setting-group">
  <div class="voice-header">
    <label>Голоса</label>
    <button @click="showAddInput = !showAddInput" class="add-model-button">
      <Plus :size="16" />
      {{ showAddInput ? 'Отмена' : 'Добавить' }}
    </button>
  </div>

  <!-- Текущий голос -->
  <div v-if="currentVoice" class="current-voice">
    Текущий голос: {{ currentVoice.name }} ({{ currentVoice.id }})
  </div>

  <!-- Инпут для добавления (показывается при нажатии на кнопку Добавить) -->
  <div v-if="showAddInput" class="add-voice-row">
    <input
      ref="voiceCodeInput"
      v-model="newVoiceCode"
      placeholder="Код голоса (например: rene)"
      @keyup.enter="handleAddVoice"
      :disabled="voiceLoading"
      class="voice-code-input"
    />
    <button
      @click="handleAddVoice"
      :disabled="!newVoiceCode.trim() || voiceLoading"
      class="add-voice-confirm"
    >
      OK
    </button>
  </div>

  <!-- Ошибка если есть -->
  <div v-if="voiceError" class="voice-error">
    {{ voiceError }}
  </div>

  <!-- Список голосов -->
  <div v-if="savedVoices.length > 0" class="voice-list">
    <div
      v-for="voice in savedVoices"
      :key="voice.id"
      :class="['voice-item', { active: voice.id === currentVoice?.id }]"
      @click="$emit('select-voice', voice.id)"
    >
      <div class="voice-info">
        <div class="voice-title">{{ voice.name }}</div>
        <div class="voice-details">
          <span class="voice-id">{{ voice.id }}</span>
        </div>
      </div>
      <button @click.stop="$emit('remove-voice', voice.id)" class="remove-button">
        <Trash2 :size="14" />
      </button>
    </div>
  </div>
  <div v-else class="empty-voices">
    Нет добавленных голосов
  </div>
</div>
```

**Логика в компоненте:**

```typescript
import { NextTick } from 'vue'

const showAddInput = ref(false)
const newVoiceCode = ref('')
const voiceCodeInput = ref<HTMLInputElement | null>(null)

watch(showAddInput, async (show) => {
  if (show) {
    await NextTick()
    voiceCodeInput.value?.focus()
  }
})

function handleAddVoice() {
  const code = newVoiceCode.value.trim()
  if (code) {
    emit('add-voice', code)
    newVoiceCode.value = ''
    showAddInput.value = false
  }
}
```

### 5.2 Обновить TelegramConnectionStatus.vue

Добавить отображение текущего голоса:

```vue
<div v-if="currentVoice" class="current-voice-info">
  Текущий голос: {{ currentVoice.name }} ({{ currentVoice.id }})
</div>
```

### 5.3 Стили для секции голосов (стиль как Fish Audio)

Добавить в `<style scoped>`:

```css
.voice-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 12px;
}

.voice-header label {
  margin-bottom: 0;
}

.add-model-button {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 0.5rem 1rem;
  background: var(--color-accent);
  border: none;
  border-radius: 8px;
  color: var(--color-text-white);
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition: filter 0.2s;
}

.add-model-button:hover {
  filter: brightness(1.1);
}

.current-voice {
  padding: 0.75rem;
  background: var(--color-accent-alpha);
  border: 1px solid var(--color-accent);
  border-radius: 8px;
  font-size: 14px;
  color: var(--color-text-primary);
  margin-bottom: 12px;
}

.add-voice-row {
  display: flex;
  gap: 8px;
  margin-bottom: 12px;
}

.voice-code-input {
  flex: 1;
  padding: 10px 12px;
  background: var(--color-bg-field);
  border: 1px solid var(--color-border-strong);
  border-radius: 10px;
  color: var(--color-text-primary);
  font-size: 13px;
}

.voice-code-input:focus {
  outline: none;
  border-color: var(--color-accent);
}

.voice-code-input:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.add-voice-confirm {
  padding: 0 1.5rem;
  background: var(--color-accent);
  border: none;
  border-radius: 10px;
  color: var(--color-text-white);
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
  transition: filter 0.2s;
}

.add-voice-confirm:hover:not(:disabled) {
  filter: brightness(1.1);
}

.add-voice-confirm:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.voice-error {
  padding: 0.5rem 0.75rem;
  background: var(--danger-bg-weak);
  color: var(--danger-text);
  border-radius: 8px;
  font-size: 13px;
  margin-bottom: 12px;
}

.voice-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  max-height: 300px;
  overflow-y: auto;
  margin-bottom: 8px;
}

.voice-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 0.75rem;
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.2s;
}

.voice-item:hover {
  background: var(--color-bg-tertiary);
}

.voice-item.active {
  border-color: var(--color-accent);
  background: var(--color-accent-alpha);
}

.voice-info {
  flex: 1;
  min-width: 0;
}

.voice-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--color-text-primary);
  margin-bottom: 2px;
}

.voice-details {
  display: flex;
  align-items: center;
  gap: 8px;
}

.voice-id {
  font-size: 12px;
  color: var(--color-text-secondary);
  font-family: monospace;
}

.remove-button {
  margin: 0;
  padding: 0;
  background: var(--danger-bg-weak);
  color: var(--color-text-white);
  border: none;
  border-radius: 8px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background 0.2s;
  width: 32px;
  height: 32px;
  flex-shrink: 0;
}

.remove-button:hover {
  background: var(--danger-bg-hover);
}

.empty-voices {
  padding: 1rem;
  text-align: center;
  color: var(--color-text-secondary);
  font-size: 13px;
  background: var(--color-bg-secondary);
  border-radius: 8px;
}
```

## Этап 6: Frontend - Composable

**Файл:** `src/composables/useTelegramAuth.ts`

```typescript
export function useTelegramAuth() {
  // ... существующие state

  const savedVoices = ref<VoiceCode[]>([])
  const voiceLoading = ref(false)
  const voiceError = ref<string | null>(null)

  // Загрузить сохраненные голоса из настроек
  async function loadSavedVoices() {
    // invoke get_all_app_settings
    // извлечь telegram.voices
  }

  // Добавить голос
  async function addVoiceCode(code: string) {
    voiceLoading.value = true
    try {
      // 1. Отправить "/speaker {code}" боту
      const success = await invoke<boolean>('telegram_set_speaker', { voiceCode: code })

      if (success) {
        // 2. Получить информацию о голосе
        const voice = await invoke<CurrentVoice>('telegram_get_current_voice')

        if (voice) {
          // 3. Добавить в сохраненные
          await invoke('telegram_add_voice_code', {
            voice: { id: voice.id, name: voice.name }
          })

          // 4. Обновить current_voice_id
          // ...

          await loadSavedVoices()
        }
      }
    } catch (error) {
      voiceError.value = error as string
    } finally {
      voiceLoading.value = false
    }
  }

  // Удалить голос
  async function removeVoiceCode(id: string) {
    await invoke('telegram_remove_voice_code', { voiceId: id })
    await loadSavedVoices()
  }

  // Выбрать голос
  async function selectVoice(id: string) {
    voiceLoading.value = true
    try {
      const success = await invoke<boolean>('telegram_select_voice', { voiceId: id })

      if (success) {
        await refreshVoice()
      }
    } finally {
      voiceLoading.value = false
    }
  }

  // Авто-обновление при запуске
  async function autoRefreshVoice() {
    try {
      const voice = await refreshVoice()

      if (voice) {
        // Проверить есть ли голос в списке
        const exists = savedVoices.value.some(v => v.id === voice.id)

        if (!exists) {
          // Авто-добавить новый голос
          await invoke('telegram_add_voice_code', {
            voice: { id: voice.id, name: voice.name }
          })
          await loadSavedVoices()
        }
      }
    } catch (error) {
      debugError('Auto-refresh voice failed:', error)
    }
  }

  return {
    // ... существующие
    savedVoices,
    voiceLoading,
    voiceError,
    loadSavedVoices,
    addVoiceCode,
    removeVoiceCode,
    selectVoice,
    autoRefreshVoice,
  }
}
```

## Этап 7: Интеграция

### 7.1 Родительский компонент (TtsPanel или подобный)

```typescript
// При монтировании
onMounted(async () => {
  if (telegramAuth.isConnected) {
    await telegramAuth.loadSavedVoices()
    await telegramAuth.autoRefreshVoice()
  }
})

// При подключении Telegram
watch(() => telegramAuth.isConnected, async (connected) => {
  if (connected) {
    await telegramAuth.loadSavedVoices()
    await telegramAuth.autoRefreshVoice()
  }
})
```

### 7.2 Обработчики событий

```typescript
async function handleRefreshVoice() {
  await telegramAuth.refreshVoice()
}

async function handleAddVoice(code: string) {
  await telegramAuth.addVoiceCode(code)
}

async function handleRemoveVoice(id: string) {
  await telegramAuth.removeVoiceCode(id)
}

async function handleSelectVoice(id: string) {
  await telegramAuth.selectVoice(id)
}
```

## Проверка (Verification)

### 1. Backend проверка

```bash
cargo check
cargo test
```

### 2. Frontend проверка

```bash
npm run type-check
npm run build
```

### 3. Функциональное тестирование

1. Запустить приложение
2. Подключить Telegram
3. Проверить что текущий голос отобразился
4. Добавить новый голос через диалог
5. Проверить что голос добавился в список
6. Выбрать голос из списка
7. Удалить голос из списка
8. Перезапустить приложение - проверить что настройки сохранились

### 4. Edge cases

- Попытка добавить несуществующий код голоса
- Таймаут при получении текущего голоса
- Удаление текущего выбранного голоса
- Пустой список голосов

### 5. Post-implementation задачи

**ВАЖНО:** После завершения реализации и тестирования нужно создать отдельный план для удаления отладочного логирования:

- Удалить все `[TG_DEBUG]` логи из `src-tauri/src/telegram/bot.rs`
- Очистить функции `set_speaker()` и `get_current_voice()` от отладочного кода
- Убрать логирование сырых ответов от бота
- Проверить что production логи не содержат лишней информации
