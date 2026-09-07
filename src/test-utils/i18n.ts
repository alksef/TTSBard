import { i18n } from '../i18n'

/**
 * Switch the active locale for the duration of `fn`, then restore it.
 *
 * Supports both synchronous and asynchronous `fn`. The returned promise is
 * resolved only when `fn` itself returns a promise, so synchronous call-sites
 * may keep calling it without `await`.
 */
export function withLocale(
  locale: 'en' | 'ru',
  fn: () => void | Promise<void>,
): void | Promise<void> {
  const previous = (i18n.global.locale as unknown as { value: string }).value
  ;(i18n.global.locale as unknown as { value: string }).value = locale
  try {
    const result = fn()
    if (result instanceof Promise) {
      return result.finally(() => {
        ;(i18n.global.locale as unknown as { value: string }).value = previous
      })
    }
  } catch (error) {
    ;(i18n.global.locale as unknown as { value: string }).value = previous
    throw error
  }
  ;(i18n.global.locale as unknown as { value: string }).value = previous
}
