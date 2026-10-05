import { describe, it, expect } from 'vitest'
import { resolveInterceptKey, formatInterceptKey, type InterceptKeyEvent } from './interceptKeys'

function ev(partial: Partial<InterceptKeyEvent>): InterceptKeyEvent {
  return { key: '', code: '', keyCode: 0, ...partial }
}

describe('resolveInterceptKey — restricted mode', () => {
  it('rejects ordinary letters', () => {
    expect(resolveInterceptKey(ev({ key: 'a', code: 'KeyA', keyCode: 65 }), false)).toBeNull()
    expect(resolveInterceptKey(ev({ key: 'ф', code: 'KeyA', keyCode: 65 }), false)).toBeNull()
  })

  it('rejects Delete and other non-restricted keys', () => {
    expect(resolveInterceptKey(ev({ key: 'Delete', code: 'Delete', keyCode: 46 }), false)).toBeNull()
    expect(resolveInterceptKey(ev({ key: ' ', code: 'Space', keyCode: 32 }), false)).toBeNull()
    expect(resolveInterceptKey(ev({ key: 'Shift', code: 'ShiftLeft', keyCode: 16 }), false)).toBeNull()
  })

  it('accepts Home and Insert', () => {
    expect(resolveInterceptKey(ev({ key: 'Home', code: 'Home', keyCode: 36 }), false)).toBe('HOME')
    expect(resolveInterceptKey(ev({ key: 'Insert', code: 'Insert', keyCode: 45 }), false)).toBe('INSERT')
  })

  it('preserves legacy NumPad names including with NumLock-off navigation keys', () => {
    expect(resolveInterceptKey(ev({ key: '1', code: 'Numpad1', keyCode: 97 }), false)).toBe('NUMPAD1')
    // NumLock off: the same key reports a navigation `key` but still maps by code.
    expect(resolveInterceptKey(ev({ key: 'End', code: 'Numpad1', keyCode: 35 }), false)).toBe('NUMPAD1')
    expect(resolveInterceptKey(ev({ key: '*', code: 'NumpadMultiply', keyCode: 106 }), false)).toBe('NUMPAD_MULTIPLY')
    expect(resolveInterceptKey(ev({ key: '+', code: 'NumpadAdd', keyCode: 107 }), false)).toBe('NUMPAD_ADD')
    expect(resolveInterceptKey(ev({ key: '/', code: 'NumpadDivide', keyCode: 111 }), false)).toBe('NUMPAD_DIVIDE')
  })

  it('preserves F-keys and navigation keys', () => {
    expect(resolveInterceptKey(ev({ key: 'F5', code: 'F5', keyCode: 116 }), false)).toBe('F5')
    expect(resolveInterceptKey(ev({ key: 'F24', code: 'F24', keyCode: 135 }), false)).toBe('F24')
    expect(resolveInterceptKey(ev({ key: 'PageUp', code: 'PageUp', keyCode: 33 }), false)).toBe('PAGEUP')
    expect(resolveInterceptKey(ev({ key: 'End', code: 'End', keyCode: 35 }), false)).toBe('END')
    expect(resolveInterceptKey(ev({ key: 'PageDown', code: 'PageDown', keyCode: 34 }), false)).toBe('PAGEDOWN')
  })
})

