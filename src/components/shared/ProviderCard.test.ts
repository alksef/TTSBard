import { describe, expect, it } from 'vitest'

import { restoreProviderRadioState } from './providerCardState'

describe('ProviderCard radio state', () => {
  it('restores an inactive provider after the browser checks its radio', () => {
    const radio = { checked: true }

    restoreProviderRadioState(radio, false)

    expect(radio.checked).toBe(false)
  })

  it('keeps an active provider checked', () => {
    const radio = { checked: false }

    restoreProviderRadioState(radio, true)

    expect(radio.checked).toBe(true)
  })
})
