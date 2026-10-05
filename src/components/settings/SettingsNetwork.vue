<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { Loader2 } from 'lucide-vue-next';
import { debugLog, debugError } from '../../utils/debug';
import { presentCommandError } from '../../ipc/commandError';
import { t } from '../../i18n';
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
const dcIdOptions = computed(() => [
  { value: '', label: t('settings.network.auto') },
  { value: '1', label: '1' },
  { value: '2', label: '2' },
  { value: '3', label: '3' },
  { value: '4', label: '4' },
  { value: '5', label: '5' },
]);

function formatError(e: unknown): string {
  if (e instanceof Error) return e.message;
  if (typeof e === 'string') return e;
  return String(e);
}

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
    debugLog('[SettingsNetwork] Loaded proxy settings, has_proxy_url:', !!settings.proxy_url);

    // Parse existing proxy URL to extract fields
    if (settings.proxy_url) {
      parseProxyUrl(settings.proxy_url);
    }
  } catch (error) {
    debugError('Failed to load proxy settings:', error);
    showStatus(presentCommandError(error, t('settings.network.error.load_socks5')), 'error');
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
    debugError('Failed to parse proxy URL:', error);
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
      showStatus(t('settings.network.socks5.cleared'), 'success');
    } catch (error) {
      showStatus(presentCommandError(error, t('settings.network.error.save')), 'error');
    }
    return;
  }

  // Validate host
  if (!host.value.trim()) {
    showStatus(t('settings.network.socks5.host_required'), 'error');
    return;
  }

  // Validate port
  const portNum = parseInt(port.value) || 1080;
  if (isNaN(portNum) || portNum < 1 || portNum > 65535) {
    showStatus(t('settings.network.error.port_range'), 'error');
    return;
  }

  isSavingNetwork.value = true;
  try {
    await invoke('set_proxy_url', {
      url: socks5Url.value,
      proxyType: 'socks5'
    });
    showStatus(t('settings.network.socks5.saved'), 'success');
  } catch (error) {
    debugError('Failed to save proxy URL:', error);
    showStatus(presentCommandError(error, t('settings.network.error.save')), 'error');
  } finally {
    isSavingNetwork.value = false;
  }
}

async function testConnection() {
  if (!hasProxyData.value) {
    showStatus(t('settings.network.socks5.test_data_required'), 'error');
    return;
  }

  if (!host.value.trim()) {
    showStatus(t('settings.network.socks5.host_required'), 'error');
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
        t('settings.network.test_success', { latency: result.latency_ms }),
        'success'
      );
    } else {
      showStatus(t('settings.network.error.connection'), 'error');
    }
  } catch (error) {
    debugError('Failed to test proxy:', error);
    socks5TestResult.value = {
      success: false,
      latency_ms: null,
      mode: 'socks5',
      error: formatError(error)
    };
    showStatus(presentCommandError(error, t('settings.network.error.test')), 'error');
  } finally {
    isTestingSocks5.value = false;
  }
}

async function loadMtProxySettings() {
  try {
    const settings = await invoke<MtProxySettings>('get_mtproxy_settings');
    debugLog('[SettingsNetwork] Loaded MTProxy settings, has_secret:', !!settings.secret, 'host:', settings.host);
    mtHost.value = settings.host || '';
    // Показываем пустое поле если порт = дефолт (8888)
    mtPort.value = settings.port === 8888 ? '' : settings.port.toString();
    mtSecret.value = settings.secret || '';
    mtDcId.value = settings.dc_id?.toString() || '';
  } catch (error) {
    debugError('Failed to load MTProxy settings:', error);
    showStatus(presentCommandError(error, t('settings.network.error.load_mtproxy')), 'error');
  }
}

