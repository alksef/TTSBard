# Task: Restore explicit no-migration default

The previous task was stopped after it partially changed `src-tauri/src/config/windows.rs`. The field `MainWindowSettings.custom_opacity` currently references a nonexistent `default_main_custom_opacity` function.

Because the product decision is explicitly **not** to preserve/migrate old opacity behavior, restore this field to `#[serde(default)]`, with `false` in the existing `Default` implementations. Do not add a default function. Leave `opacity_compact_only` and all other code unchanged.

Run `cargo check --manifest-path src-tauri/Cargo.toml` and `git diff --check`.
