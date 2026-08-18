<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted as vueOnUnmounted, inject, nextTick, type Ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { save } from '@tauri-apps/plugin-dialog'
import { useEditorSettings, useAppSettings, useTtsSettings, useAiSettings, useHotkeysSettings } from '../composables/useAppSettings'
import { SETTINGS_CHANGED_EVENT, type QuickEditorMode } from '../types/settings'
import { useErrorHandler } from '../composables/useErrorHandler'
import { debugLog, debugError } from '../utils/debug'
import { createAsyncCleanupScope } from '../utils/asyncCleanup'
import { compactModeState, initCompactDims } from '../composables/compactModeState'
import TtsEditor from './editor/TtsEditor.vue'
import PhraseHistoryList from './PhraseHistoryList.vue'
import EditorMenu from './editor/EditorMenu.vue'
import { useEditorTabs } from '../composables/useEditorTabs'
import EditorTabs from './editor/EditorTabs.vue'
import StatusMessage from './shared/StatusMessage.vue'
import { useTypingBurst, type TypingConsumer } from '../composables/useTypingBurst'
import { acceptClear, appliesQuickEditorPolicy, applyAiResponse, type SubmitIntent } from './inputAcceptance'
import { submitSpeech } from '../ipc/speech'
import { deliverTwitchMessage } from '../ipc/twitchDelivery'
import { matchesEditorHotkey } from './editor/keymapArbitration'
import { Volume2, Clock, Keyboard, Twitch, ArrowDownToLine, Undo2 } from 'lucide-vue-next'
import { enterOutcomeLabel, submitActionState } from './editor/submitAffordance'
import RouteSelector from './editor/RouteSelector.vue'
import { decodeRoutePrefix } from './editor/routeDecode'
import type { EditorRoute } from './editor/routeDecode'
import { effectiveRoute, applyRouteToText, routeSubmit } from './editor/routeResolution'
import { useTwitchRuntimeStatus } from '../composables/useTwitchRuntimeStatus'

const { showError } = useErrorHandler()
const { tabs, activeId, active, create: createTab, close: closeTab, select: selectTab, next: nextTab, previous: previousTab, rename: renameTab, init: initTabs, flushSave: flushTabsSave } = useEditorTabs()

const text = computed<string>({
  get: () => active.value.text,
  set: (v) => { active.value.text = v },
})

const editorRef = ref<InstanceType<typeof TtsEditor> | null>(null)

async function onCreate() {
  createTab()
  await nextTick()
  editorRef.value?.focus()
}

async function onSelect(id: string) {
  selectTab(id)
  await nextTick()
  editorRef.value?.focus()
}

const isCorrecting = ref(false)
const isCompleting = ref(false)
const isCheckingGrammar = ref(false)
let correctionIntent = 0
let completionIntent = 0
let grammarIntent = 0
const isSpeakingInFlight = ref(false)
const showHistory = ref(false)
const saveStatusMessage = ref('')
const replacements = ref<Map<string, string>>(new Map())
const usernames = ref<Map<string, string>>(new Map())
const isMinimalMode = inject<Ref<boolean>>('isMinimalMode', ref(false))

const editorSettings = useEditorSettings()
const aiSettings = useAiSettings()
const ttsSettings = useTtsSettings()
const hotkeySettings = useHotkeysSettings()

const appSettingsContext = useAppSettings()

const quickEditorMode = computed<QuickEditorMode>(() => editorSettings.value?.quick ?? 'disabled')

const lastSubmitOutcome = ref<'none' | 'accepted' | 'error'>('none')
let submitOutcomeTimer: ReturnType<typeof setTimeout> | null = null

const submitState = computed(() => submitActionState(isSpeakingInFlight.value, lastSubmitOutcome.value))

function clearSubmitOutcomeTimer() {
  if (submitOutcomeTimer) {
    clearTimeout(submitOutcomeTimer)
    submitOutcomeTimer = null
  }
}

