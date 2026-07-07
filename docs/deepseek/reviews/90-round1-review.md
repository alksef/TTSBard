# Review: Plan 90 Round 1 — playback-control window autosize + CSS safety net

- **Task:** `docs/deepseek/tasks/90-round1-01.md`
- **Date:** 2026-07-07
- **Verdict:** APPROVED

## Checks

- `npx vue-tsc --noEmit --project tsconfig.json` — **0 errors** ✅
- `cargo check` (src-tauri) — **Finished, 0 errors** ✅

## Diff analysis

### Part 1 — Autosize (`resizeToFit`)
- Imports: `watch`, `nextTick`, `LogicalSize` added correctly
- `resizeToFit()`: `nextTick` → `rAF` → `offsetHeight` → clamp(150,600) → DPI-aware comparison via `scaleFactor()` → `setSize(LogicalSize(350, clamped))` only if height differs by ≥1px
- Trigger points: `onMounted` after state/appearance load, `playback-appearance-update` listener, `watch(state, deep)` on all playback events
- Loop prevention: `Math.abs(current - clamped) < 1` guard — `setSize` doesn't emit Tauri events, so no `fetchState` → `watch` → `setSize` cycle

### Part 2 — CSS safety net
- `html, body { height: 100vh; margin: 0; }` — fills full window area
- `body { background: var(--bg); }` — CSS fallback before JS runs
- `syncBodyBackground()` sets inline `document.body.style.background = overlayStyle.backgroundColor` — overrides CSS var for dynamic opacity/color
- Called in `onMounted` and `playback-appearance-update` — keeps body bg in sync with card bg
- Card `.playback-window` keeps `border-radius: 16px` — transparent corners outside radius remain transparent (correct)

### Part 3 — tauri.conf.json
Not modified — correct. Initial 400px height overridden immediately by autosize on mount.

## Plan criteria

| # | Criterion | Status |
|---|-----------|--------|
| 1 | No transparent void at bottom | ✅ Body bg fills window; autosize matches height to content |
| 2 | Window height adapts to content | ✅ `watch(state, deep)` + `resizeToFit` on queue/recent changes |
| 3 | Width = 350px all DPIs | ✅ `LogicalSize(350, clamped)` with scaleFactor for comparison |
| 4 | Drag + buttons work | ✅ No template/header changes; all handlers untouched |
| 5 | No flapping/setSize-loop | ✅ Same-height guard + setSize doesn't emit Tauri events |
| 6 | Themes correct | ✅ CSS var(--bg) fallback + syncBodyBackground for dynamic values |
| 7 | `cargo check` + `vue-tsc` green | ✅ Both pass |
| 8 | Position preserved | ✅ No changes to position code |

## Notes
- The `watch(() => state.value, ...)` pattern is correct: it watches the same reactive proxy object with `deep: true`, detecting all nested changes from playback events
- `syncBodyBackground` + `resizeToFit` call order at line 107-108 occurs after potential watch-triggered resizeToFit from `fetchState()` (line 105), but same-height guard prevents duplicate `setSize`
- No Rust or config changes needed — only Vue component modified
