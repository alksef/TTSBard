<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { Loader2 } from 'lucide-vue-next';
import { debugLog } from '../../utils/debug';
import InputWithToggle from '../shared/InputWithToggle.vue';
import StatusMessage from '../shared/StatusMessage.vue';
import TestResult, { type TestResult as TestResultType } from '../shared/TestResult.vue';

// Types for proxy settings
interface ProxySettings {
  proxy_url: string | null;
  proxy_type: 'socks5' | 'socks4' | 'http';
}

interface MtProxySettings {
  host?: string;
  port: number;
  secret?: string;
  dc_id?: number;
}

// Emit status message event for parent to display
const emit = defineEmits<{
  (e: 'show-message', message: string): void;
}>();

// State - individual fields for SOCKS5
const host = ref<string>('');
const port = ref<string>('');
const username = ref<string>('');
const password = ref<string>('');

// State - individual fields for MTProxy
const mtHost = ref<string>('');
const mtPort = ref<string>('');
const mtSecret = ref<string>('');
const mtDcId = ref<string>('');

// DC ID options for MTProxy
const dcIdOptions = [
  { value: '', label: 'Авто' },
  { value: '1', label: '1' },
  { value: '2', label: '2' },
  { value: '3', label: '3' },
  { value: '4', label: '4' },
  { value: '5', label: '5' },
];

// UI State
const isLoadingNetwork = ref(false);
const isTestingSocks5 = ref(false);
const isTestingMtProxy = ref(false);
const isSavingNetwork = ref(false);
const socks5TestResult = ref<TestResultType | null>(null);
const mtProxyTestResult = ref<TestResultType | null>(null);

// Status message state (local to network tab)
const statusMessage = ref<string>('');
const statusType = ref<'success' | 'error' | 'info'>('info');

// Timer IDs for cleanup on unmount
let socks5TestTimeoutId: ReturnType<typeof setTimeout> | null = null;
let mtProxyTestTimeoutId: ReturnType<typeof setTimeout> | null = null;
let networkStatusTimeoutId: ReturnType<typeof setTimeout> | null = null;

// Computed: check if any field has value
const hasProxyData = computed(() => {
  return host.value || port.value || username.value || password.value;
});

// Computed: check if MTProxy has data
const hasMtProxyData = computed(() => {
  return mtHost.value || mtSecret.value || mtDcId.value;
});

// Computed: build SOCKS5 URL from fields
const socks5Url = computed(() => {
  if (!host.value.trim()) {
    return '';
  }
  const portNum = port.value || '1080';
  let url = `socks5://`;
  if (username.value) {
    const auth = password.value ? `${username.value}:${password.value}` : username.value;
    url += `${auth}@`;
  }
  url += `${host.value}:${portNum}`;
  return url;
});

function showStatus(message: string, type: 'success' | 'error' | 'info') {
  statusMessage.value = message;
  statusType.value = type;

  // Auto-hide success messages after 3 seconds
  if (type === 'success') {
    networkStatusTimeoutId = setTimeout(() => {
      if (statusType.value === 'success') {
        statusMessage.value = '';
        networkStatusTimeoutId = null;
      }
    }, 3000);
  }
}

function dismissStatus() {
  statusMessage.value = '';
}

async function loadProxySettings() {
  isLoadingNetwork.value = true;
  try {
    const settings = await invoke<ProxySettings>('get_proxy_settings');
    debugLog('[SettingsNetwork] Loaded proxy settings:', settings);

    // Parse existing proxy URL to extract fields
    if (settings.proxy_url) {
      parseProxyUrl(settings.proxy_url);
    }
  } catch (error) {
    console.error('Failed to load proxy settings:', error);
    showStatus('Ошибка загрузки настроек SOCKS5: ' + (error as Error).message, 'error');
  } finally {
    isLoadingNetwork.value = false;
  }
}

