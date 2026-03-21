# План: Реализация вкладок в панели настроек

**Дата:** 2026-03-21
**Номер:** 53
**Задача:** Добавить вкладки в панель настроек (Общие, Сеть)

## Обзор

Добавить две вкладки в `SettingsPanel.vue`:
- **Общие** — общие настройки, редактор, тема, логирование
- **Сеть** — SOCKS5 и MTProxy прокси (перенос из `NetworkPanel.vue`)

**PreprocessorPanel НЕ трогать** — остаётся отдельной панелью "Быстрая вставка"

## Визуальный дизайн вкладок

```
┌─────────────────────────────────────────────────────────────┐
│  Настройки                                                  │
├─────────────────────────────────────────────────────────────┤
│  ┌──────────┐  ┌──────────┐                                 │
│  │ ⚙ Общие │  │ 🌐 Сеть  │                                 │
│  └──────────┘  └──────────┘                                 │
│  ═══════════════════════                                    │
│                                                             │
│  [содержимое активной вкладки]                              │
└─────────────────────────────────────────────────────────────┘
```

**Стили:**
- Иконки слева от текста (18px): Settings (⚙), Network (🌐)
- Активная вкладка: акцентный цвет `--color-accent`, подчёркивание снизу
- Неактивная: `--color-text-secondary`, при hover → `--color-text-primary`
- Плавная анимация переключения с fade-in эффектом

## Изменения в файлах

### 1. `src/components/SettingsPanel.vue`

**Добавить в `<script setup>`:**

```typescript
import { Settings, Network } from 'lucide-vue-next'

// Tab state
const activeTab = ref<'general' | 'network'>('general')

// === Proxy state (from NetworkPanel) ===
const host = ref<string>('')
const port = ref<string>('')
const username = ref<string>('')
const password = ref<string>('')
const showPassword = ref(false)
const mtHost = ref<string>('')
const mtPort = ref<string>('')
const mtSecret = ref<string>('')
const mtShowSecret = ref(false)
const mtDcId = ref<string>('')

const dcIdOptions = [
  { value: '', label: 'Авто' },
  { value: '1', label: '1' },
  { value: '2', label: '2' },
  { value: '3', label: '3' },
  { value: '4', label: '4' },
  { value: '5', label: '5' },
]

const isLoading = ref(false)
const isTestingSocks5 = ref(false)
const isTestingMtProxy = ref(false)
const isSaving = ref(false)
const socks5TestResult = ref<TestResult | null>(null)
const mtProxyTestResult = ref<TestResult | null>(null)
const statusMessage = ref<string>('')
const statusType = ref<'success' | 'error' | 'info'>('info')

let socks5TestTimeoutId: ReturnType<typeof setTimeout> | null = null
let mtProxyTestTimeoutId: ReturnType<typeof setTimeout> | null = null
let statusTimeoutId: ReturnType<typeof setTimeout> | null = null

// Types
interface ProxySettings {
  proxy_url: string | null
  proxy_type: 'socks5' | 'socks4' | 'http'
}

interface MtProxySettings {
  host?: string
  port: number
  secret?: string
  dc_id?: number
}

interface TestResult {
  success: boolean
  latency_ms: number | null
  mode: string
  error: string | null
}

// Computed
const hasProxyData = computed(() => host.value || port.value || username.value || password.value)
const hasMtProxyData = computed(() => mtHost.value || mtSecret.value || mtDcId.value)

const socks5Url = computed(() => {
  if (!host.value.trim()) return ''
  const portNum = port.value || '1080'
  let url = `socks5://`
  if (username.value) {
    const auth = password.value ? `${username.value}:${password.value}` : username.value
    url += `${auth}@`
  }
  url += `${host.value}:${portNum}`
  return url
})

// Functions from NetworkPanel
async function loadProxySettings() { /* ... */ }
function parseProxyUrl(url: string) { /* ... */ }
async function saveSettings() { /* ... */ }
async function testConnection() { /* ... */ }
async function loadMtProxySettings() { /* ... */ }
async function saveMtProxySettings() { /* ... */ }
async function testMtProxyConnection() { /* ... */ }
function showStatus(message: string, type: 'success' | 'error' | 'info') { /* ... */ }
function dismissStatus() { /* ... */ }

// Update onMounted to include proxy loading
onMounted(async () => {
  await loadProxySettings()
  await loadMtProxySettings()
})

