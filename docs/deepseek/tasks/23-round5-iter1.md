# Plan 23 — Round 5, iteration 1: fine slider steps and denser clickable marks

Implement this refinement in `src/components/AudioPanel.vue`. Preserve Round 3/4 behavior and backend payloads. Do not add dependencies, touch Rust/unrelated files, or commit/reset/clean.

## Percentage sliders

1. Set pitch, speed, and volume native slider `step` to `1`.
2. Pitch and speed clickable marks must be:
   - values: `-100, -75, -50, -25, 0, 25, 50, 75, 100`;
   - positions: `0%, 12.5%, 25%, 37.5%, 50%, 62.5%, 75%, 87.5%, 100%`;
   - labels use a real minus and explicit plus for positive values.
3. Volume clickable marks must be:
   - values/labels: `0, 25, 50, 75, 100, 125, 150, 175, 200`;
   - positions use the same 0/12.5/.../100 percentages.
4. Keep volume `100` visually distinguished as the neutral/default mark even when it is not currently selected. Current selection must still have the active state. Use a semantic class such as `mark-btn--default`, with font-weight only or another restrained existing-theme treatment.
5. Preserve volume 0 mute and volume 100 normal accessible labels/tooltips.

## DeepFilterNet slider

1. Set attenuation slider to `step="1"` (range remains `5..30`).
2. Reuse the clickable marks pattern below it, but with actual proportional positions for this non-zero range:
   - `5 dB` at 0%;
   - `10 dB` at 20%;
   - `12 dB` at 28%;
   - `20 dB` at 60%;
   - `30 dB` at 100%.
3. Clicking assigns `draftEffects.enhance_atten_db` and calls dirty-state handling. Extend the typed helper safely or add a small dedicated helper.
4. `12` is the application default. Give it a persistent `mark-btn--default` class and Russian `title`/`aria-label` such as `Значение по умолчанию, 12 dB`. It remains bold when another value is selected; active state still denotes the current value.
5. Marks and slider are disabled when DeepFilterNet is disabled.

## Layout

- Nine percentage marks are denser than Round 4. Keep them readable and clickable without horizontal overflow. Use compact text-style buttons/ticks if bordered pills would collide; retain visible keyboard focus and active state.
- Endpoint alignment must remain inward and all mark positions must be relative to the range track width, excluding the numeric value column.
- Do not change card spacing or other AudioPanel styling.

## Verification

- Run `npx vue-tsc --noEmit` and `git diff --check -- src/components/AudioPanel.vue`.
- Report the changed file and exact results.