function scheduleOutcomeReset() {
  clearSubmitOutcomeTimer()
  submitOutcomeTimer = setTimeout(() => {
    lastSubmitOutcome.value = 'none'
    submitOutcomeTimer = null
  }, 1500)
}

function formatHotkeyDisplay(hotkey: { modifiers: string[]; key: string } | undefined): string {
  if (!hotkey || !hotkey.key) return ''
  const modMap: Record<string, string> = { ctrl: 'Ctrl', shift: 'Shift', alt: 'Alt', super: 'Win' }
  const mods = hotkey.modifiers.map(m => modMap[m] ?? m)
  return mods.length > 0 ? `${mods.join('+')}+${hotkey.key}` : hotkey.key
}

const submitContinueBinding = computed(() => formatHotkeyDisplay(hotkeySettings.value?.editor?.submit_continue))

const { isConnected: twitchConnected } = useTwitchRuntimeStatus()

const defaultRouteFromSettings = computed<EditorRoute>(() => editorSettings.value?.default_route ?? 'everywhere')

const activeDecoded = computed(() => decodeRoutePrefix(text.value))

const currentRoute = computed<EditorRoute>(() =>
  effectiveRoute(activeDecoded.value, active.value.route, defaultRouteFromSettings.value),
)

const isTwitchOnly = computed(() => currentRoute.value === 'twitch_only')

function handleRouteSelect(route: EditorRoute) {
  text.value = applyRouteToText(text.value, activeDecoded.value, route)
  const tab = tabs.value.find(t => t.id === activeId.value)
  if (tab) tab.route = route
}

async function handleSaveDefault(route: EditorRoute) {
  try {
    await invoke('set_editor_default_route', { route })
  } catch (e) {
    debugError('[InputPanel] Failed to save default route:', e)
  }
}

const speakLabel = computed(() => {
  switch (submitState.value) {
    case 'submitting':
      return 'Отправляется…'
    case 'accepted':
      return 'Принято'
    default:
      return isTwitchOnly.value ? 'Отправить' : 'Озвучить'
  }
})

const speakTitle = computed(() => {
  if (submitState.value === 'submitting') return 'Отправляется… (Enter)'
  // Twitch-only: the button label stays short; the hover tooltip carries the
  // full semantics of the route.
  if (isTwitchOnly.value) return 'Отправить текст в Twitch, без озвучки (Enter)'
  return 'Озвучить (Enter)'
})

// Quick-editor mode indicator next to the speak button: reflects what happens
// to the window after Enter. Hidden entirely for the disabled mode.
const quickModeIndicator = computed(() => {
  switch (quickEditorMode.value) {
    case 'collapse':
      return {
        icon: ArrowDownToLine,
        title: 'Быстрый редактор: после отправки окно скроется (Enter)',
      }
    case 'return_focus':
      return {
        icon: Undo2,
        title: 'Быстрый редактор: после отправки фокус вернётся в предыдущее окно (Enter)',
      }
    default:
      return null
  }
})

const speakAriaLabel = computed(() => {
  const outcome = enterOutcomeLabel(quickEditorMode.value)
  const binding = submitContinueBinding.value
  const second = binding ? `, ${binding} — отправить и продолжить` : ''
  const action = isTwitchOnly.value ? 'Отправить текст в Twitch' : 'Озвучить текст'
  return `${action}. Enter — ${outcome}${second}`
})

const aiEditorEnabled = computed(() => editorSettings.value?.ai ?? false)

