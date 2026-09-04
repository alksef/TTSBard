<script setup lang="ts">
import { computed, watch, ref, onMounted, onUnmounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { Play, Loader2, Check, RefreshCw, ListRestart } from 'lucide-vue-next';
import { useEditorSettings } from '../../composables/useAppSettings';
import { useRuAccentRuntime } from '../../composables/useRuAccentRuntime';
import type { HomographAccentorPackDto, QuickEditorMode } from '../../types/settings';
import { normalizeTypingTimeout } from '../../utils/validateTypingTimeout';
import EditorFontSettings from './EditorFontSettings.vue';

const editorSettings = useEditorSettings();

const emit = defineEmits<{
  (e: 'show-message', message: string): void;
}>();

const quickEditorMode = computed<QuickEditorMode>(() => editorSettings.value?.quick ?? 'disabled');

const spellcheckEnabled = computed(() => editorSettings.value?.spellcheck_enabled ?? true)

const keepTextAfterSend = computed(() => editorSettings.value?.keep_text_after_send ?? false)

const typingTimeoutInput = ref(editorSettings.value?.typing_idle_timeout_ms ?? 800)

watch(() => editorSettings.value?.typing_idle_timeout_ms, (newVal) => {
  if (newVal !== undefined) {
    typingTimeoutInput.value = newVal
  }
})

onUnmounted(() => {
  typingTimeoutInput.value = editorSettings.value?.typing_idle_timeout_ms ?? 800
})

const quickModeOptions: { value: QuickEditorMode; label: string }[] = [
  { value: 'disabled', label: 'Отключено' },
  { value: 'collapse', label: 'Сворачивать' },
  { value: 'return_focus', label: 'Возвращать фокус предыдущему окну' },
]

async function onTypingTimeoutChange() {
  const raw = typingTimeoutInput.value
  const normalized = normalizeTypingTimeout(raw)
  if (normalized === null) {
    typingTimeoutInput.value = editorSettings.value?.typing_idle_timeout_ms ?? 800
    return
  }
  typingTimeoutInput.value = normalized
  try {
    await invoke('set_editor_typing_idle_timeout_ms', { ms: normalized })
    emit('show-message', 'Настройка сохранена')
  } catch (e) {
    const errorMessage = e instanceof Error ? e.message : String(e)
    emit('show-message', 'Ошибка сохранения задержки набора: ' + errorMessage)
    typingTimeoutInput.value = editorSettings.value?.typing_idle_timeout_ms ?? 800
  }
}

async function setQuickMode(mode: QuickEditorMode) {
  try {
    await invoke('set_editor_quick', { value: mode });
    emit('show-message', 'Настройка сохранена');
  } catch (e) {
    const errorMessage = e instanceof Error ? e.message : String(e);
    emit('show-message', 'Ошибка переключения быстрого редактора: ' + errorMessage);
  }
}

async function toggleSpellcheck() {
  try {
    const newValue = !(editorSettings.value?.spellcheck_enabled ?? true)
    await invoke('set_editor_spellcheck_enabled', { value: newValue })
    emit('show-message', 'Настройка сохранена')
  } catch (e) {
    const errorMessage = e instanceof Error ? e.message : String(e)
    emit('show-message', 'Ошибка переключения орфографии: ' + errorMessage)
  }
}

async function toggleKeepText() {
  try {
    const newValue = !(editorSettings.value?.keep_text_after_send ?? false)
    await invoke('set_editor_keep_text', { enabled: newValue })
    emit('show-message', 'Настройка сохранена')
  } catch (e) {
    const errorMessage = e instanceof Error ? e.message : String(e)
    emit('show-message', 'Ошибка переключения сохранения текста: ' + errorMessage)
  }
}

const homographAccentor = computed(() => editorSettings.value?.homograph_accentor)
const accentorEnabled = computed(() => homographAccentor.value?.enabled ?? false)
const accentorLoadOnStart = computed(() => homographAccentor.value?.load_on_start ?? false)

function formatAccentorPackLabel(pack: HomographAccentorPackDto): string {
  return pack.display_name.trim()
}

const { statusFor, load: loadRuAccentModel, refreshPacks, rescanPacks, packLabelFor } = useRuAccentRuntime()

const accentorPacks = ref<HomographAccentorPackDto[]>([])
const selectedPackId = ref<string>('')
const accentorRescanPending = ref(false)

function syncAccentorFromSettings() {
  const persistedId = homographAccentor.value?.accentor_pack_id ?? ''
  if (selectedPackId.value !== persistedId) {
    selectedPackId.value = persistedId
  }
}

watch(homographAccentor, () => syncAccentorFromSettings(), { immediate: true })

const accentorRuntimeStatus = statusFor(selectedPackId)
const accentorReady = computed(() => accentorRuntimeStatus.value === 'ready')

const noPacks = computed(() => accentorPacks.value.length === 0)

const accentorInMemory = computed(() => {
  const id = selectedPackId.value
  if (!id) return false
  if (accentorPacks.value.some((pack) => pack.id === id)) return false
  const status = accentorRuntimeStatus.value
  return status === 'loading' || status === 'ready'
})

const accentorSelectedMissing = computed(() => {
  const id = selectedPackId.value
  if (!id) return false
  return !accentorPacks.value.some((pack) => pack.id === id)
})

const accentorStatusText = computed(() => {
  switch (accentorRuntimeStatus.value) {
    case 'not_loaded':
      return 'Не загружена'
    case 'loading':
      return 'Загружается'
    case 'ready':
      return 'Готова'
    case 'failed':
      return 'Ошибка загрузки'
    default:
      return ''
  }
})

const accentorLoadTitle = computed(() => {
  switch (accentorRuntimeStatus.value) {
    case 'loading':
      return 'Модель RUAccent загружается'
    case 'ready':
      return 'Модель RUAccent загружена'
    case 'failed':
      return 'Повторить загрузку модели RUAccent'
    default:
      return 'Загрузить модель RUAccent'
  }
})

async function loadAccentorPacks() {
  try {
    accentorPacks.value = await refreshPacks()
  } catch (e) {
    const errorMessage = e instanceof Error ? e.message : String(e)
    emit('show-message', 'Ошибка загрузки моделей RUAccent: ' + errorMessage)
  }
}

async function rescanAccentorPacks() {
  if (accentorRescanPending.value) return
  accentorRescanPending.value = true
  try {
    accentorPacks.value = await rescanPacks()
  } catch (e) {
    const errorMessage = e instanceof Error ? e.message : String(e)
    emit('show-message', 'Ошибка обновления списка моделей RUAccent: ' + errorMessage)
  } finally {
    accentorRescanPending.value = false
  }
}

onMounted(async () => {
  await loadAccentorPacks()
  syncAccentorFromSettings()
})

async function saveAccentor(enabled: boolean, packId: string | null) {
  try {
    await invoke('set_editor_homograph_accentor', { enabled, accentorPackId: packId })
    emit('show-message', 'Настройка сохранена')
    await loadAccentorPacks()
  } catch (e) {
    const errorMessage = e instanceof Error ? e.message : String(e)
    emit('show-message', 'Ошибка сохранения ударений: ' + errorMessage)
    syncAccentorFromSettings()
  }
}

async function toggleAccentor() {
  const next = !accentorEnabled.value
  let packId: string | null = selectedPackId.value || null
  if (next && !packId && accentorPacks.value.length > 0) {
    packId = accentorPacks.value[0].id
    selectedPackId.value = packId
  }
  await saveAccentor(next, packId)
}

function onPackSelect(event: Event) {
  const value = (event.target as HTMLSelectElement).value
  selectedPackId.value = value
  void saveAccentor(accentorEnabled.value, value || null)
}

async function loadSelectedModel() {
  if (
    !selectedPackId.value ||
    accentorSelectedMissing.value ||
    accentorRuntimeStatus.value === 'loading' ||
    accentorRuntimeStatus.value === 'ready'
  ) {
    return
  }
  await loadRuAccentModel(selectedPackId.value)
}

async function toggleLoadOnStart() {
  try {
    const next = !(homographAccentor.value?.load_on_start ?? false)
    await invoke('set_editor_homograph_accentor_load_on_start', { loadOnStart: next })
    emit('show-message', 'Настройка сохранена')
  } catch (e) {
    const errorMessage = e instanceof Error ? e.message : String(e)
    emit('show-message', 'Ошибка сохранения автозагрузки: ' + errorMessage)
  }
}

watch(editorSettings, (newSettings) => {
  if (!newSettings) return;
}, { immediate: true });
</script>

<template>
  <div class="settings-editor">
    <EditorFontSettings />

    <section class="settings-section">
      <div class="card-header">
        <h3 class="card-title">Быстрый редактор</h3>
        <p class="card-desc">Реакция на Enter, Esc</p>
      </div>
      <div class="setting-row" v-for="opt in quickModeOptions" :key="opt.value">
        <label class="setting-label radio-label">
          <input
            type="radio"
            :value="opt.value"
            :checked="quickEditorMode === opt.value"
            class="radio-input"
            @change="setQuickMode(opt.value)"
          />
          <span>{{ opt.label }}</span>
        </label>
        <span v-if="opt.value === 'return_focus'" class="setting-hint">
          Работает только если окно было вызвано по горячей клавише
        </span>
      </div>
      <div class="setting-row">
        <label class="setting-label checkbox-label">
          <input
            :checked="keepTextAfterSend"
            type="checkbox"
            class="checkbox-input"
            @change="toggleKeepText"
          />
          <span>Не очищать текст после отправки</span>
        </label>
        <span class="setting-hint">
          Отправленный текст остаётся в редакторе для правки или повторной отправки (Alt+Enter инвертирует разово)
        </span>
      </div>
    </section>

    <section class="settings-section">
      <div class="setting-row">
        <label class="setting-label checkbox-label">
          <input
            :checked="spellcheckEnabled"
            type="checkbox"
            class="checkbox-input"
            @change="toggleSpellcheck"
          />
          <span>Проверка орфографии (офлайн)</span>
        </label>
        <span class="setting-hint">
          Подчёркивает ошибки и предлагает варианты исправления. Работает без сети (локальный словарь)
        </span>
      </div>
    </section>

    <section class="settings-section">
      <div class="card-header">
        <h3 class="card-title">Статус набора</h3>
        <p class="card-desc">Через сколько мс без правок завершать набор для VTube Studio и WebView</p>
      </div>
      <div class="setting-row typing-row">
        <label class="setting-label">Задержка (мс):</label>
        <input
          type="number"
          v-model="typingTimeoutInput"
          @change="onTypingTimeoutChange"
          class="number-input"
          :min="200"
          :max="5000"
          :step="100"
        />
        <span class="setting-hint typing-hint">
          Начало набора передаётся сразу, задержка отсчитывается после последней пользовательской правки.
        </span>
      </div>
    </section>

    <section class="settings-section">
      <div class="card-header">
        <h3 class="card-title">Омографы и ударения</h3>
        <p class="card-desc">
          Локальная модель RUAccent расставляет ударения и выбирает вариант омографа по контексту. Работает с провайдерами Silero и Piper.
        </p>
      </div>

      <div class="setting-row accentor-select-row">
        <label class="setting-label" for="accentor-select">Модель RUAccent:</label>
        <select
          id="accentor-select"
          class="accentor-select"
          :value="noPacks && !accentorInMemory ? '' : selectedPackId"
          :disabled="noPacks || accentorRuntimeStatus === 'loading'"
          @change="onPackSelect"
        >
          <option v-if="noPacks && !accentorInMemory" value="" disabled>Модели не найдены</option>
          <template v-else-if="!noPacks">
            <option value="" disabled>Выберите модель</option>
            <option v-for="pack in accentorPacks" :key="pack.id" :value="pack.id">
              {{ formatAccentorPackLabel(pack) }}
            </option>
          </template>
          <option v-if="accentorInMemory" :value="selectedPackId" disabled>
            {{ packLabelFor(selectedPackId) }} (в памяти)
          </option>
        </select>
        <button
          type="button"
          class="accentor-load-btn"
          :disabled="
            !selectedPackId ||
            accentorSelectedMissing ||
            accentorRuntimeStatus === 'loading' ||
            accentorRuntimeStatus === 'ready'
          "
          :title="accentorLoadTitle"
          :aria-label="accentorLoadTitle"
          @click="loadSelectedModel"
        >
          <Loader2 v-if="accentorRuntimeStatus === 'loading'" :size="16" class="accentor-spin" />
          <Check v-else-if="accentorRuntimeStatus === 'ready'" :size="16" />
          <RefreshCw v-else-if="accentorRuntimeStatus === 'failed'" :size="16" />
          <Play v-else :size="16" />
        </button>
        <button
          type="button"
          class="accentor-rescan-btn"
          :disabled="accentorRescanPending"
          @click="rescanAccentorPacks"
          title="Обновить список моделей"
          aria-label="Обновить список моделей"
        >
          <ListRestart :size="16" :class="{ 'accentor-spin': accentorRescanPending }" />
        </button>
      </div>
      <span
        v-if="selectedPackId"
        class="setting-hint accentor-status"
        role="status"
        aria-live="polite"
      >
        Статус: {{ accentorStatusText }}
      </span>
      <p v-if="noPacks" class="setting-hint accentor-empty" role="status" aria-live="polite">
        Модели RUAccent не найдены. Поместите файлы моделей в:
        <code>%APPDATA%\ttsbard\models\ruaccent</code>
      </p>
      <div class="setting-row accentor-load-on-start-row">
        <label class="setting-label checkbox-label">
          <input
            :checked="accentorLoadOnStart"
            :disabled="noPacks || !selectedPackId || accentorRuntimeStatus === 'loading'"
            type="checkbox"
            class="checkbox-input"
            @change="toggleLoadOnStart"
          />
          <span>Загружать при запуске</span>
        </label>
        <span class="setting-hint">
          (может немного увеличить время запуска приложения)
        </span>
      </div>
      <div class="setting-row accentor-enable-row">
        <label class="setting-label checkbox-label">
          <input
            :checked="accentorEnabled"
            :disabled="!accentorReady"
            type="checkbox"
            class="checkbox-input"
            @change="toggleAccentor"
          />
          <span>Автоматически расставлять ударения</span>
        </label>
        <span class="setting-hint">
          Применять к каждому сообщению перед синтезом; также разрешает омографы.
        </span>
      </div>
    </section>
  </div>
</template>

<style scoped>
.settings-editor {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

.settings-section {
  padding: 12px 16px;
  background: var(--color-bg-field);
  border: 1px solid var(--color-border);
  border-radius: 12px;
  backdrop-filter: blur(8px);
}

.card-header {
  margin-bottom: 0.75rem;
}

.card-title {
  margin: 0 0 0.25rem;
  font-size: 1rem;
  font-weight: 700;
  color: var(--color-text-primary);
}

.card-desc {
  margin: 0;
  font-size: 0.8rem;
  color: var(--color-text-muted);
  line-height: 1.4;
}

.settings-editor .setting-row {
  display: block;
  margin-bottom: 0.5rem;
}

.setting-row:last-child {
  margin-bottom: 0;
}

.setting-label {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  cursor: pointer;
  user-select: none;
  font-size: 0.95rem;
  font-weight: 600;
  color: var(--color-text-primary);
}

.radio-label {
  font-weight: 500;
  padding: 0.25rem 0;
}

.checkbox-input {
  width: 18px;
  height: 18px;
  cursor: pointer;
  accent-color: var(--color-accent);
}

.radio-input {
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

.setting-hint code {
  background: var(--btn-neutral-bg);
  padding: 0.15rem 0.35rem;
  border-radius: 4px;
  font-family: var(--font-mono);
  font-size: 0.85em;
}

.settings-editor .typing-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.5rem;
}

.typing-row label {
  min-width: 110px;
  font-weight: 500;
  color: var(--color-text-secondary);
  font-size: 14px;
}

.typing-row .number-input {
  width: 88px;
  padding: 0.5rem;
  border: 1px solid var(--color-border-strong);
  border-radius: 10px;
  font-size: 14px;
  background: var(--color-bg-field);
  color: var(--color-text-primary);
}

.typing-row .number-input:focus {
  outline: none;
  border-color: var(--color-accent);
  box-shadow: 0 0 0 3px var(--color-accent-glow);
}

.typing-hint {
  margin-left: 0 !important;
  width: 100%;
}

.accentor-empty {
  margin-left: 0 !important;
}

.accentor-status {
  margin-left: 0 !important;
}

.settings-editor .accentor-select-row {
  display: flex;
  align-items: center;
  gap: 0.75rem;
}

.accentor-select-row label {
  min-width: 110px;
  font-weight: 500;
  color: var(--color-text-secondary);
  font-size: 14px;
}

.accentor-select {
  flex: 1 1 auto;
  min-width: 0;
  width: auto;
  height: 36px;
  box-sizing: border-box;
  padding: 0 0.6rem;
  background: var(--color-bg-field-hover);
  border: 1px solid var(--color-border-strong);
  border-radius: 6px;
  font-size: 14px;
  color: var(--color-text-primary);
  cursor: pointer;
  transition: all 0.15s ease;
}

.accentor-select:hover {
  background: var(--btn-neutral-bg);
  border-color: var(--color-border-strong);
}

.accentor-select:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.accentor-enable-row {
  margin-top: 0.75rem;
}

.accentor-load-on-start-row {
  margin-top: 0.25rem;
}

.accentor-load-btn,
.accentor-rescan-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 38px;
  height: 36px;
  flex-shrink: 0;
  box-sizing: border-box;
  padding: 0;
  background: var(--color-bg-field-hover);
  color: var(--color-text-primary);
  border: 1px solid var(--color-border-strong);
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.2s ease;
}

.accentor-load-btn:hover:not(:disabled),
.accentor-rescan-btn:hover:not(:disabled) {
  background: var(--btn-neutral-hover);
  border-color: var(--color-border-strong);
}

.accentor-load-btn:disabled,
.accentor-rescan-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.accentor-spin {
  animation: accentor-spin 1s linear infinite;
}

@keyframes accentor-spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

.accentor-select:focus {
  outline: none;
  border-color: var(--color-accent);
  box-shadow: 0 0 0 2px var(--focus-glow);
}

</style>
