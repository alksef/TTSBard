import { describe, it, expect } from 'vitest'
import {
  enterOutcomeLabel,
  enterOutcomeLabelCompact,
  nextQuickMode,
  resolveKeepText,
  submitActionState,
} from './submitAffordance'
import type { SubmitKeepIntent } from './submitAffordance'
import type { QuickEditorMode } from '../../types/settings'

const modes: QuickEditorMode[] = ['disabled', 'collapse', 'return_focus']

describe('nextQuickMode', () => {
  it('cycles disabled → collapse → return_focus → disabled', () => {
    expect(nextQuickMode('disabled')).toBe('collapse')
    expect(nextQuickMode('collapse')).toBe('return_focus')
    expect(nextQuickMode('return_focus')).toBe('disabled')
  })

  it('returns a mode after every step of the full cycle', () => {
    let mode: QuickEditorMode = 'disabled'
    const seen = new Set<QuickEditorMode>()
    for (let i = 0; i < modes.length; i++) {
      seen.add(mode)
      mode = nextQuickMode(mode)
    }
    expect(seen.size).toBe(modes.length)
    for (const m of modes) {
      expect(seen.has(m)).toBe(true)
    }
  })
})

describe('enterOutcomeLabel', () => {
  it('labels disabled mode', () => {
    expect(enterOutcomeLabel('disabled')).toBe('остаться')
  })

  it('labels collapse mode', () => {
    expect(enterOutcomeLabel('collapse')).toBe('скрыть окно')
  })

  it('labels return_focus mode', () => {
    expect(enterOutcomeLabel('return_focus')).toBe('вернуть фокус')
  })

  it('covers every QuickEditorMode', () => {
    for (const mode of modes) {
      expect(typeof enterOutcomeLabel(mode)).toBe('string')
    }
  })
})

describe('enterOutcomeLabelCompact', () => {
  it('labels disabled mode', () => {
    expect(enterOutcomeLabelCompact('disabled')).toBe('остаться')
  })

  it('labels collapse mode', () => {
    expect(enterOutcomeLabelCompact('collapse')).toBe('скрыть')
  })

  it('labels return_focus mode', () => {
    expect(enterOutcomeLabelCompact('return_focus')).toBe('вернуть фокус')
  })

  it('covers every QuickEditorMode', () => {
    for (const mode of modes) {
      expect(typeof enterOutcomeLabelCompact(mode)).toBe('string')
    }
  })
})

describe('submitActionState', () => {
  it('is idle when nothing happened', () => {
    expect(submitActionState(false, 'none')).toBe('idle')
  })

  it('reports accepted outcome', () => {
    expect(submitActionState(false, 'accepted')).toBe('accepted')
  })

  it('reports error outcome', () => {
    expect(submitActionState(false, 'error')).toBe('error')
  })

  it('prioritizes submitting over a previous accepted outcome', () => {
    expect(submitActionState(true, 'accepted')).toBe('submitting')
  })

  it('prioritizes submitting over a previous error outcome', () => {
    expect(submitActionState(true, 'error')).toBe('submitting')
  })

  it('prioritizes submitting over no previous outcome', () => {
    expect(submitActionState(true, 'none')).toBe('submitting')
  })
})

describe('resolveKeepText', () => {
  const intents: SubmitKeepIntent[] = ['quick', 'continue', 'keep_text', 'keep_focus']

  it('covers the full intent × setting matrix', () => {
    for (const setting of [false, true]) {
      for (const intent of intents) {
        const d = resolveKeepText(setting, intent)
        expect(typeof d.keepText).toBe('boolean')
        expect(typeof d.applyQuickPolicy).toBe('boolean')
      }
    }
  })

  it('quick applies the quick policy and clears per setting', () => {
    expect(resolveKeepText(false, 'quick')).toEqual({ keepText: false, applyQuickPolicy: true })
    expect(resolveKeepText(true, 'quick')).toEqual({ keepText: true, applyQuickPolicy: true })
  })

  it('continue stays in the editor and clears per setting', () => {
    expect(resolveKeepText(false, 'continue')).toEqual({ keepText: false, applyQuickPolicy: false })
    expect(resolveKeepText(true, 'continue')).toEqual({ keepText: true, applyQuickPolicy: false })
  })

  it('keep_text inverts the setting and applies the quick policy', () => {
    expect(resolveKeepText(false, 'keep_text')).toEqual({ keepText: true, applyQuickPolicy: true })
    expect(resolveKeepText(true, 'keep_text')).toEqual({ keepText: false, applyQuickPolicy: true })
  })

  it('keep_focus always keeps text and never applies the quick policy', () => {
    expect(resolveKeepText(false, 'keep_focus')).toEqual({ keepText: true, applyQuickPolicy: false })
    expect(resolveKeepText(true, 'keep_focus')).toEqual({ keepText: true, applyQuickPolicy: false })
  })
})