const isProviderConfigured = computed(() => {
  const provider = aiSettings.value?.provider
  const hasKey = provider === 'openai'
    ? !!aiSettings.value?.openai?.api_key
    : provider === 'zai'
      ? !!aiSettings.value?.zai?.api_key
      : provider === 'deepseek'
        ? !!aiSettings.value?.deepseek?.api_key
        : provider === 'custom'
          ? !!aiSettings.value?.custom?.api_key && !!aiSettings.value?.custom?.url
          : false

  debugLog('[InputPanel] AI provider check:', {
    provider,
    hasOpenaiKey: !!aiSettings.value?.openai?.api_key,
    hasZaiKey: !!aiSettings.value?.zai?.api_key,
    hasDeepSeekKey: !!aiSettings.value?.deepseek?.api_key,
    hasCustomKey: !!aiSettings.value?.custom?.api_key && !!aiSettings.value?.custom?.url,
    isProviderConfigured: hasKey
  })

  return hasKey
})

const isAiButtonEnabled = computed(() => {
  return isProviderConfigured.value
})

const listenerScope = createAsyncCleanupScope()
let previousCompactHeight = 0
let compactSaveTimer: ReturnType<typeof setTimeout> | null = null

const typingBurst = useTypingBurst(
  () => editorSettings.value?.typing_idle_timeout_ms ?? 800,
  [
    {
      setTyping(active: boolean) {
        return invoke('set_vtube_studio_typing', { typing: active }).then(() => {}).catch((e) => {
          debugError('[InputPanel] VTS typing failed:', e)
        })
      },
    } satisfies TypingConsumer,
    {
      setTyping(active: boolean) {
        return invoke('set_webview_typing', { typing: active }).then(() => {}).catch((e) => {
          debugError('[InputPanel] WebView typing failed:', e)
        })
      },
    } satisfies TypingConsumer,
  ],
)

const typingEnabledOverride = ref<boolean | null>(null)

const typingEnabled = computed(() => typingEnabledOverride.value ?? editorSettings.value?.typing_enabled ?? true)

const typingToggleTitle = computed(() =>
  `Передавать набор текста (WebView, VTube Studio) — ${typingEnabled.value ? 'включено' : 'выключено'}`,
)

watch(() => editorSettings.value?.typing_enabled, (val) => {
  if (val !== undefined) {
    typingEnabledOverride.value = val
  }
})

function onUserEdit() {
  if (typingEnabled.value) {
    typingBurst.edit()
  }
}

function toggleTypingEnabled() {
  const prev = typingEnabled.value
  const next = !prev
  typingEnabledOverride.value = next
  if (!next) {
    typingBurst.stop()
  }
  invoke('set_editor_typing_enabled', { enabled: next }).catch((e) => {
    typingEnabledOverride.value = prev
    console.error('[InputPanel] Failed to save typing enabled:', e)
  })
}

async function reloadPreprocessorData() {
  try {
    debugLog('[InputPanel] Reloading preprocessor data...')
    const data = await invoke<{
      replacements: Record<string, string>
      usernames: Record<string, string>
    }>('load_preprocessor_data')

    replacements.value = new Map(Object.entries(data.replacements))
    usernames.value = new Map(Object.entries(data.usernames))
    debugLog('[InputPanel] Reloaded replacements:', replacements.value.size, 'entries')
    debugLog('[InputPanel] Reloaded usernames:', usernames.value.size, 'entries')
  } catch (e) {
    debugError('[InputPanel] Failed to reload preprocessor data:', e)
  }
}

function onPreprocessorChanged() {
  debugLog('[InputPanel] Preprocessor data changed event received')
  reloadPreprocessorData()
}

