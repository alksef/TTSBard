# Error Handling Implementation for Silero TTS

## Overview
Реализована полная обработка ошибок с красной плашкой для Silero TTS провайдера в проекте app-tts-v2.

## Features Implemented

### 1. Red Error Banner in TtsPanel.vue

#### Visual Changes
- **Error State Card**: Карточка Silero провайдера получает красный фон и рамку при ошибке
  - CSS класс `.provider-card.error-state` с красным фоном `#fef2f2` и рамкой `#ef4444`
  - Автоматически применяется когда `sileroError !== null`

#### Error Banner Component
Добавлен компонент `silero-error-banner` внутри карточки Silero:

```vue
<div v-if="sileroError" class="silero-error-banner">
  <div class="error-banner-content">
    <div class="error-icon">⚠</div>
    <div class="error-text">
      <p class="error-title">Ошибка подключения Telegram</p>
      <p class="error-message">{{ sileroError }}</p>
    </div>
  </div>
  <button class="fix-button" @click="openTelegramModal">
    Исправить
  </button>
</div>
```

**Особенности:**
- Иконка предупреждения (⚠)
- Заголовок "Ошибка подключения Telegram"
- Текст ошибки из Telegram auth composable
- Кнопка "Исправить" для открытия модала авторизации

#### State Management
```typescript
// Silero error state
const sileroError = ref<string | null>(null);

// Watch for Telegram errors
watch([telegramErrorMessage, telegramHasError], () => {
  handleSileroError();
});

// Clear Silero error when successfully connected
watch(telegramConnected, (newValue) => {
  if (newValue) {
    sileroError.value = null;
  }
});
```

### 2. Enhanced TelegramAuthModal.vue

#### New Error State
Добавлено новое состояние ошибки с кнопками управления:

```vue
<div v-else-if="hasError" class="error-state">
  <div class="error-icon-modal">⚠</div>
  <h3>Ошибка подключения</h3>

  <div v-if="errorMessage" class="error-message-modal">
    {{ errorMessage }}
  </div>

  <div class="form-info error-info">
    <p>Произошла ошибка при подключении к Telegram. Попробуйте снова или отключите интеграцию.</p>
  </div>

  <div class="button-group">
    <button class="retry-button" @click="handleRetry">
      Попробовать снова
    </button>
    <button class="disable-button" @click="handleDisableAndClose">
      Отключить
    </button>
  </div>
</div>
```

#### Button Actions

1. **Попробовать снова** (`retry-button`):
   - Сбрасывает состояние в `idle`
   - Очищает форму авторизации
   - Позволяет пользователю повторить попытку

2. **Отключить** (`disable-button`):
   - Вызывает `signOut()` для отключения Telegram
   - Закрывает модальное окно
   - Сбрасывает состояние Silero ошибки

#### Visual Design
- Красная иконка ошибки на белом круглом фоне
- Информационное сообщение с объяснением
- Две кнопки для выбора действия
- Синяя кнопка "Попробовать снова"
- Серая кнопка "Отключить"

### 3. Updated useTelegramAuth.ts Composable

#### Enhanced Error Handling
```typescript
// Clear error on successful sign in
async function signIn(code: string) {
  try {
    loading.value = true
    errorMessage.value = null
    state.value = 'loading'

    const result = await invoke<TelegramStatus>('telegram_sign_in', { code })

    status.value = result
    state.value = 'connected'
    errorMessage.value = null // Clear error on success
    return true
  } catch (error) {
    console.error('Failed to sign in:', error)
    errorMessage.value = error as string
    state.value = 'error'
    return false
  } finally {
    loading.value = false
  }
}
```

## CSS Styling

### Error Banner Styles
```css
.silero-error-banner {
  padding: 16px;
  background: #fee;
  border: 1px solid #fcc;
  border-left: 4px solid #f44;
  border-radius: 6px;
  margin-bottom: 16px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}
```

### Error State Card
```css
.provider-card.error-state {
  border-color: #ef4444;
  background: #fef2f2;
}
```

