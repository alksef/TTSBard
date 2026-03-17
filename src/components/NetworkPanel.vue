<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { Check, X, Shield, Eye, EyeOff, AlertTriangle, Loader2 } from 'lucide-vue-next';
import { debugLog } from '../utils/debug';

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

interface TestResult {
  success: boolean;
  latency_ms: number | null;
  mode: string;
  error: string | null;
}

// State - individual fields for SOCKS5
const host = ref<string>('');
const port = ref<string>('');
const username = ref<string>('');
const password = ref<string>('');
const showPassword = ref(false);

// State - individual fields for MTProxy
const mtHost = ref<string>('');
const mtPort = ref<string>('');
const mtSecret = ref<string>('');
const mtShowSecret = ref(false);
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
const testHost = ref<string>(''); // For connection test
const isLoading = ref(false);
const isTestingSocks5 = ref(false);
const isTestingMtProxy = ref(false);
const isSaving = ref(false);
const socks5TestResult = ref<TestResult | null>(null);
const mtProxyTestResult = ref<TestResult | null>(null);
const statusMessage = ref<string>('');
const statusType = ref<'success' | 'error' | 'info'>('info');

// Timer IDs for cleanup on unmount
let socks5TestTimeoutId: ReturnType<typeof setTimeout> | null = null;
let mtProxyTestTimeoutId: ReturnType<typeof setTimeout> | null = null;
let statusTimeoutId: ReturnType<typeof setTimeout> | null = null;

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

// Load proxy settings on mount
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
  if (statusTimeoutId !== null) {
    clearTimeout(statusTimeoutId);
    statusTimeoutId = null;
  }
});