onMounted(async () => {
  await initTabs()

  initCompactDims(
    appSettingsContext.settings.value?.windows?.main?.compact_width ?? 450,
    appSettingsContext.settings.value?.windows?.main?.compact_height ?? 400,
  )

  await listenerScope.track(
    listen(SETTINGS_CHANGED_EVENT, async () => {
      debugLog('[InputPanel] Settings changed event received')
    }),
  )

  window.addEventListener('preprocessor-data-changed', onPreprocessorChanged)

  await reloadPreprocessorData()

  const currentWindow = getCurrentWindow()
  const closeHandler = async () => {
    await flushTabsSave()
  }
  await listenerScope.track(currentWindow.onCloseRequested(closeHandler))

  const resizeHandler = currentWindow.onResized(async () => {
    if (!isMinimalMode.value) return
    if (compactModeState.appDrivenResize > 0) return
    if (showHistory.value) return

    if (compactSaveTimer) clearTimeout(compactSaveTimer)
    compactSaveTimer = setTimeout(async () => {
      if (!isMinimalMode.value) return
      if (compactModeState.appDrivenResize > 0) return
      try {
        const size = await currentWindow.outerSize()
        const w = Math.max(300, Math.min(500, size.width))
        const h = Math.max(300, Math.min(500, size.height))
        await invoke('set_main_compact_dims', { width: w, height: h })
        compactModeState.width = w
        compactModeState.height = h
      } catch {
        // silently fail
      }
    }, 1000)
  })
  await listenerScope.track(resizeHandler)

  compactModeState.flushPendingCompactSave = async () => {
    if (!isMinimalMode.value) return
    if (compactModeState.appDrivenResize > 0) return
    if (showHistory.value) return
    if (compactSaveTimer) {
      clearTimeout(compactSaveTimer)
      compactSaveTimer = null
    }
    try {
      const size = await currentWindow.outerSize()
      const w = Math.max(300, Math.min(500, size.width))
      const h = Math.max(300, Math.min(500, size.height))
      await invoke('set_main_compact_dims', { width: w, height: h })
      compactModeState.width = w
      compactModeState.height = h
    } catch {
      // silently fail
    }
  }
})

vueOnUnmounted(async () => {
  await flushTabsSave()
  listenerScope.dispose()
  if (compactSaveTimer) clearTimeout(compactSaveTimer)
  clearSubmitOutcomeTimer()
  compactModeState.flushPendingCompactSave = null
  window.removeEventListener('preprocessor-data-changed', onPreprocessorChanged)
  typingBurst.dispose()
})

async function hideMainWindow() {
  try {
    await invoke('hide_main_window')
  } catch (e) {
    debugError('[InputPanel] Failed to hide window:', e)
  }
}

async function recordHistory(textToRecord: string) {
  try {
    await invoke('record_history', { text: textToRecord })
  } catch {
    // silently fail
  }
}

async function correctText() {
  if (!text.value.trim()) return
  const senderTabId = activeId.value
  const sourceText = text.value
  const token = ++correctionIntent
  isCorrecting.value = true
  try {
    const corrected = await invoke<string>('correct_text', { text: sourceText })
    tabs.value = applyAiResponse(tabs.value, senderTabId, sourceText, activeId.value, corrected)
  } catch (e) {
    debugError('[InputPanel] Correction failed:', e)
  } finally {
    if (token === correctionIntent) isCorrecting.value = false
  }
}

async function completeText() {
  if (!text.value.trim()) return
  const senderTabId = activeId.value
  const sourceText = text.value
  const token = ++completionIntent
  isCompleting.value = true
  try {
    const addition = await invoke<string>('get_ai_completion', { context: sourceText })
    if (!addition) return
    const composed = `${sourceText} ${addition}`.trim()
    tabs.value = applyAiResponse(tabs.value, senderTabId, sourceText, activeId.value, composed)
  } catch (e) {
    debugError('[InputPanel] AI completion failed:', e)
    showError('Не удалось дописать текст')
  } finally {
    if (token === completionIntent) isCompleting.value = false
  }
}

async function checkGrammar() {
  if (!text.value.trim()) return
  const senderTabId = activeId.value
  const sourceText = text.value
  const token = ++grammarIntent
  isCheckingGrammar.value = true
  try {
    const corrected = await invoke<string>('ai_check_grammar', { text: sourceText })
    tabs.value = applyAiResponse(tabs.value, senderTabId, sourceText, activeId.value, corrected)
    debugLog('[InputPanel] Grammar check done')
  } catch (e) {
    debugError('[InputPanel] Grammar check failed:', e)
    showError('Не удалось проверить грамматику')
  } finally {
    if (token === grammarIntent) isCheckingGrammar.value = false
  }
}

