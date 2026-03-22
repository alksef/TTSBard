<script setup lang="ts">
import { ref, computed } from 'vue';
import { Eye, EyeOff } from 'lucide-vue-next';

interface Props {
  modelValue: string;
  type?: 'text' | 'password';
  placeholder?: string;
  disabled?: boolean;
  class?: string;
}

interface Emits {
  (e: 'update:modelValue', value: string): void;
}

const props = withDefaults(defineProps<Props>(), {
  type: 'password',
  placeholder: '',
  disabled: false,
  class: '',
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
      class="input-with-toggle-input"
    />
    <button
      v-if="hasToggle"
      type="button"
      class="toggle-icon-button"
      @click="showValue = !showValue"
      :title="showValue ? 'Скрыть' : 'Показать'"
    >
      <Eye v-if="!showValue" :size="18" />
      <EyeOff v-else :size="18" />
    </button>
    <slot v-else name="suffix" />
  </div>
</template>

<style scoped>
.input-with-toggle {
  position: relative;
  display: flex;
  align-items: center;
}

.input-with-toggle-input {
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

.input-with-toggle-input:hover {
  background: var(--color-bg-field-hover);
  border-color: var(--color-border-strong);
}

.input-with-toggle-input:focus {
  outline: none;
  border-color: var(--color-accent);
  box-shadow: 0 0 0 3px var(--color-accent-glow);
}

.input-with-toggle-input::placeholder {
  color: var(--color-text-muted);
  font-size: 13px;
}

.input-with-toggle-input:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.toggle-icon-button {
  position: absolute;
  right: 8px;
  top: 50%;
  transform: translateY(-50%);
  padding: 6px;
  border: none;
  cursor: pointer;
  color: var(--color-text-secondary);
  display: flex;
  align-items: center;
  justify-content: center;
  transition: color 0.2s;
  background: transparent !important;
}

.toggle-icon-button:hover {
  color: var(--color-accent);
}
</style>
