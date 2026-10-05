import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createSSRApp, ref } from 'vue'
import { renderToString } from '@vue/server-renderer'
import OcrPanel from './OcrPanel.vue'

const mocks = vi.hoisted(() => ({ useOcr: vi.fn() }))
vi.mock('../composables/useOcr', () => ({ useOcr: mocks.useOcr }))

function panelState(pending: boolean, error: boolean, missing: boolean) {
  return {
    settings: ref({ enabled: false, model_id: null, capture_target: { type: 'monitor', devicePath: 'internal-device-key' } }),
    status: ref({ state: 'disabled' }), packs: ref([]), monitors: ref([]),
    monitorListPending: ref(pending), monitorListError: ref(error),
    selectedMonitorMissing: ref(missing), message: ref(null), messageType: ref(null),
    savePending: ref(false), rescanPending: ref(false), statusLabel: ref('Disabled'),
    statusErrorMessage: ref(null), runtimeHoldsModel: ref(false), runtimeModelLabel: ref(''),
    saveSettings: vi.fn(), rescanPacks: vi.fn(), refreshMonitors: vi.fn(),
  }
}

describe('saved OCR screen presentation', () => {
  beforeEach(() => vi.clearAllMocks())

  it.each([[true, false], [false, true], [false, false]])(
    'retains a readable selected option before inventory is known (pending=%s, error=%s)',
    async (pending, error) => {
      mocks.useOcr.mockReturnValue(panelState(pending, error, false))
      const html = await renderToString(createSSRApp(OcrPanel))
      expect(html).toMatch(/<option[^>]*value="monitor:internal-device-key"[^>]*>\s*Selected screen\s*<\/option>/)
      expect(html).not.toContain('Selected screen (unavailable)')
    },
  )

  it('shows unavailable only for a confirmed missing screen without exposing its device key', async () => {
    mocks.useOcr.mockReturnValue(panelState(false, false, true))
    const html = await renderToString(createSSRApp(OcrPanel))
    expect(html).toContain('Selected screen (unavailable)')
    const optionText = [...html.matchAll(/<option[^>]*>([^<]*)<\/option>/g)].map((match) => match[1]).join(' ')
    expect(optionText).not.toContain('internal-device-key')
  })
})
