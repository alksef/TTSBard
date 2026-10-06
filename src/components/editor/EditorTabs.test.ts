import componentSource from './EditorTabs.vue?raw'
import { describe, expect, it } from 'vitest'
import englishCatalog from '../../../locales/en.json'

const MONO_ENABLE_KEY = 'shell.mono.enable'
const MONO_ENABLE_LABEL = (englishCatalog.messages as Record<string, string>)[MONO_ENABLE_KEY]

describe('EditorTabs mono mode toggle contracts', () => {
  it('defines the tab-mono-toggle button only when compact prop is active', () => {
    expect(componentSource).toContain('v-if="compact"')
    expect(componentSource).toContain('class="tab-mono-toggle"')
    expect(componentSource).toContain('@click="emit(\'enable-mono\')"')
    expect(componentSource).toContain(':title="t(\'shell.mono.enable\')"')
    expect(componentSource).toContain(':aria-label="t(\'shell.mono.enable\')"')
    expect(MONO_ENABLE_LABEL).toBe('Enable Mono mode')
  })

  it('declares compact prop and enable-mono emit in component interface', () => {
    expect(componentSource).toContain('compact?: boolean')
    expect(componentSource).toContain('\'enable-mono\': []')
  })

  it('provides visible keyboard focus for tab-mono-toggle in styles', () => {
    expect(componentSource).toContain('.tab-mono-toggle:focus-visible')
  })
})
