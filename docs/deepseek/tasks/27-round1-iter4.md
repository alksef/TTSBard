# Stage 27 review fix: text-only menu trigger

In `src/components/editor/EditorMenu.vue`, the action bar requirement is text-only controls and the agreed label is `[⋯]`. Replace the decorative `MoreHorizontal` icon in the menu trigger with the literal text `⋯` (remove the now-unused icon import). Keep the existing menu behavior and styling. Add an explicit `aria-label` to the icon/text-only menu trigger in addition to its title. Run `npx vue-tsc --noEmit` and `git diff --check`.
