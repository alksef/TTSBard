<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, type Component } from 'vue'
import { Volume2, Globe, Twitch, Star, ChevronDown } from 'lucide-vue-next'
import { ROUTE_ORDER, ROUTE_META } from './routeDecode'
import type { EditorRoute } from './routeDecode'

const props = defineProps<{
  route: EditorRoute
  defaultRoute: EditorRoute
  twitchConnected: boolean
  compact: boolean
}>()

const emit = defineEmits<{
  select: [route: EditorRoute]
  'set-default': [route: EditorRoute]
}>()

const open = ref(false)
const activeIndex = ref(0)

const destinationIcons: Record<'voice' | 'webview' | 'twitch', Component> = {
  voice: Volume2,
  webview: Globe,
  twitch: Twitch,
}

const currentMeta = computed(() => ROUTE_META[props.route])

const buttonAriaLabel = computed(() => {
  const m = currentMeta.value
  return `${m.label} — ${m.description} (префикс: ${m.shortcut})`
})

const options = computed(() => ROUTE_ORDER.map((id, index) => {
  const meta = ROUTE_META[id]
  const isDefault = id === props.defaultRoute
  const disabled = id === 'twitch_only' && !props.twitchConnected
  return {
    id,
    meta,
    index,
    isDefault,
    isCurrent: id === props.route,
    disabled,
    title: disabled ? 'Twitch не подключён' : isDefault ? 'по умолчанию' : undefined,
  }
}))

function openDropdown() {
  activeIndex.value = Math.max(0, ROUTE_ORDER.indexOf(props.route))
  open.value = true
}

function closeDropdown() {
  open.value = false
}

function toggleDropdown() {
  if (open.value) closeDropdown()
  else openDropdown()
}

function selectOption(id: EditorRoute) {
  emit('select', id)
  closeDropdown()
}

function moveActive(dir: 1 | -1) {
  const opts = options.value
  const count = opts.length
  let next = activeIndex.value
  for (let i = 0; i < count; i++) {
    next = (next + dir + count) % count
    if (!opts[next].disabled) break
  }
  activeIndex.value = next
}

function onKeydown(event: KeyboardEvent) {
  if (!open.value) {
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault()
      openDropdown()
    }
    return
  }
  if (event.key === 'Escape') {
    event.preventDefault()
    closeDropdown()
  } else if (event.key === 'ArrowDown') {
    event.preventDefault()
    moveActive(1)
  } else if (event.key === 'ArrowUp') {
    event.preventDefault()
    moveActive(-1)
  } else if (event.key === 'Enter') {
    event.preventDefault()
    const opt = options.value[activeIndex.value]
    if (opt && !opt.disabled) selectOption(opt.id)
  } else if (event.key === 'd' || event.key === 'D') {
    event.preventDefault()
    const opt = options.value[activeIndex.value]
    if (opt && !opt.disabled) emit('set-default', opt.id)
  }
}

function onOptionClick(id: EditorRoute, disabled: boolean) {
  if (disabled) return
  selectOption(id)
}

function onStarClick(id: EditorRoute, disabled: boolean) {
  if (disabled) return
  emit('set-default', id)
}

function onDocumentClick(event: MouseEvent) {
  if (!open.value) return
  const target = event.target as HTMLElement | null
  if (target && !target.closest('.route-selector')) {
    closeDropdown()
  }
}

onMounted(() => document.addEventListener('click', onDocumentClick))
onUnmounted(() => document.removeEventListener('click', onDocumentClick))
</script>

