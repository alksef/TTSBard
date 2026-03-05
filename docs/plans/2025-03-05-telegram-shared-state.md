# Shared Telegram Auth State via provide/inject

**Дата:** 2025-03-05
**Статус:** 🔄 In Progress

## Описание проблемы

**Текущее поведение:**

1. Каждый компонент создаёт свой собственный экземпляр `useTelegramAuth()`:
   - `App.vue` → отдельный экземпляр
   - `TtsPanel.vue` → отдельный экземпляр
   - `TelegramAuthModal.vue` → отдельный экземпляр

2. Каждый экземпляр имеет своё независимое состояние (state, status, errorMessage, etc.)

3. `init()` вызывается в нескольких местах:
   - В `App.vue` при запуске приложения
   - В `TtsPanel.vue` при монтировании компонента

4. При открытии вкладки TTS каждый раз вызывается `telegram_auto_restore` и выводится сообщение `[TELEGRAM] Session auto-restored successfully`

**Проблемы:**
- ❌ Дублирование состояния между компонентами
- ❌ Лишние вызовы `telegram_auto_restore`
- ❌ Отсутствие синхронизации состояния
- ❌ Нарушение принципа Single Source of Truth

## Решение

Использовать паттерн **provide/inject** для создания единого разделяемого экземпляра `useTelegramAuth` на уровне приложения.

### Архитектура

```
App.vue (provide)
    ↓
    ├─→ TtsPanel.vue (inject)
    └─→ TelegramAuthModal.vue (inject)
```

Все компоненты используют **один и тот же экземпляр** с единым состоянием.

## Шаги реализации

### Шаг 1: Обновить `src/App.vue`

**До:**
```vue
<script setup lang="ts">
const { init: initTelegram } = useTelegramAuth()

onMounted(async () => {
  await initTelegram()
})
</script>
```

**После:**
```vue
<script setup lang="ts">
import { provide } from 'vue'

// Provide key для inject
const TELEGRAM_AUTH_KEY = Symbol('telegramAuth')

// Создаём единственный экземпляр
const telegramAuth = useTelegramAuth()

// Предоставляем всем дочерним компонентам
provide(TELEGRAM_AUTH_KEY, telegramAuth)

onMounted(async () => {
  await telegramAuth.init()
})
</script>
```

### Шаг 2: Обновить `src/composables/useTelegramAuth.ts`

Экспортировать ключ provide для использования в других компонентах:

```typescript
// Добавить в конец файла
export const TELEGRAM_AUTH_KEY = Symbol('telegramAuth')
```

### Шаг 3: Обновить `src/components/TtsPanel.vue`

**До:**
```vue
<script setup lang="ts">
import { useTelegramAuth } from '../composables/useTelegramAuth';

const {
  status: telegramStatus,
  isConnected: telegramConnected,
  init: initTelegram,
  // ...
} = useTelegramAuth();

onMounted(async () => {
  await initTelegram();  // <-- Убрать
})
</script>
```

**После:**
```vue
<script setup lang="ts">
import { inject } from 'vue'
import { TELEGRAM_AUTH_KEY, type UseTelegramAuthReturn } from '../composables/useTelegramAuth'

// Используем предоставленный экземпляр
const telegramAuth = inject<UseTelegramAuthReturn>(TELEGRAM_AUTH_KEY)!

const {
  status: telegramStatus,
  isConnected: telegramConnected,
  // init: initTelegram,  // <-- Больше не нужен
  // ...
} = telegramAuth

onMounted(async () => {
  // init() уже вызван в App.vue
  // await initTelegram()  // <-- УБРАНО
})
</script>
```

### Шаг 4: Обновить `src/components/TelegramAuthModal.vue`

Аналогично `TtsPanel.vue`:

**До:**
```vue
<script setup lang="ts">
import { useTelegramAuth } from '../composables/useTelegramAuth'

const {
  canInit,
  requestCode,
  signIn,
  signOut,
  reset,
} = useTelegramAuth()
</script>
```

**После:**
```vue
<script setup lang="ts">
import { inject } from 'vue'
import { TELEGRAM_AUTH_KEY, type UseTelegramAuthReturn } from '../composables/useTelegramAuth'

const telegramAuth = inject<UseTelegramAuthReturn>(TELEGRAM_AUTH_KEY)!

const {
  canInit,
  requestCode,
  signIn,
  signOut,
  reset,
} = telegramAuth
</script>
```

### Шаг 5: Добавить TypeScript тип для возвращаемого значения

В `src/composables/useTelegramAuth.ts` добавить тип для удобного использования:

```typescript
export type UseTelegramAuthReturn = ReturnType<typeof useTelegramAuth>
```

## Проверка

После реализации:

1. ✅ Сообщение `[TELEGRAM] Session auto-restored successfully` появляется **только один раз** при запуске приложения
2. ✅ При переключении между вкладками состояние Telegram сохраняется
3. ✅ Авторизация в модальном окне обновляет состояние в `TtsPanel`
4. ✅ Нет дублирования состояния между компонентами

## Файлы для изменения

1. `src/App.vue` - добавить `provide`
2. `src/composables/useTelegramAuth.ts` - экспортировать ключ и тип
3. `src/components/TtsPanel.vue` - использовать `inject`
4. `src/components/TelegramAuthModal.vue` - использовать `inject`

## Риски

- **Низкий риск:** Изменения локальны, не затрагивают backend логику
- **Type safety:** Нужно правильно типизировать `inject` чтобы избежать `undefined`
- **Testing:** Убедиться, что все компоненты корректно получают экземпляр

## Заметки

- Ключ provide/inject лучше сделать Symbol для избежания коллизий
- Можно добавить проверку на существование значения при inject для дополнительной безопасности
- `init()` вызывается только в `App.vue` при старте приложения
