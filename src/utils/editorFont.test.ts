import { describe, it, expect } from 'vitest'
import {
  EDITOR_FONT_SIZE_DEFAULT,
  EDITOR_FONT_SIZE_MAX,
  EDITOR_FONT_SIZE_MIN,
  editorFontCssStack,
  editorFontLabel,
  isEditorFontFamily,
  parseEditorFontSize,
  toEditorFontFamily,
} from './editorFont'
import type { EditorFontFamily } from '../types/settings'

describe('editorFont', () => {
  describe('isEditorFontFamily', () => {
    it('accepts the stable ids and any non-empty family name', () => {
      const ids: EditorFontFamily[] = ['default', 'system', 'arial', 'georgia', 'consolas']
      for (const id of ids) {
        expect(isEditorFontFamily(id)).toBe(true)
      }
      expect(isEditorFontFamily('PT Sans')).toBe(true)
      expect(isEditorFontFamily('Times New Roman')).toBe(true)
    })

    it('rejects blank and non-string values', () => {
      expect(isEditorFontFamily('')).toBe(false)
      expect(isEditorFontFamily('   ')).toBe(false)
      expect(isEditorFontFamily(null)).toBe(false)
      expect(isEditorFontFamily(undefined)).toBe(false)
      expect(isEditorFontFamily(42)).toBe(false)
    })
  })

  describe('toEditorFontFamily', () => {
    it('passes a valid family through, trimmed', () => {
      expect(toEditorFontFamily('georgia')).toBe('georgia')
      expect(toEditorFontFamily('  PT Sans ')).toBe('PT Sans')
    })

    it('falls back to default for invalid input', () => {
      expect(toEditorFontFamily('')).toBe('default')
      expect(toEditorFontFamily('   ')).toBe('default')
      expect(toEditorFontFamily(undefined)).toBe('default')
      expect(toEditorFontFamily(null)).toBe('default')
      expect(toEditorFontFamily(123)).toBe('default')
    })
  })

  describe('editorFontLabel', () => {
    it('maps each built-in id to its label', () => {
      expect(editorFontLabel('default')).toBe('По умолчанию')
      expect(editorFontLabel('system')).toBe('Системный')
      expect(editorFontLabel('arial')).toBe('Arial')
      expect(editorFontLabel('georgia')).toBe('Georgia')
      expect(editorFontLabel('consolas')).toBe('Consolas')
    })

    it('shows an arbitrary family name as its own label', () => {
      expect(editorFontLabel('PT Sans')).toBe('PT Sans')
    })
  })

  describe('editorFontCssStack', () => {
    it('maps default to the current mono chain', () => {
      expect(editorFontCssStack('default')).toBe('var(--font-mono)')
    })

    it('maps each built-in family to its CSS stack', () => {
      expect(editorFontCssStack('system')).toBe("'Segoe UI', sans-serif")
      expect(editorFontCssStack('arial')).toBe('Arial, sans-serif')
      expect(editorFontCssStack('georgia')).toBe('Georgia, serif')
      expect(editorFontCssStack('consolas')).toBe('Consolas, monospace')
    })

    it('quotes an arbitrary family and falls back to the editor font', () => {
      expect(editorFontCssStack('PT Sans')).toBe("'PT Sans', var(--font-mono)")
    })

    it('escapes quotes and backslashes inside an arbitrary family', () => {
      expect(editorFontCssStack("Murphy's")).toBe("'Murphy\\'s', var(--font-mono)")
      expect(editorFontCssStack('Back\\Slash')).toBe("'Back\\\\Slash', var(--font-mono)")
    })
  })

  describe('parseEditorFontSize', () => {
    it('accepts every value in the 12..32 range', () => {
      for (let n = EDITOR_FONT_SIZE_MIN; n <= EDITOR_FONT_SIZE_MAX; n++) {
        expect(parseEditorFontSize(n)).toBe(n)
      }
    })

    it('accepts numeric strings in range', () => {
      expect(parseEditorFontSize('16')).toBe(16)
      expect(parseEditorFontSize(' 24 ')).toBe(24)
      expect(parseEditorFontSize('12')).toBe(12)
      expect(parseEditorFontSize('32')).toBe(32)
    })

    it('rejects empty values', () => {
      expect(parseEditorFontSize('')).toBeNull()
      expect(parseEditorFontSize('   ')).toBeNull()
      expect(parseEditorFontSize(null)).toBeNull()
      expect(parseEditorFontSize(undefined)).toBeNull()
    })

    it('rejects fractional values', () => {
      expect(parseEditorFontSize(12.5)).toBeNull()
      expect(parseEditorFontSize('16.5')).toBeNull()
      expect(parseEditorFontSize(0.5)).toBeNull()
    })

    it('rejects out-of-range values', () => {
      expect(parseEditorFontSize(EDITOR_FONT_SIZE_MIN - 1)).toBeNull()
      expect(parseEditorFontSize(EDITOR_FONT_SIZE_MAX + 1)).toBeNull()
      expect(parseEditorFontSize(0)).toBeNull()
      expect(parseEditorFontSize(99)).toBeNull()
      expect(parseEditorFontSize(-5)).toBeNull()
    })

    it('rejects non-numeric values', () => {
      expect(parseEditorFontSize('abc')).toBeNull()
      expect(parseEditorFontSize(NaN)).toBeNull()
      expect(parseEditorFontSize(Infinity)).toBeNull()
    })

    it('does not clamp out-of-range values into range', () => {
      expect(parseEditorFontSize(EDITOR_FONT_SIZE_MAX + 1)).toBeNull()
      expect(parseEditorFontSize(EDITOR_FONT_SIZE_MIN - 1)).toBeNull()
    })

    it('exposes the default constant', () => {
      expect(EDITOR_FONT_SIZE_DEFAULT).toBe(16)
    })
  })
})
