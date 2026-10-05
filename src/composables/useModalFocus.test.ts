import { describe, expect, it, vi } from 'vitest'
import {
  FOCUSABLE_SELECTOR,
  isElementFocusable,
  wrapFocusIndex,
  collectFocusable,
  handleModalKeydown,
  ModalFocusStack,
  type ModalStackEntry,
} from './useModalFocus'

// ---------------------------------------------------------------------------
// Minimal fake DOM. Only the surface the composable touches is implemented, so
// the stack lifecycle and keyboard routing can be exercised in the default
// `node` test environment without pulling in jsdom/happy-dom.
// ---------------------------------------------------------------------------

class FakeKeyboardEvent {
  key = ''
  shiftKey = false
  defaultPrevented = false
  propagationStopped = false
  immediatePropagationStopped = false

  preventDefault() {
    this.defaultPrevented = true
  }

  stopPropagation() {
    this.propagationStopped = true
  }

  stopImmediatePropagation() {
    this.propagationStopped = true
    this.immediatePropagationStopped = true
  }
}

type FakeListener = (event: FakeKeyboardEvent) => void

class FakeEventTarget {
  private capture = new Map<string, FakeListener[]>()
  private bubble = new Map<string, FakeListener[]>()

  addEventListener(type: string, listener: FakeListener, options?: boolean | { capture?: boolean }) {
    const useCapture = typeof options === 'boolean' ? options : !!options?.capture
    const map = useCapture ? this.capture : this.bubble
    const list = map.get(type) ?? []
    list.push(listener)
    map.set(type, list)
  }

  removeEventListener(type: string, listener: FakeListener, options?: boolean | { capture?: boolean }) {
    const useCapture = typeof options === 'boolean' ? options : !!options?.capture
    const map = useCapture ? this.capture : this.bubble
    const list = map.get(type)
    if (!list) return
    const index = list.indexOf(listener)
    if (index >= 0) list.splice(index, 1)
  }

  captureListeners(type: string): FakeListener[] {
    return this.capture.get(type) ?? []
  }

  bubbleListeners(type: string): FakeListener[] {
    return this.bubble.get(type) ?? []
  }
}

const VISIBLE_PARENT = {} as HTMLElement

class FakeElement extends FakeEventTarget {
  private attrs = new Map<string, string>()
  parentElement: FakeElement | null = null
  children: FakeElement[] = []
  focusables: FakeElement[] = []
  offsetParent: HTMLElement | null = VISIBLE_PARENT
  focused = false

  hasAttribute(name: string): boolean {
    return this.attrs.has(name)
  }

  getAttribute(name: string): string | null {
    return this.attrs.has(name) ? this.attrs.get(name)! : null
  }

  setAttribute(name: string, value: string): void {
    this.attrs.set(name, value)
  }

  removeAttribute(name: string): void {
    this.attrs.delete(name)
  }

  querySelectorAll(_selector: string): FakeElement[] {
    return this.focusables
  }

  getClientRects(): FakeElement[] {
    return []
  }

  focus(): void {
    this.focused = true
  }

  contains(other: unknown): boolean {
    if (this === other) return true
    return this.focusables.some((el) => el === other || el.contains(other))
  }
}

class FakeDocument extends FakeEventTarget {
  activeElement: FakeElement | null = null
  body: FakeElement = new FakeElement()
}

function makeEntry(root: FakeElement) {
  const handleKeydown = vi.fn()
  const handleBoundary = vi.fn((event: FakeKeyboardEvent) => {
    event.stopPropagation()
  })
  const entry: ModalStackEntry = {
    getRoot: () => root as unknown as HTMLElement,
    handleKeydown: handleKeydown as unknown as (event: KeyboardEvent) => void,
    handleBoundary: handleBoundary as unknown as (event: Event) => void,
  }
  return { entry, handleKeydown, handleBoundary }
}

function dispatchDocumentKeydown(doc: FakeDocument, event: FakeKeyboardEvent) {
  for (const listener of doc.captureListeners('keydown')) listener(event)
}

// ---------------------------------------------------------------------------
// Existing pure helpers
// ---------------------------------------------------------------------------

