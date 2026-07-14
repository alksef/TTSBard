# Task 100 / Round 1 / Iteration 1

## Goal

Rename the existing HTTP TTS provider without changing behavior, freeing the
`LocalTts` name for the future local-model provider.

## Context

`src-tauri/src/tts/local.rs` is an HTTP client for the TTSVoiceWizard-compatible
local server. It does not execute ONNX and must remain separate from the future
`LocalModelTts` implementation.

## Scope

Find every actual use of the current type and rename only the necessary Rust
symbols, files/modules, imports, and internal comments/log messages. Do not
change the protocol, URL, timeout, request method, response format, settings,
or UI behavior.

Preferred runtime type name: `LocalHttpServerTts`. If renaming `local.rs` to
`local_http_server.rs` is appropriate, update `mod.rs` and all imports. Do not
add ONNX Runtime, commands, settings fields, or UI in this task.

## Requirements

- `LocalTts` is no longer used as the old HTTP-client type name.
- `TtsProvider` still works with the existing HTTP provider.
- The old URL and request/response behavior remain unchanged.
- Logs and comments identify this as an HTTP server provider, not an ONNX model.
- Do not rename user-facing provider labels unless technically required.

## Verification

- Search the source tree for remaining references to the old `LocalTts` symbol.
- Run `cargo check`.
- Review the diff and confirm there are no functional changes.
