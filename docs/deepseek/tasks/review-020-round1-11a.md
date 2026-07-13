You must edit the repository now; do not only inspect or report. Implement this change now in `src/components/HotkeysPanel.vue`.

Remove the direct `invoke<HotkeySettingsDto>('get_hotkey_settings')` read. Import and call `useAppSettings`, expose `settings.value?.hotkeys` as the `hotkeys` state used by the template, and use the context `reload()` after successful `set_hotkey` and `reset_hotkey_to_default` so there is one read source. Keep all write commands, recording behavior, and UI unchanged. Do not touch backend or security DTOs. Run `npx vue-tsc --noEmit` after editing.
