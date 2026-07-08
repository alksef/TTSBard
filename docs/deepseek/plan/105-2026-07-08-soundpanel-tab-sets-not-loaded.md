# План 105: Bug — на вкладке «Звуковая панель» не подгружены сохранённые наборы (Sets) при старте

- **Дата:** 2026-07-08
- **Тип:** bug / возможная regression (frontend)
- **Симптом (от пользователя):** «после запуска на вкладке звуковая панель не подгружены
  сохраненные вкладки» (= сохранённые **наборы/Sets** звуковой панели не показываются)
- **Контекст цикла:** см. `docs/deepseek/WORKFLOW.md`

---

## Что проверено Claude (факты)

1. **Файл на диске корректен** — `soundpanel_bindings.json` содержит **3 набора** (Основной с 3
   bindings, «111» с 1, «1» с 1), `active_set_id` валиден. Бэкенд `sp_get_sets`/`sp_get_bindings`
   вернёт эти данные.
2. **SoundPanelTab монтируется при старте.** `App.vue:132` — `<SoundPanelTab v-show=... />`
   (v-show, НЕ v-if). Ветка `v-else` (App.vue:125) защищена только `appSettings.error` — при
   отсутствии ошибки SoundPanelTab рендерится сразу. ⇒ `onMounted` (SoundPanelTab.vue:318)
   срабатывает при старте → `loadSets()` + `loadBindings()`.
3. **`loadSets()`/`loadBindings()` выглядят корректно** — `invoke('sp_get_sets')` →
   `sets.value = result.sets`, `invoke('sp_get_bindings')` → `bindings.value`. Без ранних return.
4. **События reload есть** — `soundpanel-bindings-changed` / `soundpanel-active-set-changed`
   перезагружают sets/bindings. План 96 добавил emit bindings-changed при show плавающего окна
   (НЕ влияет на SoundPanelTab первичную загрузку).
5. **`sets` — локальный ref**, не связан с `appSettings` (не перетирается watch).

⇒ Статически причина **не находится** — путь кода выглядит правильно. Это требует **runtime
диагностики**: что реально возвращает `sp_get_sets` при первичной загрузке, и есть ли ошибка в
catch (которая сейчас глушится без видимого лога).

---

## Подозреваемые (после диагностики)

- **A — silent catch:** `loadSets`/`loadBindings` в catch только `debugError` (не виден без
  devtools-фильтра). Если `invoke` падает (напр. команда не зарегистрирована / state не готов) —
  пользователь видит пусто без ошибки. **Главный кандидат.**
- **B — timing:** если `onMounted` отрабатывает ДО того, как бэкенд завершил `load_bindings`
  (setup.rs:158). Но load_bindings идёт ДО окон — маловероятно. Проверить логом.
- **C — regression от плана 96/101:** планы трогали `show_soundpanel_window` и `sp_play_binding`,
  не `sp_get_sets`/SoundPanelTab. Но проверить, что регистрация команд в lib.rs не сдвинулась
  (sp_get_sets на месте).

---

## Задача DeepSeek (диагностика → фикс)

### Этап 1 — добавить видимые логи (МИНИМАЛЬНО)
В `src/components/SoundPanelTab.vue`:
1. В `loadSets()` — после `const result = await invoke(...)`:
   `console.log('[SoundPanelTab] loadSets result:', result)` (видно sets.length, active_set_id).
   В catch — `console.error('[SoundPanelTab] loadSets FAILED:', e)`.
2. В `loadBindings()` — аналогично: `console.log('[SoundPanelTab] loadBindings:', loadedBindings)`
   + `console.error` в catch.
3. В `onMounted` (318) — `console.log('[SoundPanelTab] onMounted: loading sets+bindings')`.

Эти логи ПОКАЖУТ в DevTools (F12 → Console) основного окна:
- если `loadSets result: {sets: [...3...], active_set_id: ...}` → данные приходят, баг в
  рендере/реактивности sets.
- если `loadSets FAILED: ...` → причина ошибки (команда/state/timing).
- если onMounted не логирует вообще → SoundPanelTab не монтируется при старте (тогда смотреть
  App.vue v-else условие).

### Этап 2 — фикс (по данным логов)
- Если A (silent catch / команда падает): найти причину (регистрация / state timing), починить.
- Если B (timing): добавить перезагрузку при появлении данных (слушать событие готовности, ИЛИ
  retry).
- Если C: вернуть регистрацию/логику.

### Этап 3 — верификация
- `npx vue-tsc --noEmit` 0 ошибок.
- (runtime — пользователь проверит по логам, затем фикс)

## Главное
НЕ угадывать фикс. Сначала логи → данные от пользователя → точечный фикс. Это та же ситуация как
план 102 (regression без явной причины в коде — нужен runtime-факт).

## Не делать
- Не переписывать loadSets/loadBindings логику без причины.
- Не трогать бэкенд sp_get_sets (он возвращает корректные данные — проверено на диске).
