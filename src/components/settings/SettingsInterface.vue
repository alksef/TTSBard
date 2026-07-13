<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { Moon, Sun } from 'lucide-vue-next';
import type { Theme } from '../../types/settings';
import { useGeneralSettings, useWindowsSettings } from '../../composables/useAppSettings';

const generalSettings = useGeneralSettings();
const windowsSettings = useWindowsSettings();

const emit = defineEmits<{
  (e: 'show-message', message: string): void;
}>();

function showError(message: string) {
  emit('show-message', message);
}

// ==================== Local state ====================

// Main window
const mainCustomBackground = ref(false);
const mainBgColor = ref('#10131a');
const mainOpacity = ref(100);
const mainCustomOpacity = ref(false);
const mainOpacityCompactOnly = ref(false);

// Sound panel
const spSource = ref<'main' | 'own'>('own');
const spBgColor = ref('#2a2a2a');
const spOpacity = ref(90);
const spHideOnBlur = ref(true);

// Playback control
const pbSource = ref<'main' | 'own'>('own');
const pbBgColor = ref('#10131a');
const pbOpacity = ref(94);

const spOwnDisabled = computed(() => spSource.value === 'main');
const pbOwnDisabled = computed(() => pbSource.value === 'main');
const mainOpacityDisabled = computed(() => !mainCustomOpacity.value);
const mainCompactOnlyDisabled = computed(() => !mainCustomOpacity.value);

// ==================== Theme ====================

async function setTheme(theme: Theme) {
  try {
    await invoke('update_theme', { theme });
  } catch (e) {
    showError('Ошибка изменения темы: ' + (e as Error).message);
  }
}

// ==================== Main window ====================

async function toggleMainCustomBackground() {
  const newValue = !mainCustomBackground.value;
  mainCustomBackground.value = newValue;
  try {
    await invoke('set_main_custom_background', { value: newValue });
  } catch (e) {
    mainCustomBackground.value = !newValue;
    showError('Ошибка сохранения настройки: ' + (e as Error).message);
  }
}

async function saveMainBgColor() {
  try {
    await invoke('set_main_bg_color', { color: mainBgColor.value });
  } catch (e) {
    showError('Ошибка сохранения цвета: ' + (e as Error).message);
  }
}

async function saveMainOpacity() {
  try {
    await invoke('set_main_opacity', { value: mainOpacity.value });
  } catch (e) {
    showError('Ошибка сохранения прозрачности: ' + (e as Error).message);
  }
}

async function toggleMainCustomOpacity() {
  const newValue = !mainCustomOpacity.value;
  mainCustomOpacity.value = newValue;
  try {
    await invoke('set_main_custom_opacity', { value: newValue });
  } catch (e) {
    mainCustomOpacity.value = !newValue;
    showError('Ошибка сохранения настройки: ' + (e as Error).message);
  }
}

async function toggleMainOpacityCompactOnly() {
  const newValue = !mainOpacityCompactOnly.value;
  mainOpacityCompactOnly.value = newValue;
  try {
    await invoke('set_main_opacity_compact_only', { value: newValue });
  } catch (e) {
    mainOpacityCompactOnly.value = !newValue;
    showError('Ошибка сохранения настройки: ' + (e as Error).message);
  }
}

// ==================== Sound panel ====================

async function setSpSource(source: 'main' | 'own') {
  const previous = spSource.value;
  spSource.value = source;
  try {
    await invoke('set_soundpanel_appearance_source', { source });
  } catch (e) {
    spSource.value = previous;
    showError('Ошибка сохранения источника оформления: ' + (e as Error).message);
  }
}

async function saveSpBgColor() {
  try {
    await invoke('sp_set_floating_bg_color', { color: spBgColor.value });
  } catch (e) {
    showError('Ошибка сохранения цвета: ' + (e as Error).message);
  }
}

async function saveSpOpacity() {
  try {
    await invoke('sp_set_floating_opacity', { value: spOpacity.value });
  } catch (e) {
    showError('Ошибка сохранения прозрачности: ' + (e as Error).message);
  }
}

