# Plan 23 — Round 4, iteration 1: stepped sliders with clickable reference marks

Implement this refinement in `src/components/AudioPanel.vue`. Preserve all unrelated Round 2/3 behavior and backend payloads. Do not touch Rust or unrelated files. Do not commit/reset/clean.

## Required behavior

1. Pitch slider (`-100..100`) uses `step="5"`.
2. Speed slider (`-100..100`) uses `step="5"`.
3. Volume slider (`0..200`) uses `step="20"`.
4. Add compact clickable reference marks below each slider:
   - pitch: `−100`, `−50`, `0`, `+50`, `+100`;
   - speed: `−100`, `−50`, `0`, `+50`, `+100`;
   - volume: `0`, `100`, `200`.
5. Clicking a mark directly assigns that numeric value to the corresponding `draftEffects` field and calls the existing dirty-state behavior. Do not persist automatically.
6. Semantics must remain explicit: pitch/speed `0%` are neutral; volume `0%` is mute and volume `100%` is neutral. Give the volume 100 button a Russian tooltip/accessible label indicating `Нормальная громкость, 100%`; give volume 0 an accessible label indicating `Без звука, 0%`.

## UI rules

- Use real `<button type="button">` elements for marks so they are keyboard accessible.
- The active/current mark may use the existing accent color; do not invent gradients or shadows.
- Marks must align to their actual position along the slider track. Avoid a fake equally-spaced layout when values are not equally spaced.
- Keep the numeric value and existing controls aligned. Do not add a separate reset icon/button; the neutral clickable mark is the reset action.
- Avoid horizontal overflow at narrow widths. Labels may be compact and should use the current settings visual language.
- Disabled effect sections must also disable their mark buttons consistently with the slider.

## Implementation guidance

- A small local helper such as `setEffectValue(field, value)` is preferred over duplicating assignment + `markDirty()` in every click handler, as long as TypeScript remains precise.
- A small reusable local template/CSS pattern is enough; do not add a slider library or new dependency.

## Verification

- Run `npx vue-tsc --noEmit`.
- Report exact changed files and the command result.
