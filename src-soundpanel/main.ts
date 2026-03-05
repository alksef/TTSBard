import { createApp } from 'vue'
import { listen } from '@tauri-apps/api/event'
import SoundPanelApp from './SoundPanelApp.vue'

const app = createApp(SoundPanelApp)
const instance = app.mount('#app')

// Listen for "no binding" events from backend
listen('no-binding', (event) => {
  // Access the component's exposed showNoBinding method
  const component = instance as any
  if (component && component.showNoBinding) {
    component.showNoBinding(event.payload as string)
  }
})