async function toggleSpHideOnBlur() {
  const newValue = !spHideOnBlur.value;
  spHideOnBlur.value = newValue;
  try {
    await invoke('sp_set_hide_on_blur', { enabled: newValue });
  } catch (e) {
    spHideOnBlur.value = !newValue;
    showError('Ошибка сохранения настройки: ' + (e as Error).message);
  }
}

// ==================== Playback control ====================

async function setPbSource(source: 'main' | 'own') {
  const previous = pbSource.value;
  pbSource.value = source;
  try {
    await invoke('set_playback_appearance_source', { source });
  } catch (e) {
    pbSource.value = previous;
    showError('Ошибка сохранения источника оформления: ' + (e as Error).message);
  }
}

async function savePbBgColor() {
  try {
    await invoke('pc_set_bg_color', { color: pbBgColor.value });
  } catch (e) {
    showError('Ошибка сохранения цвета: ' + (e as Error).message);
  }
}

async function savePbOpacity() {
  try {
    await invoke('pc_set_opacity', { value: pbOpacity.value });
  } catch (e) {
    showError('Ошибка сохранения прозрачности: ' + (e as Error).message);
  }
}

// ==================== Sync from backend ====================

watch(
  windowsSettings,
  (settings) => {
    if (!settings) return;
    mainCustomBackground.value = settings.main.custom_background;
    mainBgColor.value = settings.main.bg_color;
    mainOpacity.value = settings.main.opacity;
    mainCustomOpacity.value = settings.main.custom_opacity;
    mainOpacityCompactOnly.value = settings.main.opacity_compact_only;

    spSource.value = settings.soundpanel.appearance_source === 'main' ? 'main' : 'own';
    spBgColor.value = settings.soundpanel.bg_color;
    spOpacity.value = settings.soundpanel.opacity;
    spHideOnBlur.value = settings.soundpanel.hide_on_blur;

    pbSource.value = settings.playback.appearance_source === 'main' ? 'main' : 'own';
    pbBgColor.value = settings.playback.bg_color;
    pbOpacity.value = settings.playback.opacity;
  },
  { immediate: true, deep: true }
);
</script>

