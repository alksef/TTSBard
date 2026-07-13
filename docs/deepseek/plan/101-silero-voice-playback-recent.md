# Plan 101 — Silero voice metadata and playback recent dedup

Stage: `docs/stage/34-silero-voice-and-playback-recent-dedup.md`

## Task split

- `101-round1-01.md`: persist Silero selected voice in settings.
- `101-round1-02.md`: use stable identity for persistent-cache replay and prevent duplicate queued/recent entries where appropriate.

## Verification

After each task: focused tests and `cargo check --manifest-path src-tauri/Cargo.toml`; after both tasks also run `npx vue-tsc --noEmit`.