function getProviderExtension(): string {
  const provider = ttsSettings.value?.provider
  if (provider === 'fish') {
    return ttsSettings.value?.fish?.format || 'mp3'
  }
  if (provider === 'local') {
    return 'wav'
  }
  return 'mp3'
}

async function saveAudio() {
  const currentText = text.value
  if (!currentText.trim()) return

  const ext = getProviderExtension()
  try {
    const filePath = await save({
      defaultPath: `tts_export.${ext}`,
      filters: [{ name: 'Audio', extensions: [ext] }],
    })
    if (!filePath) return

    await invoke('speak_text_raw_export', { text: currentText, path: filePath })
    saveStatusMessage.value = 'Аудио сохранено'
  } catch (e) {
    debugError('[InputPanel] Save audio failed:', e)
    showError(e as string)
  }
}

async function handleExpandedChange(expanded: boolean) {
  if (!isMinimalMode.value) return

  try {
    const currentWindow = getCurrentWindow()
    const size = await currentWindow.outerSize()

    compactModeState.appDrivenResize++
    const cw = compactModeState.width
    if (expanded) {
      previousCompactHeight = size.height
      const newHeight = Math.min(Math.max(size.height + 180, 300), 500)
      await invoke('resize_main_window', { width: cw, height: newHeight })
    } else {
      const restoreHeight = Math.min(Math.max(previousCompactHeight || compactModeState.height, 300), 500)
      previousCompactHeight = 0
      await invoke('resize_main_window', { width: cw, height: restoreHeight })
    }
  } catch {
    // silently fail
  } finally {
    setTimeout(() => { compactModeState.appDrivenResize-- }, 800)
  }
}

watch(showHistory, handleExpandedChange)

watch(isMinimalMode, (minimal) => {
  if (minimal && showHistory.value) {
    showHistory.value = false
  }
})

watch(() => appSettingsContext.settings.value?.windows?.main, (main) => {
  if (main) {
    compactModeState.width = main.compact_width ?? 450
    compactModeState.height = main.compact_height ?? 400
  }
}, { immediate: true })

watch(() => text.value, () => {
  lastSubmitOutcome.value = 'none'
})

watch(activeId, () => {
  lastSubmitOutcome.value = 'none'
})

async function handleSubmit(intent: SubmitIntent) {
  const currentText = text.value
  const senderTabId = activeId.value
  const mode = editorSettings.value?.quick ?? 'disabled'

  if (!currentText.trim()) return
  if (isSpeakingInFlight.value) return

  clearSubmitOutcomeTimer()
  typingBurst.stop()
  isSpeakingInFlight.value = true

  const submittedDecoded = decodeRoutePrefix(currentText)
  const tabRouteAtSubmit = active.value.route
  const submitRouting = routeSubmit(submittedDecoded, tabRouteAtSubmit, defaultRouteFromSettings.value)

  try {
    if (submitRouting.twitchOnly) {
      await deliverTwitchMessage(submitRouting.outgoingText)
    } else {
      await submitSpeech(submitRouting.outgoingText)
    }
    await recordHistory(submittedDecoded.text)
    tabs.value = acceptClear(tabs.value, senderTabId, currentText)

    if (appliesQuickEditorPolicy(intent)) {
      if (mode === 'collapse') {
        await hideMainWindow()
      } else if (mode === 'return_focus') {
        try {
          await invoke('return_to_previous_window')
        } catch (e) {
          debugError('[InputPanel] Failed to return focus:', e)
        }
      }
    } else {
      await nextTick()
      focusEditor()
    }

    await nextTick()
    const senderTab = tabs.value.find(t => t.id === senderTabId)
    if (activeId.value === senderTabId && senderTab?.text === '') {
      lastSubmitOutcome.value = 'accepted'
      scheduleOutcomeReset()
      if (senderTab.route === tabRouteAtSubmit) {
        senderTab.route = undefined
      }
    }
  } catch (e) {
    debugError('[InputPanel] Failed to speak:', e)
    showError(e instanceof Error ? e.message : String(e))
    lastSubmitOutcome.value = 'error'
  } finally {
    isSpeakingInFlight.value = false
  }
}

