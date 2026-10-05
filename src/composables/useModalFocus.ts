import { nextTick, onBeforeUnmount, watchEffect, type Ref } from 'vue'

export type FocusDirection = 'forward' | 'backward'

/**
 * Selector for interactive elements that participate in Tab navigation.
 * Kept as a plain, shared constant so focus trap behaviour stays consistent
 * across every modal without pulling in a framework or dependency.
 */
export const FOCUSABLE_SELECTOR = [
  'a[href]',
  'button:not([disabled])',
  'input:not([disabled]):not([type="hidden"])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[contenteditable]:not([contenteditable="false"])',
  '[tabindex]:not([tabindex="-1"])',
].join(',')

export interface UseModalFocusOptions {
  /** Whether the modal is currently open. */
  active: Ref<boolean> | (() => boolean)
  /** The focus-trapped dialog element (may be null while closed/unmounted). */
  container: Ref<HTMLElement | null>
  /** Optional explicit element to focus first. Falls back to the first focusable. */
  initialFocus?: () => HTMLElement | null
  /** Whether Escape closes the modal (calls `onEscape`). Defaults to `true`. */
  closeOnEscape?: boolean | (() => boolean)
  /** Invoked when Escape is pressed and `closeOnEscape` resolves to true. */
  onEscape?: () => void
  /** Restore focus to the element focused before opening. Defaults to `true`. */
  restoreFocus?: boolean
}

function unwrap<T>(value: Ref<T> | (() => T)): T {
  return typeof value === 'function' ? (value as () => T)() : (value as Ref<T>).value
}

/**
 * Pure visibility/focusability check that only relies on attributes, so it can
 * be unit-tested without a DOM. Layout visibility is handled separately.
 */
export function isElementFocusable(el: {
  hasAttribute(name: string): boolean
  getAttribute(name: string): string | null
}): boolean {
  if (el.hasAttribute('disabled')) return false
  if (el.hasAttribute('hidden')) return false
  if (el.hasAttribute('inert')) return false
  if (el.getAttribute('aria-hidden') === 'true') return false
  if (el.getAttribute('tabindex') === '-1') return false
  return true
}

function isHiddenFromLayout(el: HTMLElement): boolean {
  return el.offsetParent === null && el.getClientRects().length === 0
}

