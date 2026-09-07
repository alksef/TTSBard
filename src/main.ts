import { createApp } from 'vue'
import App from './App.vue'
import { bootstrapLocalization, i18n } from './i18n'
import './style.css'

async function start(): Promise<void> {
  await bootstrapLocalization()
  const app = createApp(App)
  app.use(i18n)

  // Unhandled render/composite errors blank the affected subtree silently
  // (e.g. a settings panel turning empty). Surface them unconditionally in the
  // console so the cause is diagnosable from devtools instead of a white panel.
  app.config.errorHandler = (err, _instance, info) => {
    console.error(`[Vue] Unhandled error (${info}):`, err)
  }
  app.mount('#app')
}

void start()
