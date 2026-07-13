# Plan 23 final review fixes — non-blocking IPC and exact volume semantics

Fix only these final review findings; preserve all unrelated work.

1. `preview_audio_file` is currently a long synchronous Tauri command that performs decoding/DeepFilterNet and waits for playback completion. Make it async and move all CPU/blocking processing and playback into `tauri::async_runtime::spawn_blocking` (or the project's established equivalent), so `stop_preview` can be invoked concurrently and actually stop playback.
2. Match real TTS volume semantics exactly:
   - clamp incoming pitch/speed to `-100..100`, effect volume to `0..200`, attenuation to `5..30`, speaker volume to `0..100`;
   - effective output volume is `speaker_base * effect_volume_factor` when voice transform is enabled;
   - do not clamp the final value to `1.0`; with 100% speaker and 200% effect it must be `2.0`, matching `tts_pipeline::enqueue_and_record`.
3. The newly added tests in `audio/effects.rs` duplicate production logic instead of testing it and currently contain contradictory clamp expectations. Remove those preview-only helper/tests, or refactor calculation into a production helper and test that actual helper. Prefer minimal production code. Existing project test compilation is blocked by the unrelated missing `tower` dev dependency; do not expand scope to that issue.
4. Confirm frontend Stop remains enabled while the awaited async preview command is running and resets state cleanly.

Run `npx vue-tsc --noEmit` and `cargo check --manifest-path src-tauri/Cargo.toml`, with zero warnings. Report results. Do not commit/reset/clean.