### Modal Error State
```css
.error-state {
  text-align: center;
  padding: 20px 0;
}

.error-icon-modal {
  width: 64px;
  height: 64px;
  margin: 0 auto 16px;
  background: #ef4444;
  color: white;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 32px;
  font-weight: bold;
}
```

## User Flow

### Error Scenario
1. **Происходит ошибка** при подключении к Telegram
2. **State обновляется** в `useTelegramAuth` → `state.value = 'error'`
3. **Watch срабатывает** в TtsPanel → `handleSileroError()`
4. **Отображается красная плашка** в карточке Silero
5. **Пользователь видит ошибку** и кнопку "Исправить"

### Fix Flow
1. **Пользователь нажимает "Исправить"**
2. **Открывается TelegramAuthModal** с состоянием ошибки
3. **Пользователь выбирает действие**:
   - **Попробовать снова**: Сброс состояния и повторная авторизация
   - **Отключить**: Отключение интеграции и закрытие модала

### Success Flow
1. **Успешная авторизация** через Telegram
2. **State обновляется** → `state.value = 'connected'`
3. **Ошибка очищается** → `errorMessage.value = null`
4. **Watch срабатывает** → `sileroError.value = null`
5. **Красная плашка исчезает**

## Error Sources

Ошибки могут возникать из следующих источников:
1. **Неверные credentials** (API ID, API Hash)
2. **Отсутствие подключения к интернету**
3. **Блокировка Telegram**
4. **Ошибка ввода кода**
5. **Сессия истекла или недействительна**

## Testing

### Manual Testing Steps
1. **Тест ошибки подключения**:
   - Введите неверные API credentials
   - Нажмите "Получить код"
   - Проверьте отображение красной плашки

2. **Тест кнопки "Исправить"**:
   - Нажмите "Исправить" на плашке
   - Проверьте открытие модала с ошибкой

3. **Тест "Попробовать снова"**:
   - В модале ошибки нажмите "Попробовать снова"
   - Проверьте очистку формы и возврат к вводу credentials

4. **Тест "Отключить"**:
   - В модале ошибки нажмите "Отключить"
   - Проверьте закрытие модала и сброс состояния

5. **Тест очистки ошибки при успехе**:
   - Авторизуйтесь успешно
   - Проверьте исчезновение красной плашки

## Files Modified

1. **D:\RustProjects\app-tts-v2\src\components\TtsPanel.vue**
   - Добавлено состояние `sileroError`
   - Добавлен watch для `telegramErrorMessage` и `telegramHasError`
   - Добавлен watch для очистки ошибки при успешном подключении
   - Добавлен CSS класс `.error-state` для карточки
   - Добавлен компонент `silero-error-banner`

2. **D:\RustProjects\app-tts-v2\src\components\TelegramAuthModal.vue**
   - Добавлено свойство `hasError` в destructuring
   - Добавлено состояние ошибки в шаблон
   - Добавлены функции `handleRetry()` и `handleDisableAndClose()`
   - Добавлены CSS стили для `.error-state`

3. **D:\RustProjects\app-tts-v2\src\composables\useTelegramAuth.ts**
   - Обновлена функция `signIn()` для очистки ошибки при успехе

## Build Status

✅ TypeScript compilation: **PASSED**
✅ Vue build: **SUCCESS**
✅ No TypeScript errors

## Integration with Existing Error Handling

Реализация следует существующим паттернам обработки ошибок в проекте:
- Использует похожие стили как в `AudioPanel.vue` (`.error-box`)
- Следует паттерну из `SettingsPanel.vue` для отображения ошибок
- Консистентные цвета: `#fee` фон, `#f44` акцент
- Та же структура error message с левым бордером

## Future Enhancements

Возможные улучшения в будущем:
1. **Автоматическое скрытие** плашки через N секунд
2. **История ошибок** для дебаггинга
3. **Подробное логирование** ошибок на бэкенд
4. **Восстановление сессии** при разрыве соединения
5. **Уведомления** в system tray при критических ошибках
