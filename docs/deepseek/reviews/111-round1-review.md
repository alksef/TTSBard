# Review 111 round 1

## Verdict

APPROVED

## Finding

The conditional `.draft-warning` is now a sibling between `.preview-panel-fixed`
and `.effects-scroll`. It remains visible while the settings content scrolls, and
the save/cancel behavior is unchanged.

## Verification

- `npx vue-tsc --noEmit` — passed.
- `npm run build` — passed.
- `git diff --check` — passed.