<template>
  <div class="route-selector" :class="{ compact }">
    <button
      type="button"
      class="route-selector-btn"
      :class="{ 'is-open': open }"
      aria-haspopup="listbox"
      :aria-expanded="open"
      :aria-activedescendant="open ? `route-option-${activeIndex}` : undefined"
      :title="buttonAriaLabel"
      :aria-label="buttonAriaLabel"
      @click.stop="toggleDropdown"
      @keydown="onKeydown"
    >
      <component
        v-for="dest in currentMeta.destinations"
        :key="dest"
        :is="destinationIcons[dest]"
        :size="14"
        class="dest-icon"
      />
      <ChevronDown :size="14" class="chevron" :class="{ 'chevron-open': open }" />
    </button>

    <ul v-if="open" class="route-dropdown" role="listbox" aria-label="Маршрут фразы">
      <li
        v-for="opt in options"
        :id="`route-option-${opt.index}`"
        :key="opt.id"
        class="route-option"
        :class="{
          'is-current': opt.isCurrent,
          'is-default': opt.isDefault,
          'is-disabled': opt.disabled,
          'keyboard-active': opt.index === activeIndex,
        }"
        role="option"
        :aria-selected="opt.isCurrent"
        :aria-disabled="opt.disabled"
        :title="opt.title"
        @click="onOptionClick(opt.id, opt.disabled)"
        @mousemove="activeIndex = opt.index"
      >
        <button
          type="button"
          class="option-star"
          :class="{ 'is-default': opt.isDefault }"
          :disabled="opt.disabled"
          :title="opt.isDefault ? 'по умолчанию' : 'сделать по умолчанию'"
          :aria-label="opt.isDefault ? 'Маршрут по умолчанию' : 'Сделать маршрутом по умолчанию'"
          tabindex="-1"
          @click.stop="onStarClick(opt.id, opt.disabled)"
        >
          <Star :size="12" :fill="opt.isDefault ? 'currentColor' : 'none'" />
        </button>
        <span class="option-label">{{ opt.meta.label }}</span>
        <span class="option-shortcut">{{ opt.meta.shortcut }}</span>
        <span class="option-desc">{{ opt.meta.description }}</span>
      </li>
    </ul>
  </div>
</template>

<style scoped>
.route-selector {
  position: relative;
  display: inline-flex;
  flex-shrink: 0;
}

.route-selector-btn {
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

.route-selector-btn:hover,
.route-selector-btn.is-open {
  background: var(--color-accent);
  color: var(--color-text-on-accent, #ffffff);
  border-color: var(--color-accent);
}

.route-selector-btn:focus-visible {
  outline: 2px solid var(--color-accent);
  outline-offset: 2px;
}

.route-selector.compact .route-selector-btn {
  padding: 0.3rem 0.55rem;
  gap: 0.2rem;
}

.dest-icon {
  flex-shrink: 0;
}

.chevron {
  flex-shrink: 0;
  opacity: 0.8;
  transition: transform 0.15s ease;
}

.chevron-open {
  transform: rotate(180deg);
}

.route-dropdown {
  position: absolute;
  top: calc(100% + 4px);
  left: 0;
  z-index: 1000;
  min-width: 100%;
  margin: 0;
  padding: 0.25rem;
  list-style: none;
  background: var(--color-bg-elevated);
  border: 1px solid var(--color-border-strong);
  border-radius: 8px;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.3);
}

.route-option {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.4rem 0.5rem;
  border-radius: 6px;
  cursor: pointer;
  white-space: nowrap;
}

.route-option:hover,
.route-option.keyboard-active {
  background: var(--color-accent);
  color: var(--color-text-on-accent, #ffffff);
}

.route-option.is-disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.route-option.is-current .option-label {
  font-weight: 600;
}

.option-label {
  font-size: 0.8rem;
}

.option-shortcut {
  font-size: 0.7rem;
  font-family: var(--font-mono);
  color: var(--color-text-secondary);
  opacity: 0.9;
}

.option-desc {
  font-size: 0.7rem;
  color: var(--color-text-secondary);
  opacity: 0.8;
  margin-left: auto;
}

.route-option:hover .option-shortcut,
.route-option:hover .option-desc,
.route-option.keyboard-active .option-shortcut,
.route-option.keyboard-active .option-desc {
  color: currentColor;
}

.option-star {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  padding: 0;
  background: transparent;
  border: none;
  color: var(--color-text-secondary);
  cursor: pointer;
  transition: color 0.15s ease;
}

.option-star:disabled {
  cursor: not-allowed;
}

.option-star.is-default {
  color: var(--color-accent);
}

.route-option:hover .option-star,
.route-option.keyboard-active .option-star {
  color: currentColor;
}
</style>