function parseProxyUrl(url: string) {
  try {
    // Remove socks5:// prefix
    let urlWithoutPrefix = url.replace(/^socks5:\/\//i, '');

    // Extract auth if present
    let authPart = '';
    const atIndex = urlWithoutPrefix.indexOf('@');
    if (atIndex !== -1) {
      authPart = urlWithoutPrefix.substring(0, atIndex);
      urlWithoutPrefix = urlWithoutPrefix.substring(atIndex + 1);
    }

    // Parse username:password
    if (authPart) {
      const colonIndex = authPart.indexOf(':');
      if (colonIndex !== -1) {
        username.value = authPart.substring(0, colonIndex);
        password.value = authPart.substring(colonIndex + 1);
      }
    }

    // Parse host:port
    const colonIndex = urlWithoutPrefix.lastIndexOf(':');
    if (colonIndex !== -1) {
      host.value = urlWithoutPrefix.substring(0, colonIndex);
      port.value = urlWithoutPrefix.substring(colonIndex + 1);
    } else {
      host.value = urlWithoutPrefix;
    }
  } catch (error) {
    console.error('Failed to parse proxy URL:', error);
  }
}

async function saveNetworkSettings() {
  if (!hasProxyData.value) {
    // Clear proxy settings
    try {
      await invoke('set_proxy_url', {
        url: '',
        proxyType: 'socks5'
      });
      showStatus('Настройки SOCKS5 очищены', 'success');
    } catch (error) {
      showStatus('Ошибка сохранения: ' + (error as Error).message, 'error');
    }
    return;
  }

  // Validate host
  if (!host.value.trim()) {
    showStatus('Введите хост SOCKS5 прокси', 'error');
    return;
  }

  // Validate port
  const portNum = parseInt(port.value) || 1080;
  if (isNaN(portNum) || portNum < 1 || portNum > 65535) {
    showStatus('Порт должен быть от 1 до 65535', 'error');
    return;
  }

  isSavingNetwork.value = true;
  try {
    await invoke('set_proxy_url', {
      url: socks5Url.value,
      proxyType: 'socks5'
    });
    showStatus('Настройки SOCKS5 сохранены', 'success');
  } catch (error) {
    console.error('Failed to save proxy URL:', error);
    showStatus('Ошибка сохранения: ' + (error as Error).message, 'error');
  } finally {
    isSavingNetwork.value = false;
  }
}

async function testConnection() {
  if (!hasProxyData.value) {
    showStatus('Введите данные SOCKS5 прокси для тестирования', 'error');
    return;
  }

  if (!host.value.trim()) {
    showStatus('Введите хост SOCKS5 прокси', 'error');
    return;
  }

  const portNum = parseInt(port.value) || 1080;

  isTestingSocks5.value = true;
  socks5TestResult.value = null;

  // Clear any existing timeout for SOCKS5 test
  if (socks5TestTimeoutId !== null) {
    clearTimeout(socks5TestTimeoutId);
    socks5TestTimeoutId = null;
  }

  try {
    const result = await invoke<TestResultType>('test_proxy', {
      proxyType: 'socks5',
      host: host.value,
      port: portNum,
      timeoutSecs: 3
    });

    socks5TestResult.value = result;

    // Auto-clear test result after 20 seconds
    socks5TestTimeoutId = setTimeout(() => {
      if (socks5TestResult.value === result) {
        socks5TestResult.value = null;
        socks5TestTimeoutId = null;
      }
    }, 20000);

    if (result.success) {
      showStatus(
        `Соединение успешно! Задержка: ${result.latency_ms}мс`,
        'success'
      );
    } else {
      showStatus(
        `Ошибка соединения: ${result.error || 'Неизвестная ошибка'}`,
        'error'
      );
    }
  } catch (error) {
    console.error('Failed to test proxy:', error);
    socks5TestResult.value = {
      success: false,
      latency_ms: null,
      mode: 'socks5',
      error: (error as Error).message
    };
    showStatus('Ошибка тестирования: ' + (error as Error).message, 'error');
  } finally {
    isTestingSocks5.value = false;
  }
}

async function loadMtProxySettings() {
  try {
    const settings = await invoke<MtProxySettings>('get_mtproxy_settings');
    debugLog('[SettingsNetwork] Loaded MTProxy settings:', settings);
    mtHost.value = settings.host || '';
    // Показываем пустое поле если порт = дефолт (8888)
    mtPort.value = settings.port === 8888 ? '' : settings.port.toString();
    mtSecret.value = settings.secret || '';
    mtDcId.value = settings.dc_id?.toString() || '';
  } catch (error) {
    console.error('Failed to load MTProxy settings:', error);
    showStatus('Ошибка загрузки настроек MTProxy: ' + (error as Error).message, 'error');
  }
}

async function saveMtProxySettings() {
  // Validate host
  if (!mtHost.value.trim()) {
    showStatus('Введите хост MTProxy', 'error');
    return;
  }

  // Validate port
  const portNum = parseInt(mtPort.value) || 8888;
  if (isNaN(portNum) || portNum < 1 || portNum > 65535) {
    showStatus('Порт должен быть от 1 до 65535', 'error');
    return;
  }

  // Validate secret format (optional if clearing)
  if (mtSecret.value.trim()) {
    const secretLen = mtSecret.value.trim().length;
    if (secretLen !== 24 && secretLen !== 32 && secretLen !== 34) {
      showStatus('Секрет должен быть 24 (base64), 32 или 34 символа (hex)', 'error');
      return;
    }
  }

  // DC ID from select (always valid due to select constraints)
  const dcIdNum: number | undefined = mtDcId.value ? parseInt(mtDcId.value) : undefined;

  isSavingNetwork.value = true;
  try {
    await invoke('set_mtproxy_settings', {
      host: mtHost.value.trim() || undefined,
      port: portNum,
      secret: mtSecret.value.trim() || undefined,
      dcId: dcIdNum
    });
    showStatus('Настройки MTProxy сохранены', 'success');
  } catch (error) {
    console.error('Failed to save MTProxy settings:', error);
    showStatus('Ошибка сохранения: ' + (error as Error).message, 'error');
  } finally {
    isSavingNetwork.value = false;
  }
}

async function testMtProxyConnection() {
  // Validate host
  if (!mtHost.value.trim()) {
    showStatus('Введите хост MTProxy', 'error');
    return;
  }

  // Validate secret
  if (!mtSecret.value.trim()) {
    showStatus('Введите секрет MTProxy', 'error');
    return;
  }

  const portNum = parseInt(mtPort.value) || 8888;

  isTestingMtProxy.value = true;
  mtProxyTestResult.value = null;

  // Clear any existing timeout for MTProxy test
  if (mtProxyTestTimeoutId !== null) {
    clearTimeout(mtProxyTestTimeoutId);
    mtProxyTestTimeoutId = null;
  }

  try {
    const result = await invoke<TestResultType>('test_mtproxy', {
      host: mtHost.value,
      port: portNum,
      secret: mtSecret.value,
      dcId: mtDcId.value ? parseInt(mtDcId.value) : null,
      timeoutSecs: 10
    });

    mtProxyTestResult.value = result;

    // Auto-clear test result after 20 seconds
    mtProxyTestTimeoutId = setTimeout(() => {
      if (mtProxyTestResult.value === result) {
        mtProxyTestResult.value = null;
        mtProxyTestTimeoutId = null;
      }
    }, 20000);

    if (result.success) {
      showStatus(
        `Соединение MTProxy успешно! Задержка: ${result.latency_ms}мс`,
        'success'
      );
    } else {
      showStatus(
        `Ошибка соединения MTProxy: ${result.error || 'Неизвестная ошибка'}`,
        'error'
      );
    }
  } catch (error) {
    console.error('Failed to test MTProxy:', error);
    mtProxyTestResult.value = {
      success: false,
      latency_ms: null,
      mode: 'mtproxy',
      error: (error as Error).message
    };
    showStatus('Ошибка тестирования MTProxy: ' + (error as Error).message, 'error');
  } finally {
    isTestingMtProxy.value = false;
  }
}

// ============================================================================
// Lifecycle
// ============================================================================

onMounted(async () => {
  await loadProxySettings();
  await loadMtProxySettings();
});

// Cleanup timers on unmount to prevent memory leaks
onUnmounted(() => {
  if (socks5TestTimeoutId !== null) {
    clearTimeout(socks5TestTimeoutId);
    socks5TestTimeoutId = null;
  }
  if (mtProxyTestTimeoutId !== null) {
    clearTimeout(mtProxyTestTimeoutId);
    mtProxyTestTimeoutId = null;
  }
  if (networkStatusTimeoutId !== null) {
    clearTimeout(networkStatusTimeoutId);
    networkStatusTimeoutId = null;
  }
});
</script>

<template>
  <div class="settings-network">
    <!-- Status Message (local to network tab) -->
    <StatusMessage
      :message="statusMessage"
      :type="statusType"
      @dismiss="dismissStatus"
    />

    <div v-if="isLoadingNetwork" class="loading-state">
      <Loader2 :size="24" class="spinner" />
      <span>Загрузка настроек...</span>
    </div>

    <div v-else class="network-content">
      <!-- SOCKS5 Section -->
      <section class="settings-section">
        <h2>SOCKS5</h2>

        <div class="network-form">
          <!-- Host and Port Row -->
          <div class="form-row">
            <label>Хост:</label>
            <input
              v-model="host"
              type="text"
              class="network-input network-input-host"
            />
            <label>Порт:</label>
            <input
              v-model="port"
              type="number"
              min="1"
              max="65535"
              class="network-input network-input-port"
            />
          </div>

          <!-- Username and Password Row -->
          <div class="form-row">
            <label>Логин:</label>
            <input
              v-model="username"
              type="text"
              placeholder="(опционально)"
              class="network-input network-input-host"
            />
            <label>Пароль:</label>
            <InputWithToggle
              v-model="password"
              type="password"
              placeholder="(опционально)"
              class="network-input-wide"
            />
          </div>

          <!-- Buttons Row -->
          <div class="button-row">
            <button
              @click="testConnection"
              :disabled="isTestingSocks5 || !hasProxyData"
              class="test-button"
              :class="{ disabled: isTestingSocks5 || !hasProxyData }"
            >{{ isTestingSocks5 ? 'Проверка...' : 'Тест' }}</button>
            <button @click="saveNetworkSettings" :disabled="isSavingNetwork" class="save-button-inline">Сохранить</button>
          </div>

          <!-- Test Result -->
          <TestResult :result="socks5TestResult" />
        </div>
      </section>

      <!-- MTProxy Section -->
      <section class="settings-section">
        <h2>MTProxy</h2>

        <div class="network-form">
          <!-- Host and Port Row -->
          <div class="form-row">
            <label>Хост:</label>
            <input
              v-model="mtHost"
              type="text"
              class="network-input network-input-host"
            />
            <label>Порт:</label>
            <input
              v-model="mtPort"
              type="number"
              min="1"
              max="65535"
              class="network-input network-input-port"
            />
          </div>

          <!-- Secret Row -->
          <div class="form-row">
            <label>Ключ:</label>
            <InputWithToggle
              v-model="mtSecret"
              type="password"
              class="network-input-key-wide"
            />
          </div>

          <!-- DC ID Row (Optional) -->
          <div class="form-row">
            <label>DC ID:</label>
            <select
              v-model="mtDcId"
              class="network-select dc-id-select"
            >
              <option v-for="opt in dcIdOptions" :key="opt.value" :value="opt.value">
                {{ opt.label }}
              </option>
            </select>
          </div>

          <!-- Buttons Row -->
          <div class="button-row">
            <button
              @click="testMtProxyConnection"
              :disabled="isTestingMtProxy || !hasMtProxyData"
              class="test-button"
              :class="{ disabled: isTestingMtProxy || !hasMtProxyData }"
            >{{ isTestingMtProxy ? 'Проверка...' : 'Тест' }}</button>
            <button @click="saveMtProxySettings" :disabled="isSavingNetwork" class="save-button-inline">Сохранить</button>
          </div>

          <!-- Test Result -->
          <TestResult :result="mtProxyTestResult" />
        </div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.settings-network {
  display: flex;
  flex-direction: column;
  position: relative;
}

.settings-section {
  margin-bottom: 1.5rem;
  padding: 12px 16px;
  background: var(--color-bg-field);
  border: 1px solid var(--color-border);
  border-radius: 12px;
  backdrop-filter: blur(8px);
}

.settings-section:last-child {
  margin-bottom: 0;
}

.settings-section h2 {
  margin: 0 0 1rem;
  font-size: 1.1rem;
  font-weight: 700;
  color: var(--color-text-primary);
  letter-spacing: 0.01em;
}

.network-content {
  display: flex;
  flex-direction: column;
}

.network-form {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.form-row {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}

.form-row label {
  font-size: 13px;
  color: var(--color-text-secondary);
  font-weight: 500;
  min-width: 50px;
}

.network-input {
  flex: 1;
  padding: 8px 12px;
  border: 1px solid var(--color-border-strong);
  border-radius: 8px;
  background: var(--color-bg-field);
  color: var(--color-text-primary);
  font-size: 14px;
  font-family: var(--font-mono);
  transition: all 0.15s ease;
  min-width: 0;
  box-sizing: border-box;
}

.network-input-host {
  max-width: 150px;
}

.network-input-port {
  max-width: 100px;
}

.network-input-wide {
  flex: 1;
  max-width: 200px;
}

.network-input-key-wide {
  flex: 1;
  max-width: 372px;
}

.network-input:hover {
  background: var(--color-bg-field-hover);
  border-color: var(--color-border-strong);
}

.network-input:focus {
  outline: none;
  border-color: var(--color-accent);
  box-shadow: 0 0 0 3px var(--color-accent-glow);
}

.network-input::placeholder {
  color: var(--color-text-muted);
  font-size: 13px;
  font-family: var(--font-sans);
}

.network-select {
  padding: 10px 12px;
  border: 1px solid var(--color-border-strong);
  border-radius: 10px;
  background: var(--color-bg-field);
  color: var(--color-text-primary);
  font-size: 14px;
  cursor: pointer;
  transition: all 0.15s ease;
}

.network-select:hover {
  background: var(--color-bg-field-hover);
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

.dc-id-select {
  max-width: 150px;
}

/* Buttons */
.button-row {
  display: flex;
  gap: 0.75rem;
  flex-wrap: wrap;
  align-items: center;
  justify-content: flex-end;
  margin-top: 0.5rem;
  padding-top: 0.5rem;
  border-top: 1px solid var(--color-border);
}

.test-button,
.save-button-inline {
  padding: 0.6rem 1.2rem;
  border: none;
  border-radius: 10px;
  cursor: pointer;
  font-weight: 600;
  font-size: 14px;
  transition: all 0.2s;
}

.save-button-inline {
  background: linear-gradient(135deg, var(--color-accent) 0%, var(--color-accent-strong) 100%);
  color: var(--color-text-white);
}

.save-button-inline:hover:not(:disabled) {
  filter: brightness(1.06);
}

.save-button-inline:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.test-button {
  background: linear-gradient(135deg, var(--color-accent) 0%, var(--color-accent-strong) 100%);
  color: var(--color-text-white);
}

.test-button:hover:not(:disabled) {
  filter: brightness(1.1);
}

.test-button.disabled {
  background: var(--btn-disabled-bg);
  cursor: not-allowed;
  opacity: 0.6;
}

.test-button.disabled:hover {
  background: var(--btn-disabled-bg);
}

/* Loading State */
.loading-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  padding: 40px;
  color: var(--color-text-secondary);
}

.loading-state .spinner {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

@media (max-width: 600px) {
  .form-row {
    grid-template-columns: 1fr;
  }
}
</style>
