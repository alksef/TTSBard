import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = resolve(fileURLToPath(new URL('..', import.meta.url)))

export function localeParity(rootDir = root) {
  const readMessages = (path) => JSON.parse(readFileSync(resolve(rootDir, path), 'utf8')).messages
  const webEn = readMessages('locales/en.json')
  const rustEn = readMessages('src-tauri/locales/en.json')
  const webRu = readMessages('locales/ru.json')
  const enKeys = Object.keys(webEn).sort()
  const rustEnKeys = Object.keys(rustEn).sort()
  const ruKeys = Object.keys(webRu).sort()

  if (JSON.stringify(enKeys) !== JSON.stringify(rustEnKeys)
      || JSON.stringify(enKeys) !== JSON.stringify(ruKeys)) {
    throw new Error('Locale key sets differ between locales/en.json, locales/ru.json and src-tauri/locales/en.json')
  }
  if (JSON.stringify(webEn) !== JSON.stringify(rustEn)) {
    throw new Error('English locale copies are not identical')
  }
  return enKeys.length
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const count = localeParity()
  console.log(`Locale parity OK: ${count} keys`)
}
