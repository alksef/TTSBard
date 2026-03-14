<script setup lang="ts">
import { onMounted, ref, watch, h } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { APP_VERSION } from '../version'
import {
  Volume2,
  Headphones,
  AppWindow,
  Music,
  Globe,
  BookOpen,
  ChevronLeft,
  ChevronRight,
  LogOut,
  ClipboardPenLine,
  Pencil,
  Settings
} from 'lucide-vue-next'

// Custom Twitch icon component
const TwitchIcon = (props: { size?: number }) => h('svg', {
  xmlns: 'http://www.w3.org/2000/svg',
  width: props.size || 20,
  height: props.size || 20,
  viewBox: '0 0 24 24',
  fill: 'currentColor'
}, [
  h('path', { d: 'M11.571 4.714h1.715v5.143H11.57zm4.715 0H18v5.143h-1.714zM6 0L1.714 4.286v15.428h5.143V24l4.286-4.286h3.428L22.286 12V0zm14.571 11.143l-3.428 3.428h-3.429l-3 3v-3H6.857V1.714h13.714Z' })
])

type Panel = 'info' | 'input' | 'tts' | 'floating' | 'soundpanel' | 'audio' | 'preprocessor' | 'webview' | 'twitch' | 'settings'

interface SidebarButton {
  id: Panel
  label: string
  icon: any
}

interface SidebarGroup {
  title?: string
  buttons: SidebarButton[]
}

const props = defineProps<{
  currentPanel: Panel
}>()

const emit = defineEmits<{
  setPanel: [panel: Panel]
}>()

function setPanel(panel: Panel) {
  emit('setPanel', panel)
}

async function quitApp() {
  try {
    await invoke('quit_app')
  } catch (e) {
    console.error('Failed to quit:', e)
  }
}

// Collapse state with localStorage persistence
const STORAGE_KEY = 'sidebar-collapsed'
const isCollapsed = ref(false)

onMounted(() => {
  const saved = localStorage.getItem(STORAGE_KEY)
  if (saved !== null) {
    isCollapsed.value = saved === 'true'
  }
})

watch(isCollapsed, (newValue) => {
  localStorage.setItem(STORAGE_KEY, String(newValue))
})

// Sidebar groups structure
const sidebarGroups: SidebarGroup[] = [
  {
    title: 'ГЛАВНОЕ',
    buttons: [
      { id: 'input', label: 'Текст', icon: Pencil },
      { id: 'info', label: 'Руководство', icon: BookOpen },
      { id: 'tts', label: 'TTS', icon: Volume2 },
      { id: 'audio', label: 'Аудио', icon: Headphones }
    ]
  },
  {
    title: 'Инструменты',
    buttons: [
      { id: 'floating', label: 'Плавающее окно', icon: AppWindow },
      { id: 'soundpanel', label: 'Звуковая панель', icon: Music }
    ]
  },
  {
    buttons: [
      { id: 'preprocessor', label: 'Быстрая вставка', icon: ClipboardPenLine }
    ]
  },
  {
    title: 'ИНТЕГРАЦИЯ',
    buttons: [
      { id: 'webview', label: 'WebView Source', icon: Globe },
      { id: 'twitch', label: 'Twitch Chat', icon: TwitchIcon }
    ]
  },
  {
    buttons: [
      { id: 'settings', label: 'Настройки', icon: Settings }
    ]
  }
]

function toggleCollapse() {
  isCollapsed.value = !isCollapsed.value
}
</script>

