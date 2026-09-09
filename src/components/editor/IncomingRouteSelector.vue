<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { ChevronDown, CircleAlert } from 'lucide-vue-next'
import { INCOMING_ROUTE_ORDER, INCOMING_ROUTE_META } from './incomingRoute'
import type { IncomingRoute } from './incomingRoute'
import { destinationIcons, type Destination } from './destinationIcons'
import { t } from '../../i18n'

const props = defineProps<{
  route: IncomingRoute
  compact: boolean
  twitchConnected: boolean
  webviewConnected: boolean
}>()

const emit = defineEmits<{
  select: [route: IncomingRoute]
}>()

const open = ref(false)
const activeIndex = ref(0)

const currentMeta = computed(() => INCOMING_ROUTE_META[props.route])

function isDestinationConnected(dest: Destination): boolean {
  if (dest === 'twitch') return props.twitchConnected
  if (dest === 'webview') return props.webviewConnected
  return true
}

const disconnected = computed<Destination[]>(() =>
  currentMeta.value.destinations.filter(dest => !isDestinationConnected(dest)),
)

const disconnectedTitle = computed(() =>
  disconnected.value.length > 0 ? t('editor.incoming.route.disconnected_hint') : undefined,
)

const buttonAriaLabel = computed(() => {
  const m = currentMeta.value
  const base = t('editor.incoming.route.button_aria', {
    label: m.label,
    description: m.description,
  })
  return disconnectedTitle.value ? `${base}. ${disconnectedTitle.value}` : base
})

const options = computed(() => INCOMING_ROUTE_ORDER.map((id, index) => {
  const meta = INCOMING_ROUTE_META[id]
  return {
    id,
    meta,
    index,
    isCurrent: id === props.route,
  }
}))

function openDropdown() {
  activeIndex.value = Math.max(0, INCOMING_ROUTE_ORDER.indexOf(props.route))
  open.value = true
}

function closeDropdown() {
  open.value = false
}

function toggleDropdown() {
  if (open.value) closeDropdown()
  else openDropdown()
}

function selectOption(id: IncomingRoute) {
  emit('select', id)
  closeDropdown()
}

function moveActive(dir: 1 | -1) {
  const count = options.value.length
  activeIndex.value = (activeIndex.value + dir + count) % count
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
    if (opt) selectOption(opt.id)
  }
}

function onOptionClick(id: IncomingRoute) {
  selectOption(id)
}

function onDocumentClick(event: MouseEvent) {
  if (!open.value) return
  const target = event.target as HTMLElement | null
  if (target && !target.closest('.incoming-route-selector')) {
    closeDropdown()
  }
}

onMounted(() => document.addEventListener('click', onDocumentClick))
onUnmounted(() => document.removeEventListener('click', onDocumentClick))
</script>

<template>
  <div class="incoming-route-selector" :class="{ compact }">
    <button
      type="button"
      class="incoming-route-btn"
      :class="{ 'is-open': open, 'has-disconnected': disconnected.length > 0 }"
      aria-haspopup="listbox"
      :aria-expanded="open"
      :aria-activedescendant="open ? `incoming-route-option-${activeIndex}` : undefined"
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
        :class="{ 'is-disconnected': !isDestinationConnected(dest) }"
      />
      <CircleAlert v-if="disconnected.length > 0" :size="12" class="alert-icon" />
      <ChevronDown :size="14" class="chevron" :class="{ 'chevron-open': open }" />
    </button>

    <ul
      v-if="open"
      class="incoming-route-dropdown"
      role="listbox"
      :aria-label="t('editor.incoming.route.listbox_aria')"
    >
      <li
        v-for="opt in options"
        :id="`incoming-route-option-${opt.index}`"
        :key="opt.id"
        class="incoming-route-option"
        :class="{
          'is-current': opt.isCurrent,
          'keyboard-active': opt.index === activeIndex,
        }"
        role="option"
        :aria-selected="opt.isCurrent"
        @click="onOptionClick(opt.id)"
        @mousemove="activeIndex = opt.index"
      >
        <span class="option-icons">
          <component
            v-for="dest in opt.meta.destinations"
            :key="dest"
            :is="destinationIcons[dest]"
            :size="13"
            class="dest-icon"
            :class="{ 'is-disconnected': !isDestinationConnected(dest) }"
          />
        </span>
        <span class="option-label">{{ opt.meta.label }}</span>
        <span class="option-desc">{{ opt.meta.description }}</span>
      </li>
    </ul>
  </div>
</template>

<style scoped>
.incoming-route-selector {
  position: relative;
  display: inline-flex;
  flex-shrink: 0;
}

.incoming-route-btn {
  display: inline-flex;
  align-items: center;
  gap: 0.3rem;
  padding: 0.22rem 0.55rem;
  background: var(--color-bg-elevated);
  color: var(--color-text-primary);
  border: 1px solid var(--color-border-strong);
  border-radius: 6px;
  font-size: 0.8rem;
  font-family: var(--font-mono);
  cursor: pointer;
  transition: all 0.2s ease;
  white-space: nowrap;
}

.incoming-route-btn:hover,
.incoming-route-btn.is-open {
  background: var(--color-accent);
  color: var(--color-text-on-accent, #ffffff);
  border-color: var(--color-accent);
}

.incoming-route-btn:focus-visible {
  outline: 2px solid var(--color-accent);
  outline-offset: 2px;
}

.incoming-route-selector.compact .incoming-route-btn {
  padding: 0.18rem 0.45rem;
  gap: 0.2rem;
}

.dest-icon {
  flex-shrink: 0;
}

.dest-icon.is-disconnected {
  opacity: 0.5;
}

.alert-icon {
  flex-shrink: 0;
  color: var(--color-warning, var(--color-text-secondary));
}

.chevron {
  flex-shrink: 0;
  opacity: 0.8;
  transition: transform 0.15s ease;
}

.chevron-open {
  transform: rotate(180deg);
}

.incoming-route-dropdown {
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

.incoming-route-option {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.4rem 0.5rem;
  border-radius: 6px;
  cursor: pointer;
  white-space: nowrap;
}

.incoming-route-option:hover,
.incoming-route-option.keyboard-active {
  background: var(--color-accent);
  color: var(--color-text-on-accent, #ffffff);
}

.incoming-route-option.is-current .option-label {
  font-weight: 600;
}

.option-icons {
  display: inline-flex;
  align-items: center;
  gap: 0.2rem;
  flex-shrink: 0;
}

.option-label {
  font-size: 0.8rem;
}

.option-desc {
  font-size: 0.7rem;
  color: var(--color-text-secondary);
  opacity: 0.8;
  margin-left: auto;
}

.incoming-route-option:hover .option-desc,
.incoming-route-option.keyboard-active .option-desc {
  color: currentColor;
}
</style>
