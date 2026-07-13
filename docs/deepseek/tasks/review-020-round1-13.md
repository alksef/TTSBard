You must edit the repository now; do not only inspect or report.

Centralize the existing `settings-changed` event name without changing its behavior or payload. Backend: add a public constant in `src-tauri/src/commands/mod.rs` and make `emit_settings_changed` use it. Frontend: add an exported constant in `src/types/settings.ts` and make `src/composables/useAppSettings.ts` and `src/components/InputPanel.vue` use that constant instead of the literal. Do not change event names, payloads, DTOs, security fields, or unrelated events. Run `cargo check` and `npx vue-tsc --noEmit` after editing.