async function loadProxySettings() {
  isLoading.value = true;
  try {
    const settings = await invoke<ProxySettings>('get_proxy_settings');
    debugLog('[NetworkPanel] Loaded proxy settings:', settings);

    // Parse existing proxy URL to extract fields
    if (settings.proxy_url) {
      parseProxyUrl(settings.proxy_url);
    }
  } catch (error) {
    console.error('Failed to load proxy settings:', error);
    showStatus('Ошибка загрузки настроек SOCKS5: ' + (error as Error).message, 'error');
  } finally {
    isLoading.value = false;
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

async function saveSettings() {
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

  isSaving.value = true;
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
    isSaving.value = false;
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
    const result = await invoke<TestResult>('test_proxy', {
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

// ============================================================================
// MTProxy Functions
// ============================================================================

async function loadMtProxySettings() {
  try {
    const settings = await invoke<MtProxySettings>('get_mtproxy_settings');
    debugLog('[NetworkPanel] Loaded MTProxy settings:', settings);
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

  isSaving.value = true;
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
    isSaving.value = false;
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
    const result = await invoke<TestResult>('test_mtproxy', {
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

function showStatus(message: string, type: 'success' | 'error' | 'info') {
  statusMessage.value = message;
  statusType.value = type;

  // Auto-hide success messages after 3 seconds
  if (type === 'success') {
    statusTimeoutId = setTimeout(() => {
      if (statusType.value === 'success') {
        statusMessage.value = '';
        statusTimeoutId = null;
      }
    }, 3000);
  }
}

function dismissStatus() {
  statusMessage.value = '';
}
</script>

<template>
  <div class="network-panel">
    <!-- Status Message -->
    <Transition name="fade">
      <div v-if="statusMessage" class="status-message" :class="statusType">
        <Check v-if="statusType === 'success'" :size="16" />
        <AlertTriangle v-else-if="statusType === 'error'" :size="16" />
        <Shield v-else :size="16" />
        <span>{{ statusMessage }}</span>
        <button class="status-close" @click="dismissStatus" title="Закрыть">
          <X :size="14" />
        </button>
      </div>
    </Transition>

    <div v-if="isLoading" class="loading-state">
      <Loader2 :size="24" class="spinner" />
      <span>Загрузка настроек...</span>
    </div>

    <div v-else class="network-content">
      <!-- SOCKS5 Section -->
      <section class="network-section">
        <div class="section-header">
          <h3>SOCKS5</h3>
        </div>

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
            <div class="input-with-toggle">
              <input
                :type="showPassword ? 'text' : 'password'"
                v-model="password"
                placeholder="(опционально)"
                class="network-input"
              />
              <button
                type="button"
                class="toggle-icon-button"
                @click="showPassword = !showPassword"
                :title="showPassword ? 'Скрыть' : 'Показать'"
              >
                <Eye v-if="!showPassword" :size="18" />
                <EyeOff v-else :size="18" />
              </button>
            </div>
          </div>

          <!-- Buttons Row -->
          <div class="button-row">
            <input
              v-model="testHost"
              type="text"
              placeholder="telegram.com"
              class="test-host-input"
            />
            <button
              @click="testConnection"
              :disabled="isTestingSocks5 || !hasProxyData"
              class="test-button"
              :class="{ disabled: isTestingSocks5 || !hasProxyData }"
            >{{ isTestingSocks5 ? 'Проверка...' : 'Тест' }}</button>
            <button @click="saveSettings" :disabled="isSaving" class="save-button-inline">Сохранить</button>
          </div>

          <!-- Test Result -->
          <Transition name="fade">
            <div v-if="socks5TestResult" class="test-result" :class="{ success: socks5TestResult.success, error: !socks5TestResult.success }">
              <Check v-if="socks5TestResult.success" :size="16" />
              <X v-else :size="16" />
              <span v-if="socks5TestResult.success">
                Соединение успешно <span v-if="socks5TestResult.latency_ms">{{ socks5TestResult.latency_ms }}мс</span>
              </span>
              <span v-else>{{ socks5TestResult.error || 'Ошибка соединения' }}</span>
            </div>
          </Transition>
        </div>

      </section>

      <!-- MTProxy Section -->
      <section class="network-section">
        <div class="section-header">
          <h3>MTProxy</h3>
        </div>

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
            <div class="input-with-toggle input-with-toggle-wide">
              <input
                :type="mtShowSecret ? 'text' : 'password'"
                v-model="mtSecret"
                class="network-input network-input-key"
              />
              <button
                type="button"
                class="toggle-icon-button"
                @click="mtShowSecret = !mtShowSecret"
                :title="mtShowSecret ? 'Скрыть' : 'Показать'"
              >
                <Eye v-if="!mtShowSecret" :size="18" />
                <EyeOff v-else :size="18" />
              </button>
            </div>
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
            <button @click="saveMtProxySettings" :disabled="isSaving" class="save-button-inline">Сохранить</button>
          </div>

          <!-- Test Result -->
          <Transition name="fade">
            <div v-if="mtProxyTestResult" class="test-result" :class="{ success: mtProxyTestResult.success, error: !mtProxyTestResult.success }">
              <Check v-if="mtProxyTestResult.success" :size="16" />
              <X v-else :size="16" />
              <span v-if="mtProxyTestResult.success">
                Соединение MTProxy успешно <span v-if="mtProxyTestResult.latency_ms">{{ mtProxyTestResult.latency_ms }}мс</span>
              </span>
              <span v-else>{{ mtProxyTestResult.error || 'Ошибка соединения MTProxy' }}</span>
            </div>
          </Transition>
        </div>

      </section>
    </div>
  </div>
</template>

<style scoped>
.network-panel {
  max-width: 700px;
  margin: 0 auto;
}

/* Status Message */
.status-message {
  position: fixed;
  top: 20px;
  left: calc(50% + 100px);
  transform: translateX(-50%);
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0.4rem 0.75rem;
  padding-right: 2rem;
  border-radius: 8px;
  font-size: 12px;
  font-weight: 500;
  z-index: 1000;
  box-shadow: 0 4px 20px rgba(0, 0, 0, 0.3);
  backdrop-filter: blur(10px);
  animation: slideDownFade 0.3s ease-out;
  white-space: nowrap;
}

.status-close {
  position: absolute;
  right: 6px;
  top: 50%;
  transform: translateY(-50%);
  background: transparent;
  border: none;
  padding: 2px;
  cursor: pointer;
  color: inherit;
  opacity: 0.7;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 4px;
  transition: opacity 0.15s;
}

.status-close:hover {
  opacity: 1;
}

.status-message.success {
  background: var(--success-bg);
  border: 1px solid var(--success-border, rgba(74, 222, 128, 0.4));
  color: var(--success-text);
}

.status-message.error {
  background: var(--danger-bg);
  border: 1px solid var(--danger-border-strong);
  border-left: 4px solid var(--danger-border-strong);
  color: var(--danger-text);
}

.status-message.info {
  background: var(--info-bg);
  border: 1px solid var(--info-border);
  color: var(--info-text);
}

@keyframes slideDownFade {
  from {
    opacity: 0;
    transform: translateX(-50%) translateY(-20px);
  }
  to {
    opacity: 1;
    transform: translateX(-50%) translateY(0);
  }
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

/* Network Content */
.network-content {
  display: flex;
  flex-direction: column;
  gap: 20px;
}

/* Network Section */
.network-section {
  background: var(--color-bg-field);
  border: 1px solid var(--color-border);
  border-radius: 12px;
  padding: 12px 16px;
  backdrop-filter: blur(8px);
  transition: all 0.2s ease;
}

.section-header {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 16px;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--color-border);
}

.section-header h3 {
  flex: 1;
  font-size: 1rem;
  font-weight: 600;
  color: var(--color-text-primary);
  margin: 0;
}

/* Form */
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
}

.network-input-host {
  max-width: 150px;
}

.network-input-port {
  max-width: 100px;
}

.network-input-key {
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

/* Input with toggle icon button */
.input-with-toggle {
  position: relative;
  flex: 1;
  max-width: 400px;
}

.input-with-toggle .network-input {
  width: 100%;
  padding-right: 40px;
}

.input-with-toggle .network-input-key {
  max-width: 372px;
}

.toggle-icon-button {
  position: absolute;
  right: 4px;
  top: 50%;
  transform: translateY(-50%);
  padding: 4px;
  background: transparent;
  border: none;
  cursor: pointer;
  color: var(--color-text-secondary);
  display: flex;
  align-items: center;
  justify-content: center;
  transition: color 0.2s;
}

.toggle-icon-button:hover {
  color: var(--color-accent);
}

/* Wide input toggle for MTProxy secret */
.input-with-toggle-wide {
  position: relative;
  flex: 1;
  max-width: 100%;
}

.input-with-toggle-wide .network-input {
  width: 100%;
  padding-right: 40px;
}

.input-with-toggle-wide .network-input-key {
  max-width: 100%;
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

.test-host-input {
  padding: 0.6rem 1rem;
  border: 1px solid var(--color-border-strong);
  border-radius: 10px;
  background: var(--color-bg-field);
  color: var(--color-text-primary);
  font-size: 14px;
  min-width: 150px;
  flex: 1;
}

.test-host-input:focus {
  outline: none;
  border-color: var(--color-accent);
  box-shadow: 0 0 0 3px var(--color-accent-glow);
}

.test-host-input::placeholder {
  color: var(--color-text-muted);
  opacity: 0.6;
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
  background: var(--btn-accent-bg);
  color: var(--color-text-white);
}

.test-button:hover:not(:disabled) {
  background: var(--btn-accent-bg-hover);
}

.test-button.disabled {
  background: var(--btn-disabled-bg);
  cursor: not-allowed;
  opacity: 0.6;
}

.test-button.disabled:hover {
  background: var(--btn-disabled-bg);
}

/* Test Result */
.test-result {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border-radius: 8px;
  font-size: 13px;
  font-weight: 500;
}

.test-result.success {
  background: var(--success-bg-weak);
  border: 1px solid var(--success-border, rgba(74, 222, 128, 0.24));
  color: var(--success-text-bright);
}

.test-result.error {
  background: var(--danger-bg-weak);
  border: 1px solid var(--danger-border-strong);
  color: var(--danger-text-weak);
}

.test-result span {
  display: flex;
  align-items: center;
  gap: 6px;
}

/* Hint Text */
.hint-text {
  display: block;
  margin-top: 8px;
  font-size: 12px;
  color: var(--color-text-muted);
  line-height: 1.4;
}

/* Transitions */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

@media (max-width: 600px) {
  .form-row {
    grid-template-columns: 1fr;
  }
}
</style>
