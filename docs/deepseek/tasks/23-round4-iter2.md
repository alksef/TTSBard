# Plan 23 — Round 4, iteration 2: review fixes for mark geometry and reset semantics

Fix only these review findings in `src/components/AudioPanel.vue`. Preserve all other Round 3/4 work. Do not touch Rust/docs/unrelated files and do not commit/reset/clean.

1. Remove the old separate `Сбросить` row/button from the voice transform card. Clickable neutral marks now provide reset behavior. Remove `resetVoiceTransform()` and obsolete `.reset-row` / `.reset-btn` CSS if they become unused.
2. Fix mark geometry. `.volume-control` consists of the flexible range track, a 12px gap, and a value with `min-width: 45px`; therefore the marks positioning container must have the same width as the range portion, e.g. `width: calc(100% - 57px)`, rather than full width plus `padding-right`. Verify left 0/25/50/75/100 percentages are relative to this track-width container.
3. Prevent endpoint mark buttons from causing horizontal overflow while keeping them anchored to the track endpoints. Use endpoint-specific classes/selectors to align the first button inward from 0% and the last inward from 100%; intermediate buttons remain centered.
4. Add `aria-label="Нормальная громкость, 100%"` to the volume 100 mark, preserving its title. Add an equivalent `title="Без звука, 0%"` to volume 0 for mouse users.
5. Run `npx vue-tsc --noEmit` and `git diff --check -- src/components/AudioPanel.vue`; remove any trailing whitespace.