async function saveMtProxySettings() {
  // Validate host
  if (!mtHost.value.trim()) {
    showStatus(t('settings.network.mtproxy.host_required'), 'error');
    return;
  }

  // Validate port
  const portNum = parseInt(mtPort.value) || 8888;
  if (isNaN(portNum) || portNum < 1 || portNum > 65535) {
    showStatus(t('settings.network.error.port_range'), 'error');
    return;
  }

  // Validate secret format (optional if clearing)
  if (mtSecret.value.trim()) {
    const secretLen = mtSecret.value.trim().length;
    if (secretLen < 24 || (secretLen >= 32 && secretLen % 2 !== 0)) {
      showStatus(t('settings.network.mtproxy.secret_invalid'), 'error');
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
    showStatus(t('settings.network.mtproxy.saved'), 'success');
  } catch (error) {
    debugError('Failed to save MTProxy settings:', error);
    showStatus(presentCommandError(error, t('settings.network.error.save')), 'error');
  } finally {
    isSavingNetwork.value = false;
  }
}

async function testMtProxyConnection() {
  // Validate host
  if (!mtHost.value.trim()) {
    showStatus(t('settings.network.mtproxy.host_required'), 'error');
    return;
  }

  // Validate secret
  if (!mtSecret.value.trim()) {
    showStatus(t('settings.network.mtproxy.secret_required'), 'error');
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
        t('settings.network.test_success_mtproxy', { latency: result.latency_ms }),
        'success'
      );
    } else {
      showStatus(t('settings.network.error.connection_mtproxy'), 'error');
    }
  } catch (error) {
    debugError('Failed to test MTProxy:', error);
    mtProxyTestResult.value = {
      success: false,
      latency_ms: null,
      mode: 'mtproxy',
      error: formatError(error)
    };
    showStatus(presentCommandError(error, t('settings.network.error.test_mtproxy')), 'error');
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
      <span class="ui-status">{{ t('settings.network.loading') }}</span>
    </div>

    <div v-else class="network-content">
      <!-- SOCKS5 Section -->
      <section class="settings-section ui-section">
        <h2 class="ui-section-title">SOCKS5</h2>

        <div class="network-form">
          <div class="network-grid">
            <label class="ui-label ui-label--secondary network-host-label">
              <span>{{ t('settings.network.host') }}</span>
              <span class="network-label-sizer" aria-hidden="true">{{ t('settings.network.login') }}</span>
              <span class="network-label-sizer" aria-hidden="true">{{ t('settings.network.secret') }}</span>
              <span class="network-label-sizer" aria-hidden="true">DC ID</span>
            </label>
            <input
              v-model="host"
              type="text"
              class="ui-input network-input"
            />
            <label class="ui-label ui-label--secondary network-host-label">
              <span>{{ t('settings.network.port') }}</span>
              <span class="network-label-sizer" aria-hidden="true">{{ t('settings.network.password') }}</span>
            </label>
            <input
              v-model="port"
              type="number"
              min="1"
              max="65535"
              class="ui-input network-input"
            />

            <label class="ui-label ui-label--secondary">{{ t('settings.network.login') }}</label>
            <input
              v-model="username"
              type="text"
              :placeholder="t('settings.network.optional')"
              class="ui-input network-input"
            />
            <label class="ui-label ui-label--secondary">{{ t('settings.network.password') }}</label>
            <InputWithToggle
              v-model="password"
              type="password"
              :placeholder="t('settings.network.optional')"
              class="network-secret-field"
              ui
            />
          </div>

          <!-- Buttons Row -->
          <div class="button-row">
            <button
              @click="testConnection"
              :disabled="isTestingSocks5 || !hasProxyData"
              class="ui-button"
            >{{ isTestingSocks5 ? t('settings.network.testing') : t('settings.network.test') }}</button>
            <button @click="saveNetworkSettings" :disabled="isSavingNetwork" class="ui-button ui-button--primary">{{ t('settings.network.save') }}</button>
          </div>

          <!-- Test Result -->
          <TestResult :result="socks5TestResult" />
        </div>
      </section>

      <!-- MTProxy Section -->
      <section class="settings-section ui-section">
        <h2 class="ui-section-title">MTProxy</h2>

        <div class="network-form">
          <div class="network-grid">
            <label class="ui-label ui-label--secondary network-host-label">
              <span>{{ t('settings.network.host') }}</span>
              <span class="network-label-sizer" aria-hidden="true">{{ t('settings.network.login') }}</span>
              <span class="network-label-sizer" aria-hidden="true">{{ t('settings.network.secret') }}</span>
              <span class="network-label-sizer" aria-hidden="true">DC ID</span>
            </label>
            <input
              v-model="mtHost"
              type="text"
              class="ui-input network-input"
            />
            <label class="ui-label ui-label--secondary network-host-label">
              <span>{{ t('settings.network.port') }}</span>
              <span class="network-label-sizer" aria-hidden="true">{{ t('settings.network.password') }}</span>
            </label>
            <input
              v-model="mtPort"
              type="number"
              min="1"
              max="65535"
              class="ui-input network-input"
            />

            <label class="ui-label ui-label--secondary">{{ t('settings.network.secret') }}</label>
            <InputWithToggle
              v-model="mtSecret"
              type="password"
              class="network-secret-field network-secret-field--wide"
              ui
            />

            <label class="ui-label ui-label--secondary">DC ID</label>
            <select
              v-model="mtDcId"
              class="ui-select dc-id-select"
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
              class="ui-button"
            >{{ isTestingMtProxy ? t('settings.network.testing') : t('settings.network.test') }}</button>
            <button @click="saveMtProxySettings" :disabled="isSavingNetwork" class="ui-button ui-button--primary">{{ t('settings.network.save') }}</button>
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

/* Card skin stays local; section padding/rhythm come from ui-section. */
.settings-section {
  background: var(--color-bg-field);
  border: 1px solid var(--color-border);
  border-radius: 12px;
  backdrop-filter: blur(8px);
}

.settings-section:last-child {
  margin-bottom: 0;
}

.settings-section .ui-section-title {
  margin: 0 0 0.5rem;
}

.network-content {
  display: flex;
  flex-direction: column;
}

.network-form {
  display: flex;
  flex-direction: column;
  gap: var(--ui-row-gap);
  container-type: inline-size;
  container-name: network-form;
}

/* Label and field columns share one grid so groups start on the same line;
   first field column uses 48% of available field space. 8px between related fields,
   12px from label text to its field (8px gap + 4px label padding). */
.network-grid {
  display: grid;
  grid-template-columns: max-content minmax(0, 1.44fr) max-content minmax(0, 1.56fr);
  column-gap: var(--ui-field-group-gap);
  row-gap: var(--ui-row-gap);
  align-items: center;
}

.network-grid > .ui-label {
  padding-right: 4px;
}

/* Both proxy forms reserve the same translated label widths in each column. */
.network-host-label {
  display: grid;
}

.network-host-label > span {
  grid-area: 1 / 1;
}

.network-label-sizer {
  visibility: hidden;
}

/* Proxy values keep the deliberate mono presentation; ui-* never sets family. */
.network-input {
  font-family: var(--font-mono);
}

/* MTProxy-only exception: the secret field spans the field columns, but its
   intrinsic minimum must not feed into track sizing — otherwise the shared
   fr columns grow and host/port widths drift away from the SOCKS5 reference. */
.network-grid .network-secret-field--wide {
  grid-column: 2 / -1;
  min-width: 0;
}

/* Password/secret fields keep mono inside the shared ui-input frame. */
.network-secret-field :deep(.input-with-toggle-input.ui-input) {
  font-family: var(--font-mono);
}

.dc-id-select {
  max-width: 150px;
}

/* Buttons */
.button-row {
  display: flex;
  gap: var(--ui-field-group-gap);
  flex-wrap: wrap;
  align-items: center;
  justify-content: flex-end;
  padding-top: 0.5rem;
  border-top: 1px solid var(--color-border);
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

/* Only very narrow containers collapse field pairs and stack labels above fields:
   label gap 8px inside a pair,
   16px between pairs via the field bottom margin. */
@container network-form (max-width: 400px) {
  .network-grid {
    grid-template-columns: minmax(0, 1fr);
    row-gap: var(--ui-row-label-gap-stack);
  }

  .network-grid > .ui-label {
    padding-right: 0;
  }

  .network-grid > :not(.ui-label) {
    margin-bottom: var(--ui-row-label-gap-stack);
  }

  /* Compound selector beats the span rule by specificity, not source order:
     in a single-column grid `2 / -1` would open a phantom second column. */
  .network-grid .network-secret-field.network-secret-field--wide {
    grid-column: auto;
  }
}
</style>
