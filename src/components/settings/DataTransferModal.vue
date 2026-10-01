<script setup lang="ts">
import { ref, computed, watch, nextTick, onMounted, onUnmounted } from 'vue'
import { t } from '../../i18n'
import {
  useDataTransfer,
  formatTransferBytes,
  progressPercent,
} from '../../composables/useDataTransfer'

const props = defineProps<{
  targetPath: string | null
}>()

const emit = defineEmits<{
  (e: 'close'): void
  (e: 'success', result: { restartRequired: boolean; path: string; isDefault: boolean }): void
}>()

const transfer = useDataTransfer()

const dialogRef = ref<HTMLElement | null>(null)
const titleId = 'data-transfer-title'

const stage = computed(() => transfer.stage.value)
const canClose = computed(() => transfer.canClose.value)

const indeterminate = computed(() => {
  if (stage.value === 'preparing') return true
  if (stage.value === 'transferring') {
    return transfer.phase.value !== 'copying' || transfer.totalBytes.value === 0
  }
  return false
})

const percent = computed(() => progressPercent(transfer.completedBytes.value, transfer.totalBytes.value))

const sizeText = computed(() =>
  t('dataTransfer.size', {
    done: formatTransferBytes(transfer.completedBytes.value),
    total: formatTransferBytes(transfer.totalBytes.value),
  }),
)

const phaseText = computed(() => t(`dataTransfer.phase.${transfer.phase.value}`))

let previouslyFocused: HTMLElement | null = null

function appRoot(): HTMLElement | null {
  return document.getElementById('app')
}

function applyInert() {
  appRoot()?.setAttribute('inert', '')
}

function removeInert() {
  appRoot()?.removeAttribute('inert')
}

function focusableElements(): HTMLElement[] {
  const dialog = dialogRef.value
  if (!dialog) return []
  const selector = [
    'button:not([disabled])',
    'a[href]',
    'input:not([disabled])',
    'select:not([disabled])',
    'textarea:not([disabled])',
    '[tabindex]:not([tabindex="-1"])',
  ].join(',')
  return Array.from(dialog.querySelectorAll<HTMLElement>(selector)).filter(
    (el) => el.offsetParent !== null,
  )
}

function focusInitial() {
  const first = focusableElements()[0]
  if (first) first.focus()
  else dialogRef.value?.focus()
}

function trapFocus(event: KeyboardEvent) {
  const dialog = dialogRef.value
  if (!dialog) return
  const focusable = focusableElements()
  if (focusable.length === 0) {
    event.preventDefault()
    dialog.focus()
    return
  }
  const first = focusable[0]
  const last = focusable[focusable.length - 1]
  const active = document.activeElement as HTMLElement | null
  const outside = !active || !dialog.contains(active)
  if (event.shiftKey) {
    if (outside || active === first || active === dialog) {
      event.preventDefault()
      last.focus()
    }
  } else if (outside || active === last || active === dialog) {
    event.preventDefault()
    first.focus()
  }
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    event.preventDefault()
    event.stopImmediatePropagation()
    if (canClose.value) emit('close')
    return
  }
  if (event.key === 'Tab') {
    trapFocus(event)
  }
}

function onOverlayClick() {
  if (canClose.value) emit('close')
}

watch(
  () => transfer.stage.value,
  () => {
    void nextTick(focusInitial)
  },
)

watch(
  () => transfer.stage.value,
  (current) => {
    if (current === 'success') {
      const result = transfer.resultInfo.value
      if (result) emit('success', { restartRequired: result.restart_required, path: result.path, isDefault: result.is_default })
    }
  },
)

onMounted(() => {
  previouslyFocused = document.activeElement as HTMLElement | null
  applyInert()
  document.addEventListener('keydown', onKeydown, true)
  void nextTick(focusInitial)
  void transfer.open(props.targetPath)
})

onUnmounted(() => {
  document.removeEventListener('keydown', onKeydown, true)
  removeInert()
  previouslyFocused?.focus?.()
})
</script>