async function handleEnter() {
  await handleSubmit('quick')
}

async function handleSubmitContinue() {
  await handleSubmit('continue')
}

async function handleEsc() {
  const mode = editorSettings.value?.quick ?? 'disabled'

  typingBurst.stop()

  if (mode === 'collapse') {
    hideMainWindow()
  } else if (mode === 'return_focus') {
    try {
      await invoke('return_to_previous_window')
    } catch (e) {
      debugError('[InputPanel] Failed to return focus:', e)
    }
  }
}

function appendPhrase(newText: string) {
  const currentText = text.value
  if (!currentText.trim()) {
    text.value = newText
    return
  }
  const sep = currentText.endsWith(' ') ? '' : ' '
  text.value = currentText + sep + newText
}

function replacePhrase(newText: string) {
  const currentText = text.value
  if (currentText.trim() && currentText !== newText) {
    if (!confirm('Заменить текущий текст на выбранную фразу?')) return
  }
  text.value = newText
}

const replacementsRecord = computed(() => {
  const obj: Record<string, string> = {}
  replacements.value.forEach((v, k) => { obj[k] = v })
  return obj
})

const usernamesRecord = computed(() => {
  const obj: Record<string, string> = {}
  usernames.value.forEach((v, k) => { obj[k] = v })
  return obj
})

// Editor resize state
const editorHeight = ref(editorSettings.value?.editor_height ?? 340)
const isResizing = ref(false)
const resizeStartY = ref(0)
const resizeStartHeight = ref(0)

watch(() => editorSettings.value?.editor_height, (newVal) => {
  if (newVal !== undefined && newVal !== editorHeight.value && !isResizing.value) {
    editorHeight.value = newVal
  }
}, { immediate: true })

function onResizePointerDown(e: PointerEvent) {
  isResizing.value = true
  resizeStartY.value = e.clientY
  resizeStartHeight.value = editorHeight.value
  ;(e.target as HTMLElement)?.setPointerCapture?.(e.pointerId)
}

function onResizePointerMove(e: PointerEvent) {
  if (!isResizing.value) return
  const dy = e.clientY - resizeStartY.value
  const newHeight = Math.max(200, Math.min(1200, resizeStartHeight.value + dy))
  editorHeight.value = newHeight
}

function onResizePointerUp(_e: PointerEvent) {
  if (!isResizing.value) return
  isResizing.value = false
  const heightToSave = editorHeight.value
  invoke('set_editor_height', { height: heightToSave }).catch(() => {})
}

const editorHeightPx = computed(() => `${editorHeight.value}px`)

function toggleHistory() {
  showHistory.value = !showHistory.value
}

function focusEditor() {
  editorRef.value?.focus()
}

async function handleEditorScopeKeydown(event: KeyboardEvent) {
  if (event.target instanceof HTMLInputElement) return
  const bindings = hotkeySettings.value?.editor
  if (!bindings) return

  const switched = matchesEditorHotkey(bindings.next_tab, event)
    ? nextTab()
    : matchesEditorHotkey(bindings.previous_tab, event)
      ? previousTab()
      : false
  if (!switched) return

  event.preventDefault()
  event.stopPropagation()
  await nextTick()
  focusEditor()
}

defineExpose({ focusEditor })
</script>

