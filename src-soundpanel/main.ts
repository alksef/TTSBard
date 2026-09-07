import { createApp } from 'vue'
import { listen } from '@tauri-apps/api/event'
import SoundPanelApp from './SoundPanelApp.vue'
import { bootstrapLocalization, i18n } from '../src/i18n'

async function start(): Promise<void> {
  await bootstrapLocalization()
  const app = createApp(SoundPanelApp)
  app.use(i18n)
  const instance = app.mount('#app')

  // Listen for "no binding" events from backend only after the instance exists.
  listen('no-binding', (event) => {
    const component = instance as { showNoBinding?: (key: string) => void }
    component?.showNoBinding?.(event.payload as string)
  })
}

void start()
