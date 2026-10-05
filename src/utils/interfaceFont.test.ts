import { describe, it, expect } from 'vitest'
import {
  INTERFACE_FONT_SIZE_DEFAULT,
  INTERFACE_FONT_SIZE_MAX,
  INTERFACE_FONT_SIZE_MIN,
  applyInterfaceFontToRoot,
  interfaceFontCssStack,
  interfaceFontLabel,
  isInterfaceFontFamily,
  parseInterfaceFontSize,
  toInterfaceFontFamily,
} from './interfaceFont'
import type { InterfaceFontFamily } from '../types/settings'

describe('interfaceFont', () => {
  describe('isInterfaceFontFamily', () => {
    it('accepts the stable ids and any non-empty family name', () => {
      const ids: InterfaceFontFamily[] = ['default', 'system', 'Segoe UI']
      for (const id of ids) {
        expect(isInterfaceFontFamily(id)).toBe(true)
      }
      expect(isInterfaceFontFamily('PT Sans')).toBe(true)
      expect(isInterfaceFontFamily('Times New Roman')).toBe(true)
    })

    it('rejects blank and non-string values', () => {
      expect(isInterfaceFontFamily('')).toBe(false)
      expect(isInterfaceFontFamily('   ')).toBe(false)
      expect(isInterfaceFontFamily(null)).toBe(false)
      expect(isInterfaceFontFamily(undefined)).toBe(false)
      expect(isInterfaceFontFamily(42)).toBe(false)
    })
  })

  describe('toInterfaceFontFamily', () => {
    it('passes a valid family through, trimmed', () => {
      expect(toInterfaceFontFamily('system')).toBe('system')
      expect(toInterfaceFontFamily('  PT Sans ')).toBe('PT Sans')
    })

    it('falls back to default for invalid input', () => {
      expect(toInterfaceFontFamily('')).toBe('default')
      expect(toInterfaceFontFamily('   ')).toBe('default')
      expect(toInterfaceFontFamily(undefined)).toBe('default')
      expect(toInterfaceFontFamily(null)).toBe('default')
      expect(toInterfaceFontFamily(123)).toBe('default')
    })
  })

  describe('interfaceFontLabel', () => {
    it('maps each built-in id to its label', () => {
      expect(interfaceFontLabel('default')).toBe('По умолчанию')
      expect(interfaceFontLabel('system')).toBe('Системный')
    })

    it('shows an arbitrary family name as its own label', () => {
      expect(interfaceFontLabel('PT Sans')).toBe('PT Sans')
    })
  })

  describe('interfaceFontCssStack', () => {
    it('maps default to the current interface stack', () => {
      expect(interfaceFontCssStack('default')).toBe('var(--font-sans)')
    })

    it('maps the system family to the Windows UI stack', () => {
      expect(interfaceFontCssStack('system')).toBe("'Segoe UI', sans-serif")
    })

    it('quotes an arbitrary family and falls back to the interface stack', () => {
      expect(interfaceFontCssStack('PT Sans')).toBe("'PT Sans', var(--font-sans)")
    })

    it('escapes quotes and backslashes inside an arbitrary family', () => {
      expect(interfaceFontCssStack("Murphy's")).toBe("'Murphy\\'s', var(--font-sans)")
      expect(interfaceFontCssStack('Back\\Slash')).toBe("'Back\\\\Slash', var(--font-sans)")
    })
  })

  describe('parseInterfaceFontSize', () => {
    it('accepts every value in the 14..20 range', () => {
      for (let n = INTERFACE_FONT_SIZE_MIN; n <= INTERFACE_FONT_SIZE_MAX; n++) {
        expect(parseInterfaceFontSize(n)).toBe(n)
      }
    })

    it('accepts numeric strings in range', () => {
      expect(parseInterfaceFontSize('16')).toBe(16)
      expect(parseInterfaceFontSize(' 18 ')).toBe(18)
      expect(parseInterfaceFontSize('14')).toBe(14)
      expect(parseInterfaceFontSize('20')).toBe(20)
    })

    it('rejects empty values', () => {
      expect(parseInterfaceFontSize('')).toBeNull()
      expect(parseInterfaceFontSize('   ')).toBeNull()
      expect(parseInterfaceFontSize(null)).toBeNull()
      expect(parseInterfaceFontSize(undefined)).toBeNull()
    })

    it('rejects fractional values', () => {
      expect(parseInterfaceFontSize(15.5)).toBeNull()
      expect(parseInterfaceFontSize('16.5')).toBeNull()
      expect(parseInterfaceFontSize(0.5)).toBeNull()
    })

    it('rejects out-of-range values', () => {
      expect(parseInterfaceFontSize(INTERFACE_FONT_SIZE_MIN - 1)).toBeNull()
      expect(parseInterfaceFontSize(INTERFACE_FONT_SIZE_MAX + 1)).toBeNull()
      expect(parseInterfaceFontSize(0)).toBeNull()
      expect(parseInterfaceFontSize(99)).toBeNull()
      expect(parseInterfaceFontSize(-5)).toBeNull()
    })

    it('rejects non-numeric values', () => {
      expect(parseInterfaceFontSize('abc')).toBeNull()
      expect(parseInterfaceFontSize(NaN)).toBeNull()
      expect(parseInterfaceFontSize(Infinity)).toBeNull()
    })

    it('does not clamp out-of-range values into range', () => {
      expect(parseInterfaceFontSize(INTERFACE_FONT_SIZE_MAX + 1)).toBeNull()
      expect(parseInterfaceFontSize(INTERFACE_FONT_SIZE_MIN - 1)).toBeNull()
    })

    it('exposes the default constant', () => {
      expect(INTERFACE_FONT_SIZE_DEFAULT).toBe(16)
    })
  })
})

describe('applyInterfaceFontToRoot', () => {
  function makeStyle(): CSSStyleDeclaration & { props: Map<string, string> } {
    const props = new Map<string, string>()
    const style = {
      props,
      setProperty: (k: string, v: string) => { props.set(k, v) },
      removeProperty: (k: string) => { props.delete(k) },
    } as unknown as CSSStyleDeclaration & { props: Map<string, string> }
    return style
  }

  it('sets custom properties for a non-default family and size', () => {
    const style = makeStyle()
    applyInterfaceFontToRoot(style, 'Segoe UI', 18)
    expect(style.props.get('--ui-font-family')).toBe(interfaceFontCssStack('Segoe UI'))
    expect(style.props.get('--ui-font-size')).toBe('18px')
  })

  it('removes both properties at defaults (reset and launch restore)', () => {
    const style = makeStyle()
    style.props.set('--ui-font-family', "'X', var(--font-sans)")
    style.props.set('--ui-font-size', '20px')
    applyInterfaceFontToRoot(style, 'default', 16)
    expect(style.props.has('--ui-font-family')).toBe(false)
    expect(style.props.has('--ui-font-size')).toBe(false)
  })

  it('falls back to defaults on invalid values', () => {
    const style = makeStyle()
    applyInterfaceFontToRoot(style, '', 999)
    expect(style.props.has('--ui-font-family')).toBe(false)
    expect(style.props.has('--ui-font-size')).toBe(false)
  })
})