<template>
  <div class="input-panel" :class="{ 'minimal-panel': isMinimalMode }">
    <StatusMessage
      :message="saveStatusMessage"
      type="success"
      @dismiss="saveStatusMessage = ''"
    />
    <div class="input-group">
      <div class="textarea-wrapper" :class="{ 'minimal-wrapper': isMinimalMode }" @keydown.capture="handleEditorScopeKeydown">
        <EditorTabs
          :tabs="tabs"
          :active-id="activeId"
          @create="onCreate"
          @close="closeTab"
          @select="onSelect"
          @rename="renameTab"
        />
        <TtsEditor
          ref="editorRef"
          v-model="text"
          :placeholder="'Введите текст для озвучивания...'"
          :replacements="replacementsRecord"
          :usernames="usernamesRecord"
          :editor-height-px="editorHeightPx"
          @user-edit="onUserEdit"
          @enter="handleEnter"
          @submit-continue="handleSubmitContinue"
          @esc="handleEsc"
        />
        <div
          class="editor-resize-handle"
          @pointerdown="onResizePointerDown"
          @pointermove="onResizePointerMove"
          @pointerup="onResizePointerUp"
          title="Изменить высоту редактора"
        />
      </div>

      <div class="editor-action-bar" :class="{ 'compact-action-bar': isMinimalMode }">
        <EditorMenu
          :is-ai-enabled="isAiButtonEnabled"
          :has-text="!!text.trim()"
          :compact="isMinimalMode"
          @correct="correctText"
          @complete="completeText"
          @grammar="checkGrammar"
          @save-audio="saveAudio"
        />
        <button
          class="action-btn history-btn"
          :class="{ active: showHistory }"
          @click="toggleHistory"
          title="История фраз"
          :aria-label="showHistory ? 'Скрыть историю фраз' : 'Показать историю фраз'"
        >
          <Clock v-if="isMinimalMode" :size="14" />
          <template v-else>История фраз</template>
        </button>
        <button
          v-if="!isMinimalMode"
          class="action-btn ai-btn"
          :class="{ loading: isCorrecting || isCompleting || isCheckingGrammar }"
          :disabled="isCorrecting || isCompleting || isCheckingGrammar || !text.trim() || !isAiButtonEnabled"
          @click="correctText"
          title="Корректировать текст с помощью AI"
          aria-label="AI корректировка текста"
        >
          AI
        </button>
        <RouteSelector
          :route="currentRoute"
          :default-route="defaultRouteFromSettings"
          :twitch-connected="twitchConnected"
          :compact="isMinimalMode"
          @select="handleRouteSelect"
          @set-default="handleSaveDefault"
        />
        <button
          type="button"
          class="action-btn typing-toggle-btn"
          :class="{ active: typingEnabled }"
          :aria-pressed="typingEnabled"
          :aria-label="typingToggleTitle"
          :title="typingToggleTitle"
          @click="toggleTypingEnabled"
        >
          <Keyboard :size="14" />
        </button>
        <div class="action-bar-spacer" />
        <span
          v-if="quickModeIndicator"
          class="quick-mode-indicator"
          role="img"
          :title="quickModeIndicator.title"
          :aria-label="quickModeIndicator.title"
        >
          <component :is="quickModeIndicator.icon" :size="13" />
        </span>
        <button
          class="action-btn speak-btn"
          :class="{ 'icon-only': isMinimalMode }"
          :disabled="!text.trim() || submitState === 'submitting'"
          :aria-busy="submitState === 'submitting'"
          :title="speakTitle"
          :aria-label="speakAriaLabel"
          @click="handleEnter"
        >
          <Twitch v-if="isTwitchOnly" :size="14" />
          <Volume2 v-else :size="14" />
          <span v-if="!isMinimalMode">{{ speakLabel }}</span>
        </button>
      </div>

      <PhraseHistoryList
        v-model:expanded="showHistory"
        :hide-toggle="true"
        @select="appendPhrase"
        @append="appendPhrase"
        @replace="replacePhrase"
      />
      <div v-if="aiEditorEnabled" class="ai-editor-hint">
        AI
      </div>
    </div>
  </div>
</template>

<style scoped>
.input-panel {
  position: relative;
  z-index: 1;
  max-width: 1120px;
  margin: 0;
  padding: 0.2rem 0 2rem;
  transition: all 0.3s ease;
}