describe('resolveInterceptKey — unrestricted mode', () => {
  it('maps letters independently of the active key text/layout', () => {
    expect(resolveInterceptKey(ev({ key: 'a', code: 'KeyA', keyCode: 65 }), true)).toBe('VK_41')
    expect(resolveInterceptKey(ev({ key: 'ф', code: 'KeyA', keyCode: 65 }), true)).toBe('VK_41')
  })

  it('maps OEM keys across key text/layout', () => {
    expect(resolveInterceptKey(ev({ key: '=', code: 'Equal', keyCode: 0xbb }), true)).toBe('VK_BB')
    expect(resolveInterceptKey(ev({ key: ';', code: 'Semicolon', keyCode: 0xba }), true)).toBe('VK_BA')
  })

  it('maps sided modifiers via physical code instead of generic keyCode', () => {
    expect(resolveInterceptKey(ev({ key: 'Shift', code: 'ShiftLeft', keyCode: 16 }), true)).toBe('VK_A0')
    expect(resolveInterceptKey(ev({ key: 'Shift', code: 'ShiftRight', keyCode: 16 }), true)).toBe('VK_A1')
    expect(resolveInterceptKey(ev({ key: 'Control', code: 'ControlLeft', keyCode: 17 }), true)).toBe('VK_A2')
    expect(resolveInterceptKey(ev({ key: 'Control', code: 'ControlRight', keyCode: 17 }), true)).toBe('VK_A3')
    expect(resolveInterceptKey(ev({ key: 'Alt', code: 'AltLeft', keyCode: 18 }), true)).toBe('VK_A4')
    expect(resolveInterceptKey(ev({ key: 'Alt', code: 'AltRight', keyCode: 18 }), true)).toBe('VK_A5')
    expect(resolveInterceptKey(ev({ key: 'Meta', code: 'MetaLeft', keyCode: 91 }), true)).toBe('VK_5B')
    expect(resolveInterceptKey(ev({ key: 'Meta', code: 'MetaRight', keyCode: 91 }), true)).toBe('VK_5C')
  })

  it('allows binding Escape in unrestricted mode', () => {
    expect(resolveInterceptKey(ev({ key: 'Escape', code: 'Escape', keyCode: 27 }), true)).toBe('VK_1B')
  })

  it('maps NumpadEnter through the VK fallback without inventing an identifier', () => {
    expect(resolveInterceptKey(ev({ key: 'Enter', code: 'NumpadEnter', keyCode: 13 }), true)).toBe('VK_0D')
  })

  it('maps top-row digits, media and browser keys', () => {
    expect(resolveInterceptKey(ev({ key: '1', code: 'Digit1', keyCode: 49 }), true)).toBe('VK_31')
    expect(resolveInterceptKey(ev({ key: 'MediaPlayPause', code: 'MediaPlayPause', keyCode: 0xb3 }), true)).toBe('VK_B3')
    expect(resolveInterceptKey(ev({ key: 'BrowserBack', code: 'BrowserBack', keyCode: 0xa6 }), true)).toBe('VK_A6')
  })

  it('reuses restricted canonical names when the VK represents a restricted key', () => {
    // keyCode still identifies Home even though the code is not the DOM name.
    expect(resolveInterceptKey(ev({ key: 'Home', code: 'Home', keyCode: 0x24 }), true)).toBe('HOME')
    expect(resolveInterceptKey(ev({ key: 'Insert', code: 'Insert', keyCode: 0x2d }), true)).toBe('INSERT')
  })

  it('falls back to physical code when keyCode is zero', () => {
    expect(resolveInterceptKey(ev({ key: 'a', code: 'KeyA', keyCode: 0 }), true)).toBe('VK_41')
    expect(resolveInterceptKey(ev({ key: 'Delete', code: 'Delete', keyCode: 0 }), true)).toBe('VK_2E')
    expect(resolveInterceptKey(ev({ key: 'F5', code: 'F5', keyCode: 0 }), true)).toBe('F5')
  })

  it('returns null for unmappable, zero and out-of-range VKs', () => {
    expect(resolveInterceptKey(ev({ key: '', code: 'NumpadEqual', keyCode: 0 }), true)).toBeNull()
    expect(resolveInterceptKey(ev({ key: '', code: '', keyCode: 0 }), true)).toBeNull()
    expect(resolveInterceptKey(ev({ key: '', code: 'UnknownCode', keyCode: 0x07 }), true)).toBeNull()
    expect(resolveInterceptKey(ev({ key: '', code: 'UnknownCode', keyCode: 0xff }), true)).toBeNull()
    expect(resolveInterceptKey(ev({ key: '', code: 'UnknownCode', keyCode: 0x100 }), true)).toBeNull()
  })
})

describe('formatInterceptKey', () => {
  it('preserves readable restricted names', () => {
    expect(formatInterceptKey('NUMPAD1')).toBe('NUMPAD1')
    expect(formatInterceptKey('F5')).toBe('F5')
    expect(formatInterceptKey('HOME')).toBe('HOME')
    expect(formatInterceptKey('INSERT')).toBe('INSERT')
  })

  it('resolves known VK codes to readable labels', () => {
    expect(formatInterceptKey('VK_41')).toBe('A')
    expect(formatInterceptKey('VK_20')).toBe('Space')
    expect(formatInterceptKey('VK_2E')).toBe('Delete')
    expect(formatInterceptKey('VK_A0')).toBe('Shift (Left)')
    expect(formatInterceptKey('VK_5B')).toBe('Win (Left)')
  })

  it('displays the raw code for unknown VK values', () => {
    expect(formatInterceptKey('VK_BA')).toBe('VK_BA')
    expect(formatInterceptKey('VK_FE')).toBe('VK_FE')
  })
})