<template>
  <aside
    class="sidebar"
    :class="{ 'sidebar-collapsed': isCollapsed }"
  >
    <!-- Floating collapse button positioned outside sidebar -->
    <button
      class="collapse-toggle-floating"
      @click="toggleCollapse"
      :title="isCollapsed ? 'Развернуть' : 'Свернуть'"
    >
      <ChevronLeft v-if="!isCollapsed" :size="18" />
      <ChevronRight v-else :size="18" />
    </button>

    <nav class="sidebar-nav">
      <template v-for="(group, groupIndex) in sidebarGroups" :key="groupIndex">
        <div
          v-for="button in group.buttons"
          :key="button.id"
          class="sidebar-button-wrapper"
        >
          <button
            class="sidebar-button"
            :class="{ 'sidebar-button-active': props.currentPanel === button.id }"
            @click="setPanel(button.id)"
            :title="isCollapsed ? button.label : undefined"
          >
            <component :is="button.icon" :size="20" class="sidebar-icon" />
            <span v-if="!isCollapsed" class="sidebar-button-label">{{ button.label }}</span>
            <div v-if="props.currentPanel === button.id" class="active-indicator" />
          </button>
        </div>

        <div v-if="groupIndex < sidebarGroups.length - 1" class="sidebar-divider" />
      </template>
    </nav>

    <div class="sidebar-footer">
      <div v-if="!isCollapsed" class="version-info">{{ APP_VERSION }}</div>
      <button
        class="sidebar-button quit-button"
        @click="quitApp"
        :title="isCollapsed ? 'Выход' : undefined"
      >
        <LogOut :size="20" class="sidebar-icon" />
        <span v-if="!isCollapsed" class="sidebar-button-label">Выход</span>
      </button>
    </div>
  </aside>
</template>

<style scoped>
.sidebar {
  flex: 0 0 200px;
  width: 200px;
  min-width: 200px;
  position: relative;
  overflow: hidden;
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.03), transparent 22%),
    linear-gradient(180deg, rgba(17, 19, 26, 0.98) 0%, rgba(14, 16, 22, 0.96) 100%);
  color: var(--color-text-primary);
  display: flex;
  flex-direction: column;
  transition: width 0.28s ease, min-width 0.28s ease;
  box-shadow: inset -1px 0 0 rgba(255, 255, 255, 0.06);
}

.sidebar::before {
  content: '';
  position: absolute;
  inset: 0;
  pointer-events: none;
  background:
    radial-gradient(circle at top left, rgba(29, 140, 255, 0.18), transparent 30%),
    linear-gradient(rgba(255, 255, 255, 0.018) 1px, transparent 1px),
    linear-gradient(90deg, rgba(255, 255, 255, 0.014) 1px, transparent 1px);
  background-size: auto, 18px 18px, 18px 18px;
  mask-image: linear-gradient(to bottom, rgba(0, 0, 0, 0.95) 0%, rgba(0, 0, 0, 0.7) 78%, rgba(0, 0, 0, 0.92) 100%);
}

.sidebar-collapsed {
  flex-basis: 70px;
  width: 70px;
  min-width: 70px;
}

/* Floating collapse button positioned on right edge of sidebar */
.collapse-toggle-floating {
  position: absolute;
  right: -17px;
  top: calc(70% + 36px);
  transform: translateY(-50%);
  width: 34px;
  height: 34px;
  border: 1px solid rgba(255, 255, 255, 0.12);
  background:
    linear-gradient(135deg, rgba(30, 32, 45, 0.98), rgba(20, 22, 32, 0.96));
  color: var(--color-text-secondary);
  cursor: pointer;
  padding: 0;
  border-radius: 999px;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.25s ease;
  z-index: 1000;
  box-shadow:
    0 4px 16px rgba(0, 0, 0, 0.4),
    0 0 0 1px rgba(255, 255, 255, 0.04),
    inset 0 1px 0 rgba(255, 255, 255, 0.08);
}

.collapse-toggle-floating:hover {
  color: var(--color-text-primary);
  background:
    linear-gradient(135deg, rgba(40, 42, 58, 0.98), rgba(28, 30, 42, 0.96));
  border-color: rgba(42, 140, 255, 0.5);
  box-shadow:
    0 6px 24px rgba(0, 0, 0, 0.5),
    0 0 0 1px rgba(42, 140, 255, 0.3),
    0 0 20px rgba(42, 140, 255, 0.25),
    inset 0 1px 0 rgba(255, 255, 255, 0.12);
  transform: translateY(-50%) scale(1.06);
}