function el(attrs: Record<string, string> = {}) {
  return {
    hasAttribute(name: string) {
      return name in attrs
    },
    getAttribute(name: string) {
      return attrs[name] ?? null
    },
  }
}

describe('isElementFocusable', () => {
  it('accepts an ordinary interactive element', () => {
    expect(isElementFocusable(el())).toBe(true)
  })

  it('rejects disabled, hidden and inert controls', () => {
    expect(isElementFocusable(el({ disabled: '' }))).toBe(false)
    expect(isElementFocusable(el({ hidden: '' }))).toBe(false)
    expect(isElementFocusable(el({ inert: '' }))).toBe(false)
  })

  it('rejects elements hidden from assistive technology or the tab order', () => {
    expect(isElementFocusable(el({ 'aria-hidden': 'true' }))).toBe(false)
    expect(isElementFocusable(el({ tabindex: '-1' }))).toBe(false)
  })

  it('keeps aria-hidden=false and positive tabindex focusable', () => {
    expect(isElementFocusable(el({ 'aria-hidden': 'false' }))).toBe(true)
    expect(isElementFocusable(el({ tabindex: '0' }))).toBe(true)
  })
})

describe('wrapFocusIndex', () => {
  it('wraps forward from the last element back to the first', () => {
    expect(wrapFocusIndex(2, 3, 'forward')).toBe(0)
  })

  it('moves forward from outside/container focus to the first element', () => {
    expect(wrapFocusIndex(-1, 3, 'forward')).toBe(0)
  })

  it('lets native Tab proceed for mid-list forward moves', () => {
    expect(wrapFocusIndex(0, 3, 'forward')).toBe(-1)
    expect(wrapFocusIndex(1, 3, 'forward')).toBe(-1)
  })

  it('wraps backward from the first element to the last', () => {
    expect(wrapFocusIndex(0, 3, 'backward')).toBe(2)
  })

  it('moves backward from outside/container focus to the last element', () => {
    expect(wrapFocusIndex(-1, 3, 'backward')).toBe(2)
  })

  it('lets native Shift+Tab proceed for mid-list backward moves', () => {
    expect(wrapFocusIndex(1, 3, 'backward')).toBe(-1)
    expect(wrapFocusIndex(2, 3, 'backward')).toBe(-1)
  })

  it('handles an empty focusable set', () => {
    expect(wrapFocusIndex(-1, 0, 'forward')).toBe(-1)
    expect(wrapFocusIndex(0, 0, 'backward')).toBe(-1)
  })

  it('handles a single focusable element as a stable cycle', () => {
    expect(wrapFocusIndex(-1, 1, 'forward')).toBe(0)
    expect(wrapFocusIndex(0, 1, 'forward')).toBe(0)
    expect(wrapFocusIndex(0, 1, 'backward')).toBe(0)
  })
})

describe('FOCUSABLE_SELECTOR', () => {
  it('covers standard interactive controls', () => {
    expect(FOCUSABLE_SELECTOR).toContain('button:not([disabled])')
    expect(FOCUSABLE_SELECTOR).toContain('input:not([disabled])')
    expect(FOCUSABLE_SELECTOR).toContain('a[href]')
    expect(FOCUSABLE_SELECTOR).toContain('[tabindex]')
  })
})

// ---------------------------------------------------------------------------
// collectFocusable
// ---------------------------------------------------------------------------

describe('collectFocusable', () => {
  it('returns only visible, focusable descendants', () => {
    const container = new FakeElement()
    const focusable = new FakeElement()
    const disabled = new FakeElement()
    disabled.setAttribute('disabled', '')
    const hidden = new FakeElement()
    hidden.offsetParent = null
    container.focusables = [focusable, disabled, hidden]

    const result = collectFocusable(container as unknown as HTMLElement)
    expect(result).toEqual([focusable])
  })

  it('returns an empty list for a null container', () => {
    expect(collectFocusable(null)).toEqual([])
  })
})

// ---------------------------------------------------------------------------
// handleModalKeydown (keyboard routing)
// ---------------------------------------------------------------------------