<template>
  <Teleport to="body">
    <div class="data-transfer-overlay" @click.self="onOverlayClick">
      <div
        ref="dialogRef"
        class="data-transfer-dialog"
        role="dialog"
        aria-modal="true"
        :aria-labelledby="titleId"
        tabindex="-1"
        @keydown.stop
      >
        <h2 :id="titleId" class="data-transfer-title">{{ t('dataTransfer.title') }}</h2>

        <!-- Preparing (initial) -->
        <div v-if="stage === 'preparing'" class="data-transfer-body">
          <div
            class="data-transfer-progress"
            role="progressbar"
            :aria-label="t('dataTransfer.progress')"
            aria-valuemin="0"
            aria-valuemax="100"
            :aria-valuetext="t('dataTransfer.phase.preparing')"
          >
            <div class="data-transfer-bar indeterminate"></div>
          </div>
          <p class="data-transfer-phase">{{ t('dataTransfer.phase.preparing') }}</p>
          <div class="data-transfer-actions">
            <button type="button" class="data-transfer-btn neutral" @click="emit('close')">
              {{ t('common.cancel') }}
            </button>
          </div>
        </div>

        <!-- Confirmation -->
        <div v-else-if="stage === 'confirm'" class="data-transfer-body">
          <p class="data-transfer-line">{{ t('dataTransfer.to_target') }}</p>
          <p class="data-transfer-path">{{ transfer.targetPath.value }}</p>
          <p class="data-transfer-line">{{ t('dataTransfer.from_source') }}</p>
          <p class="data-transfer-path muted">{{ transfer.sourcePath.value }}</p>
          <p class="data-transfer-size">{{ t('dataTransfer.total_size', { total: formatTransferBytes(transfer.totalBytes.value) }) }}</p>
          <div class="data-transfer-actions">
            <button type="button" class="data-transfer-btn neutral" @click="emit('close')">
              {{ t('common.cancel') }}
            </button>
            <button type="button" class="data-transfer-btn primary" @click="transfer.confirm()">
              {{ t('dataTransfer.transfer') }}
            </button>
          </div>
        </div>

        <!-- Transferring -->
        <div v-else-if="stage === 'transferring'" class="data-transfer-body">
          <p class="data-transfer-line">{{ t('dataTransfer.to_target') }}</p>
          <p class="data-transfer-path">{{ transfer.targetPath.value }}</p>
          <div
            class="data-transfer-progress"
            role="progressbar"
            :aria-label="t('dataTransfer.progress')"
            aria-valuemin="0"
            aria-valuemax="100"
            :aria-valuenow="indeterminate ? undefined : percent"
            :aria-valuetext="phaseText"
          >
            <div
              class="data-transfer-bar"
              :class="{ indeterminate }"
              :style="indeterminate ? undefined : { width: `${percent}%` }"
            ></div>
          </div>
          <p class="data-transfer-phase">{{ phaseText }}</p>
          <p class="data-transfer-size">{{ sizeText }}</p>
          <p v-if="!indeterminate" class="data-transfer-percent">{{ percent }}%</p>
          <p class="data-transfer-wait">{{ t('dataTransfer.wait') }}</p>
        </div>

        <!-- Error -->
        <div v-else-if="stage === 'error'" class="data-transfer-body">
          <div class="data-transfer-error" role="alert">
            {{ transfer.errorMessage.value }}
          </div>
          <div class="data-transfer-actions">
            <button type="button" class="data-transfer-btn neutral" @click="transfer.retry()">
              {{ t('dataTransfer.retry') }}
            </button>
            <button type="button" class="data-transfer-btn neutral" @click="emit('close')">
              {{ t('common.close') }}
            </button>
          </div>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.data-transfer-overlay {
  position: fixed;
  inset: 0;
  z-index: 1000;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 20px;
  background: var(--modal-overlay);
  backdrop-filter: blur(4px);
}

.data-transfer-dialog {
  background: var(--color-bg-panel-strong);
  border: 1px solid var(--color-border-strong);
  border-radius: 16px;
  max-width: 460px;
  width: 100%;
  max-height: 90vh;
  overflow-y: auto;
  box-shadow: var(--shadow-soft);
  padding: 20px 24px;
  outline: none;
}

.data-transfer-title {
  margin: 0 0 16px;
  font-size: 18px;
  font-weight: 600;
  color: var(--color-text-primary);
}

.data-transfer-body {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.data-transfer-line {
  margin: 0;
  font-size: 13px;
  font-weight: 500;
  color: var(--color-text-secondary);
}

.data-transfer-path {
  margin: 0 0 8px;
  font-family: var(--font-mono);
  font-size: 0.85rem;
  color: var(--color-text-primary);
  line-height: 1.4;
  overflow-wrap: anywhere;
  word-break: break-word;
}

.data-transfer-path.muted {
  color: var(--color-text-muted);
}

.data-transfer-size {
  margin: 8px 0 0;
  font-size: 14px;
  font-weight: 600;
  color: var(--color-text-primary);
}

.data-transfer-phase {
  margin: 0;
  font-size: 13px;
  color: var(--color-text-secondary);
}

.data-transfer-percent {
  margin: 0;
  font-size: 13px;
  font-weight: 600;
  color: var(--color-text-secondary);
}

.data-transfer-wait {
  margin: 4px 0 0;
  font-size: 13px;
  color: var(--color-text-muted);
}

.data-transfer-progress {
  height: 8px;
  border-radius: 999px;
  background: var(--color-bg-field-hover);
  overflow: hidden;
}

.data-transfer-bar {
  height: 100%;
  border-radius: 999px;
  background: var(--color-accent);
  transition: width 0.15s ease;
}

.data-transfer-bar.indeterminate {
  width: 40%;
  animation: data-transfer-indeterminate 1.2s ease-in-out infinite;
}

@keyframes data-transfer-indeterminate {
  0% {
    transform: translateX(-100%);
  }
  100% {
    transform: translateX(350%);
  }
}

.data-transfer-error {
  padding: 12px 16px;
  background: var(--danger-bg-weak);
  border: 1px solid var(--danger-border-strong);
  border-left: 4px solid var(--status-disconnected);
  border-radius: 8px;
  color: var(--danger-text-weak);
  font-size: 14px;
  line-height: 1.4;
  overflow-wrap: anywhere;
}

.data-transfer-actions {
  display: flex;
  justify-content: flex-end;
  gap: 12px;
  margin-top: 16px;
}

.data-transfer-btn {
  padding: 10px 18px;
  border-radius: 10px;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.15s ease;
}

.data-transfer-btn.neutral {
  background: var(--btn-neutral-bg);
  color: var(--color-text-primary);
  border: 1px solid var(--color-border-strong);
}

.data-transfer-btn.neutral:hover {
  background: var(--btn-neutral-hover);
  border-color: var(--color-accent);
}

.data-transfer-btn.primary {
  background: linear-gradient(135deg, var(--color-accent) 0%, var(--color-accent-strong) 100%);
  color: var(--color-text-white);
  border: none;
  font-weight: 600;
}

.data-transfer-btn.primary:hover {
  filter: brightness(1.06);
}

.data-transfer-btn:focus-visible {
  outline: none;
  border-color: var(--color-accent);
  box-shadow: 0 0 0 2px var(--focus-glow);
}
</style>
