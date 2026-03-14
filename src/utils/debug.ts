/**
 * Debug utilities
 */

export const DEBUG = import.meta.env.DEV

/**
 * Conditional debug logging - only logs in development mode
 */
export function debugLog(...args: unknown[]): void {
  if (DEBUG) {
    console.log(...args)
  }
}

/**
 * Conditional debug error logging - always logs errors
 */
export function debugError(...args: unknown[]): void {
  console.error(...args)
}

/**
 * Conditional debug warning logging - only logs in development mode
 */
export function debugWarn(...args: unknown[]): void {
  if (DEBUG) {
    console.warn(...args)
  }
}

/**
 * Conditional debug info logging - only logs in development mode
 */
export function debugInfo(...args: unknown[]): void {
  if (DEBUG) {
    console.info(...args)
  }
}
