<script setup lang="ts">
import { ref, computed } from 'vue';
import { Eye, EyeOff } from 'lucide-vue-next';
import { t } from '../../i18n';

interface Props {
  modelValue: string;
  type?: 'text' | 'password';
  placeholder?: string;
  disabled?: boolean;
  readonly?: boolean;
  label?: string;
  class?: string;
  /** Opt into the shared ui-input / ui-icon-button presentation. */
  ui?: boolean;
}

interface Emits {
  (e: 'update:modelValue', value: string): void;
}

const props = withDefaults(defineProps<Props>(), {
  type: 'password',
  placeholder: '',
  disabled: false,
  readonly: false,
  label: '',
  class: '',
  ui: false,
});

const emit = defineEmits<Emits>();

const showValue = ref(false);

const inputType = computed(() => {
  if (props.type === 'password') {
    return showValue.value ? 'text' : 'password';
  }
  return props.type;
});

const hasToggle = computed(() => props.type === 'password');

function updateValue(event: Event) {
  const target = event.target as HTMLInputElement;
  emit('update:modelValue', target.value);
}
</script>

<template>
  <div class="input-with-toggle" :class="props.class">
    <input
      :type="inputType"
      :value="modelValue"
      @input="updateValue"
      :placeholder="placeholder"
      :disabled="disabled"
      :readonly="readonly"
      :aria-label="label || undefined"
      class="input-with-toggle-input"
      :class="{ 'ui-input': ui }"
    />
    <button
      v-if="hasToggle"
      type="button"
      class="toggle-icon-button token-visibility-button"
      :class="{ 'ui-icon-button': ui, 'ui-icon-button--inset': ui }"
      @click="showValue = !showValue"
      :title="showValue ? t('common.hide') : t('common.show')"
      :aria-label="showValue ? t('common.hide') : t('common.show')"
      :aria-pressed="showValue ? 'true' : 'false'"
      :disabled="disabled"
    >
      <Eye v-if="!showValue" :size="18" />
      <EyeOff v-else :size="18" />
    </button>
    <slot v-else name="suffix" />
  </div>
</template>

<style scoped>
/* WebView2 supplies its own reveal control; this component already owns one. */
.input-with-toggle-input::-ms-reveal {
  display: none;
}

.input-with-toggle {
  position: relative;
  display: flex;
  align-items: center;
}

/* Legacy presentation (no opt-in): own typography and geometry, unchanged. */
.input-with-toggle-input:not(.ui-input) {
  flex: 1;
  width: 100%;
  padding: 10px;
  padding-right: 40px;
  background: var(--color-bg-field);
  border: 1px solid var(--color-border-strong);
  border-radius: 10px;
  color: var(--color-text-primary);
  font-size: 14px;
  box-sizing: border-box;
}

.input-with-toggle-input:not(.ui-input):hover {
  background: var(--color-bg-field-hover);
  border-color: var(--color-border-strong);
}

.input-with-toggle-input:not(.ui-input):focus {
  outline: none;
  border-color: var(--color-accent);
  box-shadow: 0 0 0 3px var(--color-accent-glow);
}

.input-with-toggle-input:not(.ui-input)::placeholder {
  color: var(--color-text-muted);
  font-size: 13px;
}

.input-with-toggle-input:not(.ui-input):disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

/* Opt-in presentation: the shared ui-input role supplies padding, border,
   radius, typography and states. Only the layout and the space under the
   32 px inset toggle stay local; padding-right keeps text clear of it. */
.input-with-toggle-input.ui-input {
  flex: 1;
  width: 100%;
  padding-right: 40px;
}

/* Toggle positioning is shared by both presentations; the vertical center
   follows the input height as it grows. */
.toggle-icon-button {
  position: absolute;
  right: 8px;
  top: 50%;
  transform: translateY(-50%);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: color 0.2s;
  background: transparent !important;
}

/* Legacy toggle sizing applies only without the opt-in. */
.toggle-icon-button:not(.ui-icon-button) {
  padding: 6px;
  border: none;
}
</style>