<template>
  <div class="settings-interface">
    <!-- Theme -->
    <section class="settings-section">
      <div class="theme-selector">
        <label class="theme-option" :class="{ active: generalSettings?.theme === 'dark' }">
          <input
            type="radio"
            value="dark"
            :checked="generalSettings?.theme === 'dark'"
            @change="setTheme('dark')"
          />
          <Moon :size="16" />
          <span>Тёмная</span>
        </label>

        <label class="theme-option" :class="{ active: generalSettings?.theme === 'light' }">
          <input
            type="radio"
            value="light"
            :checked="generalSettings?.theme === 'light'"
            @change="setTheme('light')"
          />
          <Sun :size="16" />
          <span>Светлая</span>
        </label>
      </div>
    </section>

    <!-- Main window -->
    <section class="settings-section">
      <h2 class="section-title">Главное окно</h2>

      <div class="setting-row">
        <label class="setting-label checkbox-label">
          <input
            :checked="mainCustomBackground"
            type="checkbox"
            class="checkbox-input"
            @change="toggleMainCustomBackground"
          />
          <span>Использовать свой цвет</span>
        </label>
        <span class="setting-hint">Если выключено, используется цвет активной темы</span>
      </div>

      <div class="setting-row" style="margin-bottom: 0.75rem">
        <label class="setting-label checkbox-label">
          <input
            :checked="mainCustomOpacity"
            type="checkbox"
            class="checkbox-input"
            @change="toggleMainCustomOpacity"
          />
          <span>Использовать свою прозрачность</span>
        </label>
      </div>

      <div class="appearance-grid" style="margin-bottom: 0.5rem">
        <div class="appearance-column">
          <label class="setting-label">Цвет</label>
          <div class="appearance-controls">
            <input
              v-model="mainBgColor"
              type="color"
              class="color-input"
              :disabled="!mainCustomBackground"
              @change="saveMainBgColor"
            />
            <input
              v-model="mainBgColor"
              type="text"
              placeholder="#10131a"
              class="text-input color-text"
              maxlength="7"
              :disabled="!mainCustomBackground"
              @blur="saveMainBgColor"
              @keyup.enter="saveMainBgColor"
            />
          </div>
        </div>

        <div class="appearance-column">
          <label class="setting-label">Прозрачность</label>
          <div class="appearance-controls">
            <input
              v-model.number="mainOpacity"
              type="range"
              min="10"
              max="100"
              step="5"
              class="slider-input inline-slider"
              :disabled="mainOpacityDisabled"
              @change="saveMainOpacity"
            />
            <span class="opacity-value">{{ mainOpacity }}%</span>
          </div>
        </div>
      </div>

      <div class="setting-row indent-row" :class="{ 'row-disabled': mainCompactOnlyDisabled }">
        <label class="setting-label checkbox-label">
          <input
            :checked="mainOpacityCompactOnly"
            type="checkbox"
            class="checkbox-input"
            :disabled="mainCompactOnlyDisabled"
            @change="toggleMainOpacityCompactOnly"
          />
          <span>Применять прозрачность только в компактном режиме</span>
        </label>
        <span class="setting-hint">В обычном режиме окно будет полностью непрозрачным</span>
      </div>
    </section>

    <!-- Sound panel -->
    <section class="settings-section">
      <h2 class="section-title">Звуковая панель</h2>

      <div class="setting-row">
        <label class="setting-label checkbox-label">
          <input
            :checked="spSource === 'own'"
            type="checkbox"
            class="checkbox-input"
            @change="setSpSource(($event.target as HTMLInputElement).checked ? 'own' : 'main')"
          />
          <span>Использовать свой цвет</span>
        </label>
        <span class="setting-hint">Если выключено, используется цвет активной темы</span>
      </div>

      <div class="appearance-grid">
        <div class="appearance-column">
          <label class="setting-label">Цвет</label>
          <div class="appearance-controls">
            <input
              v-model="spBgColor"
              type="color"
              class="color-input"
              :disabled="spOwnDisabled"
              @change="saveSpBgColor"
            />
            <input
              v-model="spBgColor"
              type="text"
              placeholder="#2a2a2a"
              class="text-input color-text"
              maxlength="7"
              :disabled="spOwnDisabled"
              @blur="saveSpBgColor"
              @keyup.enter="saveSpBgColor"
            />
          </div>
        </div>

        <div class="appearance-column">
          <label class="setting-label">Прозрачность</label>
          <div class="appearance-controls">
            <input
              v-model.number="spOpacity"
              type="range"
              min="10"
              max="100"
              step="5"
              class="slider-input inline-slider"
              :disabled="spOwnDisabled"
              @change="saveSpOpacity"
            />
            <span class="opacity-value">{{ spOpacity }}%</span>
          </div>
        </div>
      </div>

      <div class="setting-row" style="margin-top: 0.75rem">
        <label class="setting-label checkbox-label">
          <input
            :checked="spHideOnBlur"
            type="checkbox"
            class="checkbox-input"
            @change="toggleSpHideOnBlur"
          />
          <span>Скрывать при потере фокуса</span>
        </label>
        <span class="setting-hint">Выключите, чтобы настроить положение панели перетаскиванием</span>
      </div>
    </section>

    <!-- Playback control -->
    <section class="settings-section">
      <h2 class="section-title">Управление воспроизведением</h2>

      <div class="setting-row">
        <label class="setting-label checkbox-label">
          <input
            :checked="pbSource === 'own'"
            type="checkbox"
            class="checkbox-input"
            @change="setPbSource(($event.target as HTMLInputElement).checked ? 'own' : 'main')"
          />
          <span>Использовать свой цвет</span>
        </label>
        <span class="setting-hint">Если выключено, используется цвет активной темы</span>
      </div>

      <div class="appearance-grid">
        <div class="appearance-column">
          <label class="setting-label">Цвет</label>
          <div class="appearance-controls">
            <input
              v-model="pbBgColor"
              type="color"
              class="color-input"
              :disabled="pbOwnDisabled"
              @change="savePbBgColor"
            />
            <input
              v-model="pbBgColor"
              type="text"
              placeholder="#10131a"
              class="text-input color-text"
              maxlength="7"
              :disabled="pbOwnDisabled"
              @blur="savePbBgColor"
              @keyup.enter="savePbBgColor"
            />
          </div>
        </div>

        <div class="appearance-column">
          <label class="setting-label">Прозрачность</label>
          <div class="appearance-controls">
            <input
              v-model.number="pbOpacity"
              type="range"
              min="10"
              max="100"
              step="5"
              class="slider-input inline-slider"
              :disabled="pbOwnDisabled"
              @change="savePbOpacity"
            />
            <span class="opacity-value">{{ pbOpacity }}%</span>
          </div>
        </div>
      </div>
    </section>
  </div>
