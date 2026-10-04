import { createApp } from 'vue'
import SettingsRecoveryApp from './SettingsRecoveryApp.vue'
import { i18n, locale, t } from '../src/i18n'
import russianCatalog from '../locales/ru.json'
import englishCatalog from '../locales/en.json'
import { invoke } from '@tauri-apps/api/core'

async function start(): Promise<void> {
  i18n.global.setLocaleMessage('ru', russianCatalog.messages)
  i18n.global.setLocaleMessage('en', englishCatalog.messages)
  let initialLanguage: 'ru' | 'en' = 'en'
  try {
    const diagnostics = await invoke<{ requestedLocale: string }>('settings_recovery_get_diagnostics')
    if (diagnostics.requestedLocale === 'ru') initialLanguage = 'ru'
  } catch {
    // Built-in English remains available when diagnostics cannot be loaded.
  }
  i18n.global.locale.value = initialLanguage
  locale.value = initialLanguage
  document.documentElement.lang = initialLanguage
  const app = createApp(SettingsRecoveryApp)
  app.use(i18n)
  app.mount('#app')

  // The window title follows the active dialog language; the static config
  // title only covers the pre-localization flash.
  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window')
    await getCurrentWindow().setTitle(t('recovery.windowTitle'))
  } catch {
    // The static configured title remains.
  }
}

void start()
