# Stage 27 review fix: single history toggle

The action bar in `InputPanel.vue` is now the controlled owner of history expansion, but `PhraseHistoryList.vue` still renders its own internal toggle button. This creates duplicate «История фраз» controls in normal and compact modes.

Fix this without changing the working controlled expansion behavior:

- Add an explicit prop/variant that lets the parent hide the list component's internal toggle row.
- Use that variant from `InputPanel.vue`; the action bar's `История фраз` button must be the only toggle.
- Keep `v-model:expanded` or equivalent controlled state and all list/filter/actions intact.
- In compact mode the visible action bar must contain only this one history button; no duplicate toggle may appear below it.
- Run `npx vue-tsc --noEmit`, `cargo check --manifest-path src-tauri/Cargo.toml`, and `git diff --check`.
