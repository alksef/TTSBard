**English | [Русский](../../ru/user/ai-text.md)**

# AI features in the editor

AI can correct spelling/punctuation, recover text typed with the wrong keyboard
layout, change style or suggest a short continuation. Results depend on the
**Prompt** and selected model. Review them when exact meaning matters.
AI and TTS providers are independent: for example, correct with DeepSeek and
speak through Fish Audio or local TTS.

## Language support

The default **Prompt** is written for Russian text, including Russian number
pronunciation and keyboard-layout correction. For English or another language,
replace it with suitable instructions; changing the interface language does
not rewrite the saved prompt. Results also depend on the selected AI model.

**AI: grammar** uses a separate built-in Russian instruction and ignores the
Prompt field. For correction in another language, use **AI: correct** or
automatic correction with your own Prompt. AI continuation requests the same
language as its context.

Before automatic AI correction, TTSBard converts digits to Russian words.
Your Prompt does not disable that earlier step. For English speech, type
numbers as English words; see [language limitations](guide.md#language-support).

## Available features

- **Automatic correction**: **Apply AI correction automatically** processes
  each phrase after quick replacements and number conversion, before TTS.
- **AI: correct**: replaces editor text using the Prompt without starting speech.
- **AI: grammar**: checks Russian spelling/grammar while trying to preserve
  meaning, style and case. It uses a built-in instruction, not the Prompt field.
- **AI: continue**: adds a short continuation in the same language when
  **AI text continuation (autocomplete)** is enabled.
- **AI autocomplete**: offers a short continuation marked `AI` after some text.
  It is inserted only when selected. Both AI continuation and **Show autocomplete
  suggestions** under **Settings → Editor → Proofing and suggestions** must be
  enabled. The master suggestions switch stops all suggestion-source requests
  but does not disable the manual continuation command.

Manual commands are in the editor's **…** menu. The **AI** button next to Speak
runs correction. Manual correction works with automatic correction disabled
as long as the selected AI provider is configured.

## Automatic processing

1. Resolve the [phrase route](editor.md#phrase-route) and remove its control
   prefix. Twitch only does not create a synthesis job.
2. Apply quick replacements and convert numbers to Russian words.
3. If enabled, send prepared text and Prompt to the selected AI provider.
4. Apply [RUAccent](ruaccent.md) for supported providers if enabled, then synthesize.

If automatic AI fails or is unavailable, speech still proceeds using ordinary
preprocessed text without AI correction. Failed manual commands leave the
editor unchanged.

## Setup

1. Open **Settings → AI**.
2. Choose **OpenAI**, **Z.ai**, **DeepSeek** or **Custom**.
3. Expand the provider card, fill in and save required fields: an API key for
   cloud providers, also a URL for Z.ai, and a compatible API URL, key and model
   name for Custom.
4. Enter correction rules in **Prompt** and click **Save**.
5. Enable automatic correction and/or AI continuation as needed.

Request only the resulting text, for example:

```text
Correct spelling, punctuation and text typed with the wrong keyboard layout.
Preserve the meaning and style. Return only the corrected text, without comments.
```

For a style change: “Rewrite in a friendly conversational style without adding
facts. Return only the resulting phrase.” One global prompt serves manual
correction and automatic correction before TTS.

## Network and privacy

OpenAI, Z.ai and DeepSeek receive the typed text and prompt through external
APIs; their pricing, limits and data policies apply. Custom sends requests to
your configured endpoint, local or remote. Do not enable cloud correction for
text you cannot send to a third-party service.
