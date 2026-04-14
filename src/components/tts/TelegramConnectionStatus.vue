<script setup lang="ts">
import { computed } from 'vue';
import { LogOut, RefreshCw } from 'lucide-vue-next';

interface TelegramStatus {
  first_name?: string;
  last_name?: string;
  username?: string;
}

interface ProxyStatus {
  mode: string;
  proxy_url: string | null;
}

interface Props {
  connected: boolean;
  statusMessage?: string | null;
  userPhone?: string | null;
  reconnecting?: boolean;
  telegramStatus?: TelegramStatus | null;
  currentProxyStatus?: ProxyStatus | null;
  errorMessage?: string | null;
  proxyMode?: string;
  proxyModes?: Array<{ value: string; label: string }>;
}

interface Emits {
  (e: 'connect'): void;
  (e: 'disconnect'): void;
  (e: 'reconnect'): void;
  (e: 'proxy-mode-change', mode: string): void;
}

const props = withDefaults(defineProps<Props>(), {
  reconnecting: false,
  proxyMode: 'none',
  proxyModes: () => [
    { value: 'none', label: 'Нет' },
    { value: 'socks5', label: 'SOCKS5' },
    { value: 'mtproxy', label: 'MTProxy' }
  ],
});

const emit = defineEmits<Emits>();

const proxyModeLabel = computed(() => {
  if (props.currentProxyStatus) {
    const mode = props.currentProxyStatus.mode;
    if (mode === 'none') return '';
    if (mode === 'socks5') return 'SOCKS5';
    if (mode === 'mtproxy') return 'MTProxy';
  }
  return '';
});

function handleProxyChange(event: Event) {
  const target = event.target as HTMLSelectElement;
  emit('proxy-mode-change', target.value);
}
</script>

<template>
  <div class="telegram-connection-status">
    <!-- Error Banner -->
    <div v-if="errorMessage" class="silero-error-banner">
      <div class="error-banner-content">
        <div class="error-icon">⚠</div>
        <div class="error-text">
          <p class="error-title">Ошибка подключения Telegram</p>
          <p class="error-message">{{ errorMessage }}</p>
        </div>
      </div>
      <button class="fix-button" @click="$emit('connect')">
        Исправить
      </button>
    </div>

    <!-- Connection Status -->
    <div class="telegram-status">
      <div v-if="connected" class="status-connected">
        <div class="status-indicator connected"></div>
        <div class="status-info">
          <p class="status-text">Подключено к Telegram</p>
          <p v-if="telegramStatus" class="status-details">
            {{ telegramStatus.first_name }} {{ telegramStatus.last_name }}
            <span v-if="telegramStatus.username">@{{ telegramStatus.username }}</span>
          </p>
          <p v-if="proxyModeLabel" class="status-proxy">через {{ proxyModeLabel }}</p>
          <p v-if="currentProxyStatus?.proxy_url" class="status-details">
            {{ currentProxyStatus.proxy_url }}
          </p>
        </div>
        <button class="status-signout-button" @click="$emit('disconnect')" title="Выйти">
          <LogOut :size="16" />
        </button>
      </div>
      <div v-else class="status-disconnected">
        <div class="status-indicator disconnected"></div>
        <div class="status-info">
          <p class="status-text">Не подключено</p>
          <p class="status-details">Авторизуйтесь для использования silero TTS</p>
        </div>
      </div>
    </div>

    <!-- Proxy Settings -->
    <div class="setting-group">
      <div class="proxy-settings-row">
        <div class="proxy-select-row">
          <div class="form-field">
            <label>Прокси:</label>
            <select
              :value="proxyMode"
              @change="handleProxyChange"
              class="network-select"
            >
              <option
                v-for="mode in proxyModes"
                :key="mode.value"
                :value="mode.value"
              >
                {{ mode.label }}
              </option>
            </select>
          </div>
        </div>
        <button
          v-if="connected"
          @click="$emit('reconnect')"
          :disabled="reconnecting"
          class="reconnect-button-fixed"
          title="Переподключить Telegram"
        >
          <RefreshCw v-if="reconnecting" :size="14" class="spin-icon" />
          <RefreshCw v-else :size="14" />
          {{ reconnecting ? 'Переподключение...' : 'Переподключить' }}
        </button>
      </div>
    </div>

    <!-- Connect Button -->
    <div v-if="!connected" class="setting-group">
      <button
        class="telegram-connect-button"
        @click="$emit('connect')"
      >
        Подключить Telegram
      </button>
    </div>

    <!-- Info Section -->
    <div v-if="!connected" class="telegram-info">
      <p class="info-title">Информация:</p>
      <ul class="info-list">
        <li>Для работы silero TTS необходима авторизация через Telegram</li>
        <li>Получите API credentials на <a href="https://my.telegram.org/apps" target="_blank" rel="noopener noreferrer">my.telegram.org</a></li>
        <li>Убедитесь, что в боте <strong>@sileroBot</strong> включены голосовые сообщения</li>
        <li>TTS работает через отправку сообщений в бота и получение голосового ответа</li>
      </ul>
    </div>
  </div>
</template>

