**English | [Русский](../../../ru/integrations/lunatranslator/README.md)**

# LunaTranslator → TTSBard

A selfbuild TTS engine forwards game text (HOOK/OCR and translation handled by
LunaTranslator) through TTSBard's [Input server](../../user/input-server.md).
For direct screen capture, see [built-in OCR](../../user/ocr.md).

TTSBard handles synthesis/playback: active provider/voice, preprocessing, stress,
effects, queue and speaker/virtual microphone. LunaTranslator's player is silent,
preventing double playback; its volume/speed/pitch controls have no effect.

```text
LunaTranslator (HOOK/OCR, translation) --POST {"text"}--> TTSBard
```

The server acknowledges text with `202`; no audio is returned. TTSBard speaks
it itself. The original [integration research](../../../research/15-lunatranslator-input-server-integration.md)
is available in Russian for background; it is not required for setup.

## Requirements

- TTSBard 0.25.0 or newer.
- LunaTranslator with the selfbuild engine (`自定义`), tested on v10.16.5.2.
  Labels depend on its translation; the Russian label has appeared as
  “Настройка” / “Собственный”. Identify it by selfbuild / `自定义`.

## Configure TTSBard

1. Open **Input server** in the sidebar's integrations area.
2. Start it (default `10101`); use `127.0.0.1` for the script on this PC.
3. Enable **Accept automatically** for immediate speech. Otherwise, phrases await
   review in Incoming: use `Ctrl+K` to speak a phrase or `Ctrl+I` to edit it.

## Install the LunaTranslator script

1. Copy [selfbuild_tts.py](../../../integrations/lunatranslator/selfbuild_tts.py)
   to `<LunaTranslator folder>\userconfig\selfbuild_tts.py`. Alternatively open
   the selfbuild engine's pencil editor, paste and save. It hot-reloads without restart.
2. Enable that engine (`自定义`). It appears under the Online tab due to
   LunaTranslator's classification, though the script is entirely local.
3. Enable automatic reading under Behavior so that each phrase invokes the engine.

`TTSBARD_URL` supplies the server address (default `http://127.0.0.1:10101`);
selfbuild has no settings UI. Set it before starting LunaTranslator for a custom port.

## Test

```powershell
Invoke-RestMethod http://127.0.0.1:10101/health
$body = [Text.Encoding]::UTF8.GetBytes('{"text":"Connection test"}')
Invoke-RestMethod http://127.0.0.1:10101/v1/speech `
    -Method Post -ContentType 'application/json; charset=utf-8' -Body $body
```

The first returns `status: ok`. Automatic acceptance returns HTTP `202`,
`status: queued`, `job_id`, and speech. Manual review returns
`status: pending_review`, `incoming_id`. Trigger a read in LunaTranslator:
sound should come from TTSBard, with the job in Playback control and incoming
text in Text → Incoming.

## Notes

- The script returns no audio, so LunaTranslator sends repeated phrases again
  rather than caching its own audio result.
- `429`: queue/inbox full; `422`: empty/too long; `503`: provider not ready.
  LunaTranslator shows an engine error.
- The supplied script is designed for local unauthenticated access. The server
  also supports LAN, but remote clients need token authentication as described
  in its guide.
- Stop/pause/repeat are controlled by TTSBard's playback window, not LunaTranslator.
