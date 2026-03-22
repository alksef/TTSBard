<script setup lang="ts">
import { computed } from 'vue';
import { Bot } from 'lucide-vue-next';
import ProviderCard from '../shared/ProviderCard.vue';
import TelegramConnectionStatus from './TelegramConnectionStatus.vue';

interface Props {
  active?: boolean;
  expanded?: boolean;
  connected?: boolean;
  telegramStatus?: {
    first_name?: string;
    last_name?: string;
    username?: string;
  } | null;
  currentProxyStatus?: {
    mode: string;
    proxy_url: string | null;
  } | null;
  errorMessage?: string | null;
  reconnecting?: boolean;
  proxyMode?: string;
  proxyModes?: Array<{ value: string; label: string }>;
}

interface Emits {
  (e: 'select'): void;
  (e: 'toggle'): void;
  (e: 'connect'): void;
  (e: 'disconnect'): void;
  (e: 'reconnect'): void;
  (e: 'proxy-mode-change', mode: string): void;
}

const props = withDefaults(defineProps<Props>(), {
  active: false,
  expanded: false,
  connected: false,
  reconnecting: false,
  proxyMode: 'none',
  proxyModes: () => [
    { value: 'none', label: 'Нет' },
    { value: 'socks5', label: 'SOCKS5' },
    { value: 'mtproxy', label: 'MTProxy' }
  ],
});

const emit = defineEmits<Emits>();

const hasError = computed(() => props.errorMessage !== null);
</script>

<template>
  <ProviderCard
    title="Silero Bot"
    :icon="Bot"
    :active="active"
    :expanded="expanded"
    :class="{ 'error-state': hasError }"
    @select="$emit('select')"
    @toggle="$emit('toggle')"
  >
    <TelegramConnectionStatus
      :connected="connected"
      :telegram-status="telegramStatus"
      :current-proxy-status="currentProxyStatus"
      :error-message="errorMessage"
      :reconnecting="reconnecting"
      :proxy-mode="proxyMode"
      :proxy-modes="proxyModes"
      @connect="$emit('connect')"
      @disconnect="$emit('disconnect')"
      @reconnect="$emit('reconnect')"
      @proxy-mode-change="$emit('proxy-mode-change', $event)"
    />
  </ProviderCard>
</template>

<style scoped>
.error-state {
  border-color: var(--card-error-border) !important;
  background: var(--card-error-bg) !important;
}
</style>