<style scoped>
.telegram-connection-status {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

/* Error Banner */
.silero-error-banner {
  padding: 16px;
  background: var(--danger-bg-weak);
  border: 1px solid var(--danger-border-strong);
  border-left: 4px solid var(--color-danger);
  border-radius: 10px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.error-banner-content {
  display: flex;
  align-items: flex-start;
  gap: 12px;
  flex: 1;
}

.error-icon {
  font-size: 20px;
  line-height: 1;
  flex-shrink: 0;
}

.error-text {
  flex: 1;
}

.error-title {
  margin: 0 0 4px;
  font-size: 14px;
  font-weight: 600;
  color: var(--danger-text-bright);
}

.error-message {
  margin: 0;
  font-size: 13px;
  color: var(--danger-text-weak);
}

.fix-button {
  padding: 8px 16px;
  background: var(--danger-bg-hover);
  color: var(--color-text-white);
  border: none;
  border-radius: 10px;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  white-space: nowrap;
  transition: background 0.2s;
}

.fix-button:hover {
  background: var(--danger-border-strong);
}

/* Telegram Status */
.telegram-status {
  padding: 16px;
  background: var(--color-bg-field);
  border: 1px solid var(--color-border);
  border-radius: 12px;
  margin-top: 8px;
}

.status-connected,
.status-disconnected {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.status-indicator {
  width: 12px;
  height: 12px;
  border-radius: 50%;
  flex-shrink: 0;
}

.status-indicator.connected {
  background: var(--status-connected);
  box-shadow: 0 0 0 3px var(--status-connected-glow);
}

.status-indicator.disconnected {
  background: var(--status-disconnected);
  box-shadow: 0 0 0 3px var(--status-disconnected-glow);
}

.status-info {
  flex: 1;
}

.status-text {
  margin: 0;
  font-size: 14px;
  font-weight: 600;
  color: var(--color-text-primary);
}

.status-details {
  margin: 4px 0 0;
  font-size: 13px;
  color: var(--color-text-secondary);
}

.status-proxy {
  margin: 2px 0 0;
  font-size: 12px;
  color: var(--color-accent);
  font-weight: 500;
}

/* Sign out button */
.status-signout-button {
  padding: 6px;
  background: var(--danger-border);
  border: 1px solid var(--danger-border);
  border-radius: 8px;
  color: var(--color-danger);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s;
  flex-shrink: 0;
}

.status-signout-button:hover {
  background: var(--danger-bg-weak);
  color: var(--danger-text-bright);
}

/* Setting group */
.setting-group {
  margin-top: 8px;
}

/* Proxy settings row */
.proxy-settings-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
}

.proxy-select-row {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
  margin-bottom: 8px;
}

.proxy-select-row .form-field {
  display: flex;
  align-items: center;
  gap: 10px;
}

.proxy-select-row label {
  min-width: fit-content;
  font-size: 13px;
  color: var(--color-text-secondary);
  font-weight: 500;
}

.network-select {
  padding: 10px 12px;
  border: 1px solid var(--color-border-strong);
  border-radius: 10px;
  background: var(--color-bg-field-hover);
  color: var(--color-text-primary);
  font-size: 14px;
  cursor: pointer;
  transition: all 0.15s ease;
  width: fit-content;
  min-width: 100px;
}

.network-select:hover {
  background: var(--input-bg-strong);
  border-color: var(--color-border-strong);
}

.network-select:focus {
  outline: none;
  border-color: var(--color-accent);
  box-shadow: 0 0 0 3px var(--color-accent-glow);
}

.network-select option {
  background: var(--select-bg);
  color: var(--color-text-primary);
  padding: 0.3rem 0.5rem;
}

/* Reconnect button */
.reconnect-button-fixed {
  padding: 0.6rem 1.2rem;
  margin-bottom: 8px;
  background: linear-gradient(135deg, var(--color-accent) 0%, var(--color-accent-strong) 100%);
  border: none;
  color: var(--color-text-white);
  border-radius: 10px;
  cursor: pointer;
  font-size: 14px;
  font-weight: 600;
  transition: all 0.2s;
  display: inline-flex;
  align-items: center;
  gap: 8px;
}

.reconnect-button-fixed:hover:not(:disabled) {
  filter: brightness(1.06);
}

.reconnect-button-fixed:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.spin-icon {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

/* Connect button */
.telegram-connect-button {
  width: 100%;
  padding: 12px 20px;
  background: linear-gradient(135deg, var(--color-accent) 0%, var(--color-accent-strong) 100%);
  color: var(--color-text-white);
  border: none;
  border-radius: 6px;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: background 0.2s;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
}

.telegram-connect-button:hover {
  filter: brightness(1.06);
}

/* Info section */
.telegram-info {
  padding: 16px;
  background: var(--info-bg-weak);
  border-left: 4px solid var(--color-accent);
  border-radius: 10px;
}

.info-title {
  margin: 0 0 8px;
  font-size: 14px;
  font-weight: 600;
  color: var(--color-text-primary);
}

.info-list {
  margin: 0;
  padding-left: 20px;
  font-size: 13px;
  color: var(--color-text-secondary);
  line-height: 1.6;
}

.info-list li {
  margin-bottom: 4px;
}

.info-list li:last-child {
  margin-bottom: 0;
}

.info-list a {
  color: var(--color-info);
  text-decoration: none;
  font-weight: 500;
}

.info-list a:hover {
  text-decoration: underline;
}

.info-list strong {
  color: var(--color-text-primary);
}
</style>
