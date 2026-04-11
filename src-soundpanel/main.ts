import { createApp } from 'vue'
import { listen } from '@tauri-apps/api/event'
import SoundPanelApp from './SoundPanelApp.vue'

const app = createApp(SoundPanelApp)
const instance = app.mount('#app')

// Listen for "no binding" events from backend
listen('no-binding', (event) => {
  const component = instance as { showNoBinding?: (key: string) => void }
  if (component?.showNoBinding) {
    component.showNoBinding(event.payload as string)
  }
})
