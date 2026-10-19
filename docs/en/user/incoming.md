**English | [Русский](../../ru/user/incoming.md)**

# Incoming: HTTP and OCR

**Incoming** is shared by the [HTTP server](input-server.md) and [built-in OCR](ocr.md).
It appears when the server is running, OCR is enabled or phrases await review,
and participates in ordinary editor tab switching. Its presence does not mean
the OCR model is ready: check the OCR panel.

## Automatic acceptance

With **Accept automatically** enabled, received text enters the shared speech
queue using the selected provider/voice. View it in Playback control. Accepted
jobs retain their original provider, even after you switch voices.
Normal text preparation, enabled AI/RUAccent, effects and audio outputs apply.
Successfully spoken phrases enter history.

Choose a route next to the automatic-acceptance switch:

| Route | Destinations |
|---|---|
| Voice | Speech queue only |
| Everywhere | Speech queue, WebView, Twitch and enabled VRChat |
| No Twitch | Speech queue, WebView and enabled VRChat |
| Voice + Twitch | Speech queue and connected Twitch |

All routes include audio. The shared HTTP/OCR choice persists and applies both
to automatic acceptance and to the manual **Speak** action. Local speech continues if an external
destination is unavailable. WebView/Twitch availability is shown; disabled
VRChat is a dim icon without connection warning because OSC provides no
delivery acknowledgement. See [VRChat](../integrations/vrchat.md).

## Manual review

Disable **Accept automatically** to review phrases:

| Action | Result |
|---|---|
| Speak | Send original text through normal preparation/speech processing |
| Edit | Move text to the special **Incoming text** tab to edit and send using its incoming route |
| Reject | Remove without synthesis or history entry |

The switch in Incoming and the HTTP panel controls the same setting for both
sources. OCR does not require the HTTP server. One special editing tab is reused:
the next selected phrase replaces its text and route. Sending/canceling closes it.

## Shortcuts

| Default | Action |
|---|---|
| `Ctrl+K` | Speak the next pending phrase |
| `Ctrl+I` | Edit the next pending phrase |

Change these local editor shortcuts under **Hotkeys → Editor**. Pause, stop and
repeat accepted jobs in Playback control (`Ctrl+Shift+F7`).

## If a phrase is missing

Check the source: [HTTP responses/server status](input-server.md#common-errors)
or [OCR readiness](ocr.md#if-the-model-does-not-work). Check manual review for
pending text or playback state for accepted jobs.
