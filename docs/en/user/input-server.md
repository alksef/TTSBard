**English | [Русский](../../ru/user/input-server.md)**

# Incoming text server

The server receives text from other applications and passes it through TTSBard's
normal speech pipeline. The sender extracts/prepares text; TTSBard synthesizes
and plays it using the active provider/voice, preprocessing, enabled AI/RUAccent,
effects, Speaker and Virtual microphone. Incoming phrases default to audio only;
other [Incoming routes](incoming.md) can include WebView, Twitch and
[VRChat](../integrations/vrchat.md).

## Start and configure

1. Open **Input server** in the integrations section of the sidebar.
2. Optionally change the port (`1024–65535`, default `10101`).
3. Click **Start**; a changed port is saved before starting. **Save** saves
   settings without starting.
4. Wait for **Running**.

Stop before changing the port, save and start again. **Start with the app**
starts the listener on the next launch.

The default bind is `0.0.0.0`: loopback `127.0.0.1` works without a token;
other devices require one. The panel provides **On this PC** and **On local
network** addresses. Copy the LAN link with its token to a phone/tablet on the
same network; allow incoming connections through Windows Firewall if needed.
This integration does not use CORS or UPnP.

Remote clients pass `Authorization: Bearer <token>` or `?token=<token>`.
`/health` is unauthenticated. The LAN web form forwards its token automatically.
**Generate a new token** immediately invalidates old links without restarting
the server; copy fresh links to your devices.

## Accepted phrases

See [Incoming](incoming.md) for automatic acceptance, manual review and shortcuts,
shared by HTTP and OCR.

## Connect a client

Copy the endpoint from the panel:

```text
http://127.0.0.1:10101/v1/speech
```

Send `POST`, `Content-Type: application/json` and a `text` field:

```json
{"text": "Incoming server test"}
```

PowerShell test:

```powershell
Invoke-RestMethod http://127.0.0.1:10101/health
$body = [Text.Encoding]::UTF8.GetBytes('{"text":"Incoming server test"}')
Invoke-RestMethod http://127.0.0.1:10101/v1/speech `
  -Method Post -ContentType 'application/json; charset=utf-8' -Body $body
```

`/health` returns `{"status":"ok"}`. Automatic acceptance returns `202 Accepted`,
`job_id` and `queued`; manual mode returns `incoming_id` and `pending_review`.
Limits: 20,000 Unicode characters and a 64 KiB request body.

## Test without another application

Enter text in the panel's **Test** block and click **Send**. Automatic mode
queues speech; manual mode places it in Incoming. This checks TTSBard before
configuring an external client.

## Overlay input form

While the server runs, a browser form is available on the same configured port:

```text
http://127.0.0.1:10101/overlay
```

The minimal form has a multiline field and status area, without headings or
buttons. Enter sends, Shift+Enter inserts a newline, empty text is not sent.
Successful submission clears/refocuses the field and shows `queued` or
`pending_review`. Use any browser on this PC, including Steam Overlay, or the
token-bearing LAN link on another device. It posts to the same `/v1/speech`
without external resources or CORS.

## Common errors

| HTTP | Cause/action |
|---|---|
| `400` | Invalid JSON; check the body and required `text` field |
| `401` | Missing/expired token on a non-loopback connection; copy the current LAN link or use Bearer authentication |
| `403` | Invalid Host; use the panel's loopback/local-network IP address |
| `413` | Body exceeds 64 KiB; shorten it |
| `415` | Missing `Content-Type: application/json` |
| `422` | Empty text or over 20,000 characters |
| `429` | Speech queue or pending inbox full; retry later |
| `503` | TTS provider/output not ready; check TTS and Audio |

If startup fails, another application may occupy the port. Stop that process
or select a different port, save and start again.

## Example: LunaTranslator

The [selfbuild integration](../integrations/lunatranslator/README.md) forwards
recognized/translated game text without returning audio to LunaTranslator's
player, avoiding double playback.