.input-panel.minimal-panel {
  padding: 0 !important;
  max-width: none !important;
}

.input-group {
  position: relative;
  margin-bottom: 1.6rem;
}

.textarea-wrapper {
  position: relative;
  margin-bottom: 0;
}

.textarea-wrapper.minimal-wrapper :deep(.cm-editor) {
  min-height: 200px;
}

.textarea-wrapper.minimal-wrapper :deep(.cm-scroller) {
  min-height: 200px !important;
}

.editor-resize-handle {
  height: 6px;
  cursor: ns-resize;
  background: transparent;
  transition: background 0.15s ease;
  border-radius: 0 0 3px 3px;
  margin-top: -1px;
}

.editor-resize-handle:hover {
  background: var(--color-accent);
  opacity: 0.4;
}

.editor-action-bar {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.4rem 0;
  flex-wrap: nowrap;
}

.editor-action-bar.compact-action-bar {
  padding: 0.25rem 0;
  gap: 0.25rem;
}

.action-btn {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  padding: 0.3rem 0.75rem;
  background: var(--color-bg-elevated);
  color: var(--color-text-primary);
  border: 1px solid var(--color-border-strong);
  border-radius: 8px;
  font-size: 0.8rem;
  font-family: var(--font-mono);
  cursor: pointer;
  transition: all 0.2s ease;
  white-space: nowrap;
}

.action-btn:hover:not(:disabled) {
  background: var(--color-accent);
  color: var(--color-text-on-accent, #ffffff);
  border-color: var(--color-accent);
}

.action-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.action-btn.active {
  background: var(--color-accent);
  color: var(--color-text-on-accent, #ffffff);
  border-color: var(--color-accent);
}

.ai-btn.loading {
  animation: pulse 1.5s ease-in-out infinite;
}

.speak-btn {
  font-weight: 500;
  flex-shrink: 0;
  background: var(--color-accent);
  color: var(--color-text-on-accent, #ffffff);
  border-color: var(--color-accent);
}

.speak-btn:hover:not(:disabled) {
  filter: brightness(1.1);
}

.speak-btn:focus-visible {
  outline: 2px solid var(--color-accent);
  outline-offset: 2px;
}

.speak-btn.icon-only {
  padding: 0.3rem 0.55rem;
}

.typing-toggle-btn {
  position: relative;
  flex-shrink: 0;
  padding: 0.3rem 0.55rem;
}

/* Neutral hover: the typing toggle is a state indicator, not an action —
   no accent fill on hover (overrides .action-btn:hover). */
.typing-toggle-btn:hover:not(:disabled) {
  background: var(--color-bg-elevated);
  color: var(--color-text-primary);
  border-color: var(--color-border-strong);
}

.typing-toggle-btn.active {
  background: var(--color-bg-elevated);
  color: var(--color-text-primary);
  border-color: var(--color-border-strong);
}

.typing-toggle-btn:not(.active) {
  opacity: 0.45;
}

.typing-toggle-btn:not(.active)::after {
  content: '';
  position: absolute;
  left: 50%;
  top: 50%;
  width: 70%;
  height: 1.5px;
  background: currentColor;
  border-radius: 1px;
  transform: translate(-50%, -50%) rotate(-45deg);
}

.action-bar-spacer {
  flex: 1;
  min-width: 0;
}

/* Ambient indicator of the quick-editor mode (collapse / return focus);
   hidden entirely when the mode is disabled. Not a control. */
.quick-mode-indicator {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  color: var(--color-text-secondary);
  opacity: 0.8;
  margin-right: 0.35rem;
}

@media (max-width: 960px) {
  .input-panel {
    padding-bottom: 1.5rem;
  }
}

.ai-editor-hint {
  margin-top: 0.5rem;
  font-size: 0.8rem;
  color: var(--color-accent);
  opacity: 0.8;
  text-align: center;
  font-weight: 600;
}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.6; }
}
</style>