describe('handleModalKeydown', () => {
  it('swallows Escape and invokes onEscape when allowed', () => {
    const container = new FakeElement()
    const event = new FakeKeyboardEvent()
    event.key = 'Escape'
    const onEscape = vi.fn()

    handleModalKeydown(
      event as unknown as KeyboardEvent,
      container as unknown as HTMLElement,
      null,
      () => true,
      onEscape,
    )

    expect(event.defaultPrevented).toBe(true)
    expect(event.immediatePropagationStopped).toBe(true)
    expect(onEscape).toHaveBeenCalledTimes(1)
  })

  it('still swallows Escape when closing is disabled', () => {
    const container = new FakeElement()
    const event = new FakeKeyboardEvent()
    event.key = 'Escape'
    const onEscape = vi.fn()

    handleModalKeydown(
      event as unknown as KeyboardEvent,
      container as unknown as HTMLElement,
      null,
      () => false,
      onEscape,
    )

    expect(event.defaultPrevented).toBe(true)
    expect(event.immediatePropagationStopped).toBe(true)
    expect(onEscape).not.toHaveBeenCalled()
  })

  it('does not suppress native typing for ordinary keys', () => {
    const container = new FakeElement()
    const event = new FakeKeyboardEvent()
    event.key = 'a'

    handleModalKeydown(
      event as unknown as KeyboardEvent,
      container as unknown as HTMLElement,
      null,
      () => true,
    )

    expect(event.defaultPrevented).toBe(false)
    expect(event.propagationStopped).toBe(false)
  })

  it('wraps forward from the last focusable to the first on Tab', () => {
    const container = new FakeElement()
    const first = new FakeElement()
    const last = new FakeElement()
    container.focusables = [first, last]
    const event = new FakeKeyboardEvent()
    event.key = 'Tab'

    handleModalKeydown(
      event as unknown as KeyboardEvent,
      container as unknown as HTMLElement,
      last as unknown as HTMLElement,
      () => true,
    )

    expect(event.defaultPrevented).toBe(true)
    expect(first.focused).toBe(true)
    expect(last.focused).toBe(false)
  })

  it('wraps backward from the first focusable to the last on Shift+Tab', () => {
    const container = new FakeElement()
    const first = new FakeElement()
    const last = new FakeElement()
    container.focusables = [first, last]
    const event = new FakeKeyboardEvent()
    event.key = 'Tab'
    event.shiftKey = true

    handleModalKeydown(
      event as unknown as KeyboardEvent,
      container as unknown as HTMLElement,
      first as unknown as HTMLElement,
      () => true,
    )

    expect(event.defaultPrevented).toBe(true)
    expect(last.focused).toBe(true)
  })

  it('lets native Tab proceed for mid-list moves', () => {
    const container = new FakeElement()
    const first = new FakeElement()
    const second = new FakeElement()
    container.focusables = [first, second]
    const event = new FakeKeyboardEvent()
    event.key = 'Tab'

    handleModalKeydown(
      event as unknown as KeyboardEvent,
      container as unknown as HTMLElement,
      first as unknown as HTMLElement,
      () => true,
    )

    expect(event.defaultPrevented).toBe(false)
    expect(first.focused).toBe(false)
    expect(second.focused).toBe(false)
  })

  it('focuses the container itself when nothing is focusable', () => {
    const container = new FakeElement()
    const event = new FakeKeyboardEvent()
    event.key = 'Tab'

    handleModalKeydown(
      event as unknown as KeyboardEvent,
      container as unknown as HTMLElement,
      null,
      () => true,
    )

    expect(event.defaultPrevented).toBe(true)
    expect(container.focused).toBe(true)
  })
})

// ---------------------------------------------------------------------------
// ModalFocusStack lifecycle
// ---------------------------------------------------------------------------