</template>

<style scoped>
.settings-interface {
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
}

.settings-section {
  padding: 12px 16px;
  background: var(--color-bg-field);
  border: 1px solid var(--color-border);
  border-radius: 12px;
  backdrop-filter: blur(8px);
}

.section-title {
  margin: 0 0 1rem;
  font-size: 1.05rem;
  color: var(--color-text-primary);
}

.setting-row {
  display: block;
  margin-bottom: 1rem;
}

.setting-row:last-child {
  margin-bottom: 0;
}

.setting-label {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  font-size: 0.95rem;
  font-weight: 600;
  color: var(--color-text-primary);
  margin-bottom: 0.5rem;
}

.checkbox-label {
  cursor: pointer;
  user-select: none;
  margin-bottom: 0;
}

.checkbox-input {
  width: 18px;
  height: 18px;
  cursor: pointer;
  accent-color: var(--color-accent);
}

.setting-hint {
  display: block;
  margin-top: 0.4rem;
  margin-left: 2.4rem;
  font-size: 0.85rem;
  color: var(--color-text-muted);
  line-height: 1.4;
}

.appearance-grid {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 2fr);
  gap: 1rem;
}

.appearance-column {
  min-width: 0;
}

.appearance-controls {
  display: flex;
  gap: 0.75rem;
  align-items: center;
  flex-wrap: wrap;
}

.color-input {
  width: 50px;
  height: 36px;
  border: 1px solid var(--color-border-strong);
  border-radius: 10px;
  cursor: pointer;
  padding: 0;
  background: transparent;
}

.color-input:disabled {
  cursor: not-allowed;
  opacity: 0.4;
}

.color-text {
  width: 95px;
  font-family: var(--font-mono);
  text-transform: uppercase;
}

.text-input {
  padding: 0.6rem;
  border: 1px solid var(--color-border-strong);
  border-radius: 10px;
  font-size: 1rem;
  background: var(--color-bg-field);
  color: var(--color-text-primary);
}

.text-input:disabled {
  cursor: not-allowed;
  opacity: 0.4;
}

.slider-input {
  cursor: pointer;
  accent-color: var(--color-accent);
}

.slider-input:disabled {
  cursor: not-allowed;
  opacity: 0.4;
}

.inline-slider {
  width: 150px;
  flex: 1;
  min-width: 100px;
}

.opacity-value {
  font-size: 0.9rem;
  color: var(--color-text-secondary);
  min-width: 45px;
}

.source-select {
  padding: 0.4rem 0.6rem;
  background: var(--color-bg-field-hover);
  border: 1px solid var(--color-border-strong);
  border-radius: 6px;
  color: var(--color-text-primary);
  font-size: 0.9rem;
  cursor: pointer;
  min-width: 200px;
}

.source-select:focus {
  outline: none;
  border-color: var(--color-accent);
  box-shadow: 0 0 0 2px var(--focus-glow);
}

.source-select option {
  background: var(--select-bg);
  color: var(--color-text-primary);
}

/* Theme selector */
.theme-selector {
  display: flex;
  gap: 1rem;
}

.theme-option {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.5rem 1rem;
  background: var(--color-bg-field);
  border: 1px solid var(--color-border);
  border-radius: 8px;
  cursor: pointer;
  user-select: none;
  transition: all 0.2s ease;
  font-size: 0.9rem;
  font-weight: 500;
  color: var(--color-text-secondary);
}

.theme-option:hover {
  background: var(--color-bg-field-hover);
  border-color: var(--color-border-strong);
}

.theme-option.active {
  background: var(--btn-accent-bg);
  border-color: var(--color-accent);
  color: var(--color-text-primary);
}

.theme-option input[type="radio"] {
  display: none;
}

.indent-row {
  margin-left: 1.6rem;
}

.row-disabled {
  opacity: 0.4;
  pointer-events: none;
}
</style>
