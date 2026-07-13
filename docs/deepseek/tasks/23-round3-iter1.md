# Plan 23 — Round 3, iteration 1: contextual refresh, unsaved warning, stable preview status

Implement this UI refinement in `src/components/AudioPanel.vue`. Preserve all existing device/effect behavior, draft/save semantics, preview race protection, and backend invoke APIs. Do not touch Rust or unrelated files. Do not commit/reset/clean.

## Requirements

1. **Device refresh belongs only to Devices**
   - Render the existing `.panel-footer` refresh action only while `activeTab === 'devices'`.
   - Do not duplicate it or show it in Effects.

2. **Unsaved draft warning at the top of Effects**
   - At the top of the Effects tab content, before the preview card, render a warning banner only when `isDirty` is true.
   - Russian copy: `Есть несохранённые изменения. Нажмите «Сохранить», чтобы применить их к TTS.`
   - Use a Lucide warning icon and the project's existing warning variables (`--warning-bg`, `--warning-bg-weak`, `--warning-border`, `--warning-text` as appropriate). The banner must be orange/warning, compact, accessible, theme-safe, and not a newly invented accent style.
   - It disappears after successful save when `isDirty` becomes false.

3. **Explain which settings preview uses**
   - In the `Проверка эффектов` card, add always-visible compact helper text explaining: `Режим «С эффектами» использует текущие выбранные настройки, даже если они ещё не сохранены.`
   - Place it naturally below the card header and before the file picker/file information. Use the existing muted/info visual language.

4. **Playback indication must not make the panel jump**
   - Remove the current conditional in-flow `Воспроизведение...` row below `.preview-controls`.
   - Show the spinner and `Воспроизведение...` in the preview card header, aligned on the right in a compact status element when `isPreviewPlaying` is true.
   - The card header already occupies that row, so normal playback start/end must not add/remove vertical content or change card height. Keep the title flexible and allow clean narrow-width behavior.
   - Keep `previewError` local below the controls. Do not reserve a permanent error area unless needed; this requirement concerns normal playback indication.

## Styling and accessibility

- Reuse project CSS variables and the current settings visual language.
- Do not add gradients, shadows, or unrelated restyling.
- Ensure the header status and warning banner work at narrow widths without horizontal overflow.
- The warning should use `role="status"` or an appropriate accessible equivalent.

## Verification

- Run `npx vue-tsc --noEmit`.
- Report the exact changed file and result.
