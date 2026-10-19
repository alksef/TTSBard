**English | [Русский](../../ru/integrations/README.md)**

# Integrations

Choose a connection scenario. Guides cover both TTSBard and the other
application; HTTP/SSE references also help authors of external clients.

## Connect another application

| Task | Guide |
|---|---|
| Display phrases in OBS | [WebView overlay](webview.md): server, addresses, templates, external access, troubleshooting |
| Receive text and typing in your own client | [SSE contract](sse.md): endpoints, events, authentication, examples, delivery limits |
| Receive game text from LunaTranslator | [LunaTranslator → TTSBard](lunatranslator/README.md): selfbuild script and forwarding |
| Send text from another application | [Input server](../user/input-server.md): HTTP API, queue and manual review |
| Show typing on a model | [VTube Studio](vtube-studio.md): Plugin API, INPUT/OUTPUT, hotkeys and scene items |
| Duplicate phrases in Twitch Chat | [Twitch](twitch.md): Twitch API/classic IRC, connection/recovery |
| Send phrases and typing to a chatbox | [VRChat](vrchat.md): OSC, UDP, routes and formatting |

WebView/SSE is outgoing text. Input server receives text for speech. They are
different servers, default ports `10100` and `10101`. Input server accepts local
and LAN text; non-loopback clients require its token.

## Screen recognition and models

Built-in OCR appears in integrations but requires no external client. The
[OCR guide](../user/ocr.md) covers eSlav PP-OCRv5 Mobile downloads, manifest,
folders and recognition. If LunaTranslator already captures/translates text,
use its script; you do not also need built-in OCR.

## Speech synthesis

[TTS providers](../user/tts.md) covers Silero/Telegram, ElevenLabs and Piper.
See separate guides for [Silero timing](../user/silero-settings.md) and
[RUAccent models](../user/ruaccent.md).
