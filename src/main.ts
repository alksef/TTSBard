import { createApp } from 'vue'
import App from './App.vue'
import './style.css'

const app = createApp(App)

// Unhandled render/composite errors blank the affected subtree silently
// (e.g. a settings panel turning empty). Surface them unconditionally in the
// console so the cause is diagnosable from devtools instead of a white panel.
app.config.errorHandler = (err, _instance, info) => {
  console.error(`[Vue] Unhandled error (${info}):`, err)
}

app.mount('#app')
