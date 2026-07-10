# Plan 111: SoundPanelTab — утечка Tauri-listener'ов

**Дата:** 2026-07-11  
**Источник:** review-001-2026-07-11 (MINOR)  
**Сложность:** Средняя — рефактор `<script setup>` SoundPanelTab.vue.

---

## Проблема

`onUnmounted` регистрируется внутри `async onMounted` после минимум 4 `await`-границ
(строки ~323–354 в `SoundPanelTab.vue`). По документации Vue 3, lifecycle-хуки
работают только при синхронном вызове во время setup. После первого `await`
`currentInstance === null`, поэтому `onUnmounted`-колбек **никогда не вызывается**.

Следствие: три Tauri-listener'а (`unlistenAppearance`, `unlistenBindings`,
`unlistenActiveSet`) **никогда не отписываются** при размонтировании компонента.

---

## Решение

### Паттерн: cleanup-список на уровне setup

```typescript
// В <script setup>, вне всех хуков — синхронно
const cleanups: Array<() => void> = []

// Единственный onUnmounted — синхронный, на верхнем уровне
onUnmounted(() => {
  cleanups.forEach(fn => fn())
  cleanups.length = 0
})

// В onMounted: собираем cleanup через push
onMounted(async () => {
  await loadSets()
  await loadBindings()
  // ...
  const unlistenAppearance = await listen('soundpanel-appearance-changed', ...)
  cleanups.push(() => unlistenAppearance())

  const unlistenBindings = await listen('soundpanel-bindings-changed', ...)
  cleanups.push(() => unlistenBindings())

  const unlistenActiveSet = await listen('soundpanel-active-set-changed', ...)
  cleanups.push(() => unlistenActiveSet())
})
```

`onUnmounted` регистрируется **синхронно на уровне setup** — до любых `await`.
Колбек вызывается при размонтировании и дренирует `cleanups`.

---

## Ограничения
- Не переписывать логику загрузки (loadSets, loadBindings, loadAppearanceSettings).
- Не изменять события, которые слушаются.
- Если в файле уже есть другие `onUnmounted` — объединить с ними или убедиться
  что не дублируют логику.
- `vue-tsc --noEmit` — 0 ошибок после правки.
