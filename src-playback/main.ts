import { createApp } from 'vue'
import PlaybackControlApp from './PlaybackControlApp.vue'
import { bootstrapLocalization, i18n } from '../src/i18n'

async function start(): Promise<void> {
  await bootstrapLocalization()
  const app = createApp(PlaybackControlApp)
  app.use(i18n)
  app.mount('#app')
}

void start()
