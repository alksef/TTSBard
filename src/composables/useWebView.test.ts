import { describe, it, expect, vi, beforeEach } from 'vitest'
import { shallowRef, nextTick } from 'vue'
import type { WebViewSettingsDto } from '../types/settings'

vi.stubGlobal('window', globalThis)

const {
  mockInvoke,
  listenMock,
} = vi.hoisted(() => ({
  mockInvoke: vi.fn(),
  listenMock: vi.fn(async () => vi.fn()),
}))

let capturedOnMountedCb: (() => void) | null = null

vi.mock('vue', async () => {
  const actual = await vi.importActual<typeof import('vue')>('vue')
  return {
    ...actual,
    onMounted: (cb: () => void) => { capturedOnMountedCb = cb },
    onUnmounted: () => {},
  }
})

vi.mock('@tauri-apps/api/core', () => ({
  invoke: mockInvoke,
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: listenMock,
}))

vi.mock('@tauri-apps/plugin-dialog', () => ({
  confirm: vi.fn(async () => true),
}))

vi.mock('../utils/debug', () => ({
  debugLog: vi.fn(),
  debugError: vi.fn(),
}))

let mockWebViewSettingsRef = shallowRef<WebViewSettingsDto | undefined>(undefined)
vi.mock('./useAppSettings', () => ({
  useWebViewSettings: vi.fn(() => mockWebViewSettingsRef),
}))

import { useWebView } from './useWebView'

function makeSettings(overrides: Partial<WebViewSettingsDto> = {}): WebViewSettingsDto {
  return {
    enabled: false,
    start_on_boot: false,
    port: 10100,
    bind_address: '0.0.0.0',
    access_token: null,
    upnp_enabled: false,
    ...overrides,
  }
}

async function setupAndMount() {
  mockInvoke.mockImplementation(async (cmd: string) => {
    if (cmd === 'get_webview_token') return null
    return undefined
  })
  const composable = useWebView()
  if (capturedOnMountedCb) {
    await capturedOnMountedCb()
  }
  return composable
}

describe('useWebView displayUrl', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    capturedOnMountedCb = null
  })

  it('uses 127.0.0.1 and the configured port when bind is 0.0.0.0', async () => {
    mockWebViewSettingsRef.value = makeSettings({ bind_address: '0.0.0.0', port: 10100 })
    const { displayUrl, updateDisplayUrl } = await setupAndMount()
    await nextTick()
    updateDisplayUrl()
    expect(displayUrl.value).toBe('http://127.0.0.1:10100')
  })

  it('uses 127.0.0.1 and the configured port when bind is 127.0.0.1', async () => {
    mockWebViewSettingsRef.value = makeSettings({ bind_address: '127.0.0.1', port: 8080 })
    const { displayUrl, updateDisplayUrl } = await setupAndMount()
    await nextTick()
    updateDisplayUrl()
    expect(displayUrl.value).toBe('http://127.0.0.1:8080')
  })

  it('does not call get_local_ip on mount', async () => {
    mockWebViewSettingsRef.value = makeSettings({ bind_address: '0.0.0.0' })
    await setupAndMount()
    await nextTick()
    expect(mockInvoke).not.toHaveBeenCalledWith('get_local_ip')
  })
})
