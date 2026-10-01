import { effectScope, nextTick, ref } from 'vue'
import { afterEach, expect, it, vi } from 'vitest'
import type { AppSettingsDto } from '../types/settings'
import { ErrorLevel, useErrorHandler } from './useErrorHandler'
import { useStartupNotifications } from './useStartupNotifications'

vi.mock('../utils/debug', () => ({
  debugLog: vi.fn(), debugError: vi.fn(), debugWarn: vi.fn(), debugInfo: vi.fn(),
}))

afterEach(() => useErrorHandler().clearAllErrors())

it('delivers startup errors globally while retaining warning severity across refreshes', async () => {
  const scope = effectScope()
  const settings = ref<AppSettingsDto | null>(null)
  scope.run(() => useStartupNotifications(settings))
  settings.value = { notifications: ['warning'], startup_errors: ['Piper unavailable'] } as AppSettingsDto
  await nextTick()
  expect(useErrorHandler().errors.value.map(({ message, level }) => ({ message, level }))).toEqual([
    { message: 'warning', level: ErrorLevel.WARNING },
    { message: 'Piper unavailable', level: ErrorLevel.ERROR },
  ])
  // Backend consumes both queues on first read; later snapshots omit them.
  settings.value = {} as AppSettingsDto
  await nextTick()
  expect(useErrorHandler().errors.value).toHaveLength(2)
  scope.stop()
})
