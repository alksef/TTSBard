# Plan 23 — Round 6, iteration 1: separate end marks and simplify DeepFilterNet card

Implement only these UI corrections in `src/components/AudioPanel.vue`. Preserve all existing slider steps, clickable values, draft behavior, accessibility, and backend APIs. Do not touch Rust/unrelated files and do not commit/reset/clean.

## 1. Prevent end-mark overlap

The user observes `+75` overlapping `+100` on pitch/speed and `175` overlapping `200` on volume at the real panel width.

- Add a semantic class to the penultimate marks (`+75` on pitch/speed and `175` on volume), e.g. `mark-btn--before-end`, and bias those labels inward/left enough that they do not collide with the inward-aligned end labels.
- Keep each button's anchor `left` at its mathematically correct 87.5% position; adjust only text/button translation, not the underlying value or position.
- Use available row width more efficiently if safe, for example slightly reduce only the fixed label reservation/gap for effect slider rows through a scoped class. Do not globally damage device rows. Prefer the minimal reliable fix; do not reduce font below readable size.
- Verify first/last marks still do not overflow.

## 2. Simplify DeepFilterNet

- Remove the clickable `10 dB` mark completely.
- Keep marks `5 / 12 / 20 / 30` at their correct proportional positions (`0 / 28 / 60 / 100%`).
- Keep `12 dB` persistently bold via `mark-btn--default`, with its existing title and aria-label.
- Remove the text `Модель встроена в приложение, загрузка не требуется` and its template element.
- Remove `.model-info` CSS because it becomes unused. Preserve `.model-hint`.

## Verification

- Run `npx vue-tsc --noEmit` and `git diff --check -- src/components/AudioPanel.vue`.
- Report the changed file and exact results.
