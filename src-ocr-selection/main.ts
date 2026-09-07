import { createApp } from 'vue'
import SelectionApp from './SelectionApp.vue'
import { bootstrapLocalization, i18n } from '../src/i18n'

async function start(): Promise<void> {
  await bootstrapLocalization()
  const app = createApp(SelectionApp)
  app.use(i18n)
  app.mount('#app')
}

void start()
