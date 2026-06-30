# Plan 83: Очистка не того таба после speak (quick-editor race)

**Дата:** 2026-06-30
**Статус:** draft (для DeepSeek по WORKFLOW)
**Связано:** план 77 (editor tabs — computed-прокси `text`), `InputPanel.vue` handleEnter/speak.

## Контекст / баг
В quick-editor mode (`editorSettings.quick = true`) при нажатии Enter:
1. Текст активного таба уходит на TTS (`speak()` — **fire-and-forget**, без await).
2. `text.value = ''` очищает активный таб.

**Баг:** если пользователь после Enter **переключился на другой таб** (пока TTS ещё
озвучивает/обрабатывается), то:
- `speak()` и `recordHistory()` внутри — async, читают `text.value` **после** await-паузы.
  К этому моменту `text.value` уже возвращает текст **другого** таба (активный сменился).
- `text.value = ''` очищает **новый** активный таб, а не тот, что отправлялся на TTS.

**Причина:** `text` — `computed`-прокси к **активному** табу (план 77). Любая отложенная
операция (fire-and-forget speak, recordHistory после await) читает «активный таб на момент
чтения», а не «таб, с которым работали». Race condition между async-операцией и
переключением таба.

## Воспроизведение
1. Включить quick-editor mode (настройка).
2. Таб 1: набрать текст, нажать Enter → текст ушёл на TTS.
3. **Быстро** переключиться на таб 2.
4. После озвучивания — таб 2 очищается (а должен был очиститься таб 1, либо не очищаться
   вообще, т.к. это другой таб).

## Решение (архитектурное)
**Захватывать текст таба в локальную переменную синхронно** в момент Enter, до любых
async-операций. Передавать его **явно** в `speak`/`recordHistory`, не читая `text.value`
позже.

### Изменения в `src/components/InputPanel.vue`
1. `speak()` принимает текст параметром: `async function speak(textToSend: string)`.
   Внутри использует `textToSend` вместо `text.value`. Соответственно `recordHistory(textToSend)`.
2. `recordHistory()` принимает текст параметром: `async function recordHistory(textToRecord: string)`.
3. `handleEnter()` — захватить текст синхронно:
   ```ts
   async function handleEnter() {
     const currentText = text.value          // ← захват СИНХРОННО, до await
     if (!currentText.trim()) return
     const quickEditorEnabledValue = editorSettings.value?.quick ?? false
     if (quickEditorEnabledValue && !currentText.trim()) return

     if (quickEditorEnabledValue) {
       speak(currentText)                    // fire-and-forget, но текст уже зафиксирован
       // Очистить ИМЕННО тот таб, с которого отправили:
       active.value.text = ''                // ← очистить active-таб (он = currentText в этот момент)
       await hideMainWindow()
     } else {
       await speak(currentText)
       active.value.text = ''
     }
   }
   ```
4. `correctText`/`completeText` — тоже читают `text.value` синхронно в начале (они уже
   примерно так делают, но проверить, что нет отложенного чтения после await).

### Тонкость: «очистить именно тот таб»
`active.value.text = ''` в строке очистки — `active` это computed к **активному** табу. Между
захватом `currentText` и строкой очистки нет await (оба синхронны в одной функции до первого
await в quick-ветке) → активный таб тот же → очищается правильно. **Но** `speak(currentText)`
запускается fire-and-forget **до** очистки — это ок, текст уже в локальной переменной.

Альтернатива (надёжнее): очистить по **id таба**, а не по `active`:
```ts
const activeTabId = activeId.value   // захват id синхронно
speak(currentText)
const tab = tabs.value.find(t => t.id === activeTabId)
if (tab) tab.text = ''
```
Это полностью устраняет зависимость от того, какой таб активен в момент очистки.

## Риски
- Не сломать normal mode (не-quick): там `await speak()` → после очистка. С параметром —
  поведение идентично, просто текст передаётся явно.
- `recordHistory` вызывается из `speak` — пробросить параметр.
- Проверить, что `speak`/`recordHistory` не вызываются ещё откуда-то со старой сигнатурой
  (без параметра) — grep по вызовам.

## Критерии готовности
1. `speak(text)` и `recordHistory(text)` принимают текст параметром, не читают `text.value`
   после await.
2. `handleEnter` захватывает `currentText` синхронно до await.
3. Очистка идёт по id таба (или гарантированно синхронно до любого переключения).
4. **Тест кейс:** таб 1 → Enter → переключиться на таб 2 → после TTS таб 1 очищен, таб 2
   сохранён (или наоборот — очищается ровно тот, с которого отправили, а не «активный на
   момент очистки»).
5. `npx vue-tsc --noEmit` — 0 ошибок.

## Объём
Малый, фронт-only (1 файл). По WORKFLOW — через DeepSeek (task-файл + round), либо прямая
правка (тривиально, но требует аккуратности с синхронностью).
