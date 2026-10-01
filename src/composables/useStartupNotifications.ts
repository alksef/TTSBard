import { watch, type Ref } from 'vue'
import type { AppSettingsDto } from '../types/settings'
import { useErrorHandler } from './useErrorHandler'

export function useStartupNotifications(settings: Ref<AppSettingsDto | null>) {
  const { showWarning, showError } = useErrorHandler()
  watch(
    () => settings.value?.notifications,
    messages => messages?.forEach(message => showWarning(message, 8000)),
    { immediate: true },
  )
  watch(
    () => settings.value?.startup_errors,
    messages => messages?.forEach(message => showError(message, { duration: 8000 })),
    { immediate: true },
  )
}
