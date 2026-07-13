# Unified audio settings save controls

## Goal

Replace the two independent save/cancel toolbars in the Effects & DSP tab with one shared toolbar at the very bottom. The shared actions must save or cancel both voice-effects and DSP drafts together.

## Scope

- `src/components/audio/AudioEffectsTab.vue`
- `src/components/audio/DspSettings.vue`

## Design

`AudioEffectsTab` owns the single save/cancel toolbar because it owns both settings composables and both draft states. `DspSettings` remains a presentation/editor component for DSP controls and must no longer render its own toolbar or require toolbar-specific save status/error props and events.

The shared save action must preserve the existing persistence behavior for both settings groups, handle asynchronous saves without duplicate concurrent requests, and expose one coherent saving/success/error state in the shared toolbar. The shared cancel action must restore both drafts to their last saved values. Keep the existing individual DSP controls, collapsible sections, presets, and preview behavior unchanged.

## Acceptance criteria

1. Only one save toolbar is rendered in the Effects & DSP tab, at the bottom after the DSP content.
2. The toolbar has one Cancel and one Save button; no duplicate buttons remain inside `DspSettings`.
3. Save persists both voice effects and DSP drafts; cancel restores both drafts.
4. Buttons are disabled while the combined save operation is running and when there are no unsaved changes.
5. Existing success and error feedback remains visible in the shared toolbar without losing an error from either settings group.
6. No horizontal overflow is introduced at narrow panel widths; the existing scroll layout and spacing remain intact.
7. `DspSettings` has no dead props/events related solely to its removed toolbar.
8. Verify with `npx vue-tsc --noEmit` and `npm run build`.