export function collectFocusable(container: HTMLElement | null): HTMLElement[] {
  if (!container) return []
  return Array.from(container.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter(
    (el) => isElementFocusable(el) && !isHiddenFromLayout(el),
  )
}

/**
 * Resolve which focusable index the trap should move to for a Tab keypress.
 *
 * `current` is the index of the currently focused element within the focusable
 * list, or `-1` when focus sits on the container itself, outside it, or on a
 * now-disabled/removed control. Returns the target index to focus, or `-1` to
 * let the browser's native Tab behaviour proceed (used for mid-list moves).
 */
export function wrapFocusIndex(current: number, count: number, direction: FocusDirection): number {
  if (count <= 0) return -1
  if (direction === 'forward') {
    if (current < 0 || current === count - 1) return 0
    return -1
  }
  if (current <= 0) return count - 1
  return -1
}

/**
 * Handle Escape/Tab for a single keydown that already reached the modal.
 *
 * Only Escape and Tab are intercepted here; every other key is left untouched
 * so native typing and target-level Enter handlers keep working. Escape is
 * swallowed entirely, Tab is trapped only when focus would otherwise escape.
 */
export function handleModalKeydown(
  event: KeyboardEvent,
  container: HTMLElement | null,
  activeElement: HTMLElement | null,
  shouldCloseOnEscape: () => boolean,
  onEscape?: () => void,
): void {
  if (!container) return

  if (event.key === 'Escape') {
    // Escape must never leak to background handlers (global hotkeys, popovers).
    event.preventDefault()
    event.stopImmediatePropagation()
    if (shouldCloseOnEscape()) onEscape?.()
    return
  }

  if (event.key === 'Tab') {
    const focusable = collectFocusable(container)
    if (focusable.length === 0) {
      event.preventDefault()
      container.focus()
      return
    }
    let currentIndex = -1
    if (activeElement && activeElement !== container && container.contains(activeElement)) {
      currentIndex = focusable.indexOf(activeElement)
    }
    const targetIndex = wrapFocusIndex(
      currentIndex,
      focusable.length,
      event.shiftKey ? 'backward' : 'forward',
    )
    if (targetIndex === -1) return
    event.preventDefault()
    focusable[targetIndex].focus()
  }
}

interface InertRecord {
  el: HTMLElement
  hadInert: boolean
  value: string | null
}

/** A single open modal participating in the shared focus stack. */
export interface ModalStackEntry {
  /** The dialog element that traps focus and marks the inert boundary. */
  getRoot: () => HTMLElement | null
  /** Escape/Tab routing for a keydown that bubbled to the shared capture listener. */
  handleKeydown: (event: KeyboardEvent) => void
  /** Boundary guard that stops keyboard events from reaching background handlers. */
  handleBoundary: (event: Event) => void
}

/**
 * Coordinates the small stack of concurrently open modals so that only the
 * topmost one traps focus, handles Escape/Tab and keeps the background inert.
 * Framework-free on purpose: the lifecycle can be unit-tested against a fake
 * DOM without Vue or any extra dependency.
 */
export class ModalFocusStack {
  private entries: ModalStackEntry[] = []
  private listenerInstalled = false
  private inertRecords: InertRecord[] = []
  private boundaryRoots = new Map<ModalStackEntry, HTMLElement>()

  constructor(private readonly doc: Document) {}

  get size(): number {
    return this.entries.length
  }

  top(): ModalStackEntry | undefined {
    return this.entries[this.entries.length - 1]
  }

  isTop(entry: ModalStackEntry): boolean {
    return this.top() === entry
  }

  push(entry: ModalStackEntry): void {
    if (this.entries.includes(entry)) return
    this.entries.push(entry)
    if (this.entries.length === 1) this.installDocumentListener()
    this.installBoundary(entry)
    this.recomputeInert()
  }

  /** Remove `entry`; returns true when it was the topmost modal. */
  remove(entry: ModalStackEntry): boolean {
    const index = this.entries.indexOf(entry)
    if (index === -1) return false
    const wasTop = index === this.entries.length - 1
    this.entries.splice(index, 1)
    this.removeBoundary(entry)
    this.recomputeInert()
    if (this.entries.length === 0) this.uninstallDocumentListener()
    return wasTop
  }

  private installDocumentListener(): void {
    if (this.listenerInstalled) return
    this.listenerInstalled = true
    this.doc.addEventListener('keydown', this.handleDocumentKeydown, true)
  }

  private uninstallDocumentListener(): void {
    if (!this.listenerInstalled) return
    this.listenerInstalled = false
    this.doc.removeEventListener('keydown', this.handleDocumentKeydown, true)
  }

  private handleDocumentKeydown = (event: KeyboardEvent): void => {
    const entry = this.top()
    if (entry) entry.handleKeydown(event)
  }

  private installBoundary(entry: ModalStackEntry): void {
    const root = entry.getRoot()
    if (!root) return
    root.addEventListener('keydown', entry.handleBoundary)
    root.addEventListener('keyup', entry.handleBoundary)
    root.addEventListener('keypress', entry.handleBoundary)
    this.boundaryRoots.set(entry, root)
  }

  private removeBoundary(entry: ModalStackEntry): void {
    const root = this.boundaryRoots.get(entry)
    if (!root) return
    root.removeEventListener('keydown', entry.handleBoundary)
    root.removeEventListener('keyup', entry.handleBoundary)
    root.removeEventListener('keypress', entry.handleBoundary)
    this.boundaryRoots.delete(entry)
  }

  private recomputeInert(): void {
    this.clearInert()
    const root = this.top()?.getRoot() ?? null
    if (!root) return
    const roots = this.backgroundRoots(root)
    this.inertRecords = roots.map((el) => ({
      el,
      hadInert: el.hasAttribute('inert'),
      value: el.getAttribute('inert'),
    }))
    for (const el of roots) el.setAttribute('inert', '')
  }

  private clearInert(): void {
    for (const record of this.inertRecords) {
      if (record.hadInert) {
        if (record.value === null) record.el.setAttribute('inert', '')
        else record.el.setAttribute('inert', record.value)
      } else {
        record.el.removeAttribute('inert')
      }
    }
    this.inertRecords = []
  }

  /** Everything under `<body>` except the teleported modal root becomes inert. */
  private backgroundRoots(el: HTMLElement): HTMLElement[] {
    let teleportRoot: HTMLElement | null = el
    while (
      teleportRoot
      && teleportRoot.parentElement
      && teleportRoot.parentElement !== this.doc.body
    ) {
      teleportRoot = teleportRoot.parentElement
    }
    const roots: HTMLElement[] = []
    for (const child of Array.from(this.doc.body.children)) {
      if (child !== teleportRoot) roots.push(child as HTMLElement)
    }
    return roots
  }
}

let sharedStack: ModalFocusStack | null = null

function getSharedStack(): ModalFocusStack {
  if (!sharedStack) sharedStack = new ModalFocusStack(document)
  return sharedStack
}

export function useModalFocus(options: UseModalFocusOptions) {
  const stack = getSharedStack()
  const { container, initialFocus, onEscape, restoreFocus = true } = options

  let active = false
  let previouslyFocused: HTMLElement | null = null
  let observer: MutationObserver | null = null

  const entry: ModalStackEntry = {
    getRoot: () => container.value,
    handleKeydown: onKeydown,
    handleBoundary: onBoundary,
  }

  function closeOnEscapeEnabled(): boolean {
    const value = options.closeOnEscape
    if (value === undefined) return true
    return typeof value === 'function' ? value() : value
  }

  function focusInitial() {
    if (!stack.isTop(entry)) return
    const el = container.value
    if (!el) return
    const explicit = initialFocus?.()
    if (explicit && isElementFocusable(explicit)) {
      explicit.focus()
      return
    }
    const first = collectFocusable(el)[0]
    if (first) first.focus()
    else el.focus()
  }

  /** Re-focus when dynamically replaced content leaves focus stranded. */
  function refocusIfNeeded() {
    void nextTick(() => {
      if (!active || !stack.isTop(entry)) return
      const el = container.value
      if (!el) return
      const current = document.activeElement as HTMLElement | null
      const focusable = collectFocusable(el)
      const stillFocused = !!current
        && current !== el
        && el.contains(current)
        && focusable.includes(current)
      if (!stillFocused) focusInitial()
    })
  }

  function onKeydown(event: KeyboardEvent) {
    if (!stack.isTop(entry)) return
    handleModalKeydown(
      event,
      container.value,
      document.activeElement as HTMLElement | null,
      closeOnEscapeEnabled,
      onEscape,
    )
  }

  /**
   * Dialog-boundary propagation guard. Keyboard events that originated inside
   * the dialog never bubble past this point, so document/window background
   * handlers (global hotkeys, recorder keyup listeners, etc.) stay dormant.
   * Native default actions (typing, Tab) are intentionally NOT prevented.
   */
  function onBoundary(event: Event) {
    event.stopPropagation()
  }

  function activate() {
    if (active) return
    active = true
    previouslyFocused = document.activeElement as HTMLElement | null
    const el = container.value
    if (!el) return
    stack.push(entry)
    observer = new MutationObserver(refocusIfNeeded)
    observer.observe(el, {
      childList: true,
      subtree: true,
      attributes: true,
      attributeFilter: ['disabled', 'hidden', 'inert', 'aria-hidden'],
    })
    void nextTick(focusInitial)
  }

  function deactivate() {
    if (!active) return
    active = false
    const wasTop = stack.remove(entry)
    if (observer) {
      observer.disconnect()
      observer = null
    }
    if (restoreFocus && wasTop) previouslyFocused?.focus?.()
    previouslyFocused = null
  }

  watchEffect(
    () => {
      const shouldBeActive = unwrap(options.active) && !!container.value
      if (shouldBeActive && !active) activate()
      else if (!shouldBeActive && active) deactivate()
    },
    { flush: 'post' },
  )

  onBeforeUnmount(() => deactivate())

  return { focusInitial, focusTrap: onKeydown }
}