describe('ModalFocusStack', () => {
  function setup() {
    const doc = new FakeDocument()
    const app = new FakeElement()
    const overlayA = new FakeElement()
    const overlayB = new FakeElement()
    app.parentElement = doc.body
    overlayA.parentElement = doc.body
    overlayB.parentElement = doc.body
    doc.body.children = [app, overlayA, overlayB]
    const stack = new ModalFocusStack(doc as unknown as Document)
    return { doc, app, overlayA, overlayB, stack }
  }

  it('inerts the background and installs document/boundary listeners on first push', () => {
    const { doc, app, overlayA, overlayB, stack } = setup()
    const a = makeEntry(overlayA)

    stack.push(a.entry)

    expect(stack.size).toBe(1)
    expect(app.hasAttribute('inert')).toBe(true)
    expect(overlayB.hasAttribute('inert')).toBe(true)
    expect(overlayA.hasAttribute('inert')).toBe(false)
    expect(doc.captureListeners('keydown').length).toBe(1)
    expect(overlayA.bubbleListeners('keydown').length).toBe(1)
    expect(overlayA.bubbleListeners('keyup').length).toBe(1)
    expect(overlayA.bubbleListeners('keypress').length).toBe(1)
  })

  it('keeps only the topmost modal interactive when nested', () => {
    const { app, overlayA, overlayB, stack } = setup()
    const a = makeEntry(overlayA)
    const b = makeEntry(overlayB)

    stack.push(a.entry)
    stack.push(b.entry)

    expect(overlayB.hasAttribute('inert')).toBe(false)
    expect(overlayA.hasAttribute('inert')).toBe(true)
    expect(app.hasAttribute('inert')).toBe(true)
    expect(stack.top()).toBe(b.entry)
  })

  it('routes a document keydown only to the topmost modal', () => {
    const { doc, overlayA, overlayB, stack } = setup()
    const a = makeEntry(overlayA)
    const b = makeEntry(overlayB)
    stack.push(a.entry)
    stack.push(b.entry)

    const event = new FakeKeyboardEvent()
    event.key = 'Escape'
    dispatchDocumentKeydown(doc, event)

    expect(b.handleKeydown).toHaveBeenCalledTimes(1)
    expect(a.handleKeydown).not.toHaveBeenCalled()
  })

  it('re-activates the lower modal when the topmost closes', () => {
    const { app, overlayA, overlayB, stack } = setup()
    const a = makeEntry(overlayA)
    const b = makeEntry(overlayB)
    stack.push(a.entry)
    stack.push(b.entry)

    const wasTop = stack.remove(b.entry)

    expect(wasTop).toBe(true)
    expect(stack.size).toBe(1)
    expect(stack.top()).toBe(a.entry)
    expect(overlayA.hasAttribute('inert')).toBe(false)
    expect(app.hasAttribute('inert')).toBe(true)
  })

  it('removes boundary guards when a modal is popped', () => {
    const { overlayA, overlayB, stack } = setup()
    const a = makeEntry(overlayA)
    const b = makeEntry(overlayB)
    stack.push(a.entry)
    stack.push(b.entry)

    stack.remove(b.entry)

    expect(overlayB.bubbleListeners('keydown').length).toBe(0)
    expect(overlayB.bubbleListeners('keyup').length).toBe(0)
    expect(overlayA.bubbleListeners('keydown').length).toBe(1)
  })

  it('supports out-of-order unmount without disturbing the top modal', () => {
    const { doc, overlayA, overlayB, stack } = setup()
    const a = makeEntry(overlayA)
    const b = makeEntry(overlayB)
    stack.push(a.entry)
    stack.push(b.entry)

    const wasTop = stack.remove(a.entry)

    expect(wasTop).toBe(false)
    expect(stack.size).toBe(1)
    expect(stack.top()).toBe(b.entry)
    expect(overlayB.hasAttribute('inert')).toBe(false)
    expect(overlayA.bubbleListeners('keydown').length).toBe(0)
    expect(doc.captureListeners('keydown').length).toBe(1)
  })

  it('tears down the document listener and restores original inert state when empty', () => {
    const { doc, app, overlayA, stack } = setup()
    app.setAttribute('inert', 'custom')
    const a = makeEntry(overlayA)

    stack.push(a.entry)
    expect(app.getAttribute('inert')).toBe('')

    stack.remove(a.entry)

    expect(doc.captureListeners('keydown').length).toBe(0)
    expect(app.getAttribute('inert')).toBe('custom')
    expect(overlayA.hasAttribute('inert')).toBe(false)
  })

  it('ignores duplicate pushes of the same entry', () => {
    const { overlayA, stack } = setup()
    const a = makeEntry(overlayA)

    stack.push(a.entry)
    stack.push(a.entry)

    expect(stack.size).toBe(1)
  })
})