.sidebar-collapsed .collapse-toggle-floating {
  right: -17px;
}

.collapse-toggle-floating svg {
  transform: translateX(-5px);
}

.sidebar-nav {
  position: relative;
  z-index: 1;
  display: flex;
  flex-direction: column;
  flex: 1;
  overflow-y: auto;
  padding: 1rem 0 0.75rem;
}

.sidebar-button-wrapper {
  position: relative;
  margin-bottom: 0;
}

.sidebar-button {
  width: 100%;
  min-height: 30px;
  padding: 0 0.85rem 0 1rem;
  border: 1px solid transparent;
  background: rgba(255, 255, 255, 0.01);
  color: var(--color-text-secondary);
  cursor: pointer;
  text-align: left;
  transition: all 0.18s ease;
  display: flex;
  align-items: center;
  gap: 0.75rem;
  position: relative;
  border-radius: 0;
  backdrop-filter: blur(8px);
}

.sidebar-button:hover {
  background: rgba(255, 255, 255, 0.06);
  color: var(--color-text-primary);
  border-color: rgba(255, 255, 255, 0.08);
}

.sidebar-button-active {
  background: rgba(255, 255, 255, 0.09);
  border-color: rgba(255, 255, 255, 0.08);
  color: var(--color-text-primary) !important;
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.04);
}

.sidebar-button-active .sidebar-icon {
  color: var(--color-text-primary);
}

.active-indicator {
  position: absolute;
  left: 0;
  top: 50%;
  transform: translateY(-50%);
  width: 6px;
  height: 1.75rem;
  background: linear-gradient(180deg, #2aa9ff 0%, #0f74ff 100%);
  border-radius: 0 999px 999px 0;
  box-shadow: 0 0 16px rgba(29, 140, 255, 0.5);
}

.sidebar-divider {
  height: 1px;
  background: rgba(255, 255, 255, 0.08);
  margin: 1rem 0 0.85rem;
}

.sidebar-collapsed .sidebar-divider {
  margin: 0.75rem 0 0.6rem;
}

.sidebar-icon {
  min-width: 20px;
  flex-shrink: 0;
}

.sidebar-collapsed .sidebar-icon {
  margin: 0;
}

.sidebar-button-label {
  flex: 1;
  font-size: 0.92rem;
  font-weight: 600;
  line-height: 1;
  display: flex;
  align-items: center;
}

.sidebar-footer {
  position: relative;
  z-index: 1;
  padding: 0.7rem 0 0.85rem;
  border-top: 1px solid rgba(255, 255, 255, 0.08);
  display: flex;
  flex-direction: column;
  gap: 0.45rem;
  margin-top: auto;
}

.version-info {
  font-size: 0.76rem;
  color: var(--color-text-muted);
  font-family: var(--font-mono);
  padding: 0 1rem;
}

.quit-button {
  justify-content: center;
  color: var(--color-danger);
  background: rgba(255, 111, 105, 0.05);
  border-color: rgba(255, 111, 105, 0.12);
}

.quit-button:hover {
  background: rgba(255, 111, 105, 0.12);
  color: #ff8f8a;
}

.sidebar-collapsed .version-info {
  display: none;
}

.sidebar-collapsed .quit-button {
  justify-content: center;
  padding: 0.8rem;
}

.sidebar-collapsed .sidebar-nav {
  padding-left: 0;
  padding-right: 0;
}

.sidebar-collapsed .sidebar-button {
  justify-content: center;
  padding: 0;
}

.sidebar-collapsed .active-indicator {
  left: 0;
}

@media (max-width: 720px) {
  .sidebar,
  .sidebar-collapsed {
    width: 100%;
    min-width: 100%;
    flex-basis: auto;
  }

  .sidebar {
    box-shadow: inset 0 -1px 0 rgba(255, 255, 255, 0.08);
  }

  .sidebar-nav {
    padding-bottom: 1.2rem;
  }
}
</style>