// Add to onUnmounted
onUnmounted(() => {
  if (socks5TestTimeoutId) clearTimeout(socks5TestTimeoutId)
  if (mtProxyTestTimeoutId) clearTimeout(mtProxyTestTimeoutId)
  if (statusTimeoutId) clearTimeout(statusTimeoutId)
})
```

**Обновить `<template>`:**

```vue
<template>
  <div class="settings-panel">
    <!-- Error/Info Message Display (existing) -->
    <div v-if="errorMessage" class="message-box">...</div>

    <!-- Status Message from NetworkPanel -->
    <Transition name="fade">
      <div v-if="statusMessage" class="status-message" :class="statusType">
        <Check v-if="statusType === 'success'" :size="16" />
        <AlertTriangle v-else-if="statusType === 'error'" :size="16" />
        <Shield v-else :size="16" />
        <span>{{ statusMessage }}</span>
        <button class="status-close" @click="dismissStatus" title="Закрыть">
          <X :size="14" />
        </button>
      </div>
    </Transition>

    <!-- Tabs Navigation -->
    <div class="settings-tabs">
      <button
        :class="{ active: activeTab === 'general' }"
        @click="activeTab = 'general'"
      >
        <Settings :size="18" />
        <span>Общие</span>
      </button>
      <button
        :class="{ active: activeTab === 'network' }"
        @click="activeTab = 'network'"
      >
        <Network :size="18" />
        <span>Сеть</span>
      </button>
    </div>

    <!-- General Tab Content -->
    <div v-show="activeTab === 'general'" class="tab-content">
      <section class="settings-section">
        <h2>Общие настройки</h2>
        <!-- existing content -->
      </section>

      <section class="settings-section">
        <h2>Редактор</h2>
        <!-- existing content -->
      </section>

      <section class="settings-section">
        <h2>Внешний вид</h2>
        <!-- existing content -->
      </section>

      <section class="settings-section">
        <h2>Логирование</h2>
        <!-- existing content -->
      </section>
    </div>

    <!-- Network Tab Content -->
    <div v-show="activeTab === 'network'" class="tab-content">
      <div v-if="isLoading" class="loading-state">
        <Loader2 :size="24" class="spinner" />
        <span>Загрузка настроек...</span>
      </div>

      <div v-else class="network-content">
        <!-- SOCKS5 Section -->
        <section class="network-section">
          <div class="section-header">
            <h3>SOCKS5</h3>
          </div>
          <div class="network-form">
            <!-- form content from NetworkPanel -->
          </div>
        </section>

        <!-- MTProxy Section -->
        <section class="network-section">
          <div class="section-header">
            <h3>MTProxy</h3>
          </div>
          <div class="network-form">
            <!-- form content from NetworkPanel -->
          </div>
        </section>
      </div>
    </div>
  </div>
</template>
```

**Добавить в `<style scoped>`:**

```css
/* Tabs Navigation */
.settings-tabs {
  display: flex;
  gap: 0.5rem;
  margin-bottom: 1.5rem;
  border-bottom: 1px solid var(--color-border);
  padding-bottom: 0.5rem;
}

.settings-tabs button {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.5rem 1rem;
  background: transparent;
  border: none;
  border-radius: 8px 8px 0 0;
  color: var(--color-text-secondary);
  cursor: pointer;
  transition: all 0.2s ease;
  font-size: 0.9rem;
  font-weight: 500;
}

.settings-tabs button:hover {
  color: var(--color-text-primary);
  background: var(--color-bg-field-hover);
}

.settings-tabs button.active {
  color: var(--color-accent);
  background: var(--color-bg-field);
  border-bottom: 2px solid var(--color-accent);
}

/* Tab Content */
.tab-content {
  animation: fadeIn 0.2s ease;
}

@keyframes fadeIn {
  from { opacity: 0; transform: translateY(-5px); }
  to { opacity: 1; transform: translateY(0); }
}

/* Network styles (from NetworkPanel) */
.network-content {
  display: flex;
  flex-direction: column;
  gap: 20px;
}

.network-section {
  background: var(--color-bg-field);
  border: 1px solid var(--color-border);
  border-radius: 12px;
  padding: 12px 16px;
  backdrop-filter: blur(8px);
}

/* ... rest of NetworkPanel styles ... */
```

### 2. `src/components/Sidebar.vue`

**Убрать из `sidebarGroups`:**

```typescript
// Удалить эту кнопку:
{ id: 'proxy', label: 'Сеть', icon: Network }
```

**Оставить:**
```typescript
{ id: 'preprocessor', label: 'Быстрая вставка', icon: ClipboardPenLine }
```

**Убрать импорт `Network`** (если больше нигде не используется)

### 3. `src/App.vue`

**Убрать из imports:**
```typescript
import NetworkPanel from './components/NetworkPanel.vue'
```

**Убрать из template:**
```vue
<NetworkPanel v-show="currentPanel === 'proxy'" />
```

**Обновить тип `Panel`:**
```typescript
type Panel = 'info' | 'input' | 'tts' | 'floating' | 'soundpanel' | 'audio' | 'preprocessor' | 'webview' | 'twitch' | 'settings'
```
(убрать `'proxy'`)

### 4. `src/components/NetworkPanel.vue`

**Удалить файл** — функциональность полностью переносится в SettingsPanel

## Проверка

1. Запустить `npm run dev`
2. Открыть панель "Настройки"
3. Проверить переключение между вкладками "Общие" и "Сеть"
4. На вкладке "Общие" проверить:
   - Скрытие от захвата экрана
   - Быстрый редактор
   - Переключение темы (dark/light)
   - Логирование
5. На вкладке "Сеть" проверить:
   - SOCKS5: ввод host/port, сохранение, тест соединения
   - MTProxy: ввод настроек, сохранение, тест соединения
   - Status messages (success/error)
6. Проверить что в sidebar нет кнопки "Сеть"
7. Проверить что панель "Быстрая вставка" работает как раньше
8. Проверить темы (light/dark) для вкладок

## Файлы для изменения

| Файл | Действие |
|------|----------|
| `src/components/SettingsPanel.vue` | Модифицировать |
| `src/components/Sidebar.vue` | Модифицировать |
| `src/App.vue` | Модифицировать |
| `src/components/NetworkPanel.vue` | Удалить |

**НЕ трогать:**
- `src/components/PreprocessorPanel.vue`

## CSS переменные для использования

- `--color-accent` — акцентный цвет для активной вкладки
- `--color-bg-field` — фон вкладки и секций
- `--color-bg-field-hover` — hover состояние
- `--color-border` — границы
- `--color-text-primary` — основной текст
- `--color-text-secondary` — неактивная вкладка
