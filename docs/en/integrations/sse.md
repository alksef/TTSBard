**English | [Русский](../../ru/integrations/sse.md)**

# SSE: client contract

WebView delivers text/typing through Server-Sent Events. See
[WebView](webview.md) for listener, OBS, templates, network and UPnP setup.

## HTTP endpoints

| Request | Result |
|---|---|
| `GET /` | HTML with CSS inserted. Public clients need a cookie or query `token`, otherwise `401`. A valid query token sets a cookie. |
| `GET /auth?token=...` | Valid configured token: `200` and `webview_auth` cookie; missing/wrong: `401`. |
| `GET /sse` | Authorized: `200`, `Content-Type: text/event-stream`; otherwise `401`. |

The cookie uses `HttpOnly; Path=/; SameSite=Lax`. Public `/sse` access checks
the cookie; a token in the SSE URL does not replace it. Local clients are
exempt according to WebView's rules. Browser clients must share scheme, host
and port with the server. No CORS headers are added. Cross-origin clients need
a configured proxy; `withCredentials: true` alone is insufficient.

## Events

Events end with a blank line; `data:` contains JSON.

### Connection

```text
event: connected
data: {}

```

Sent immediately when the stream opens; a connection confirmation, not a phrase.

### Text

```text
data: {"text":"Hello!"}

```

Without `event:`, this reaches `onmessage`. `text` is the display string, not
confirmation of synthesis/playback completion. HTTP/OCR appear only with an
Incoming route including WebView; Twitch is not an incoming source for SSE.

### Typing

```text
event: typing
data: {"typing":true}

```

True starts typing, false ends it, subject to the editor's typing switch.
Named events do not invoke `onmessage`, preserving older text-only clients.
Active true repeats at least every five seconds: the editor emits an initial
start and idle-timeout end, so refreshes distinguish continued typing from a
lost false event. Repeated true is idempotent.

## Delivery and reconnection

- Idle keep-alive comments arrive every ten seconds; do not parse them as JSON.
- No history, event IDs or Last-Event-ID recovery. New clients receive connected
  then new events, without a current text/typing snapshot.
- A slow client falling behind the 100-event broadcast buffer stays connected.
  The server skips missed events, logs their count and continues the same
  stream without replay/recovery.
- There is no skip notification to the client. Derived state must expire itself:
  reset typing after a timeout clearly longer than five seconds.
- A closed stream means server/network loss, not a buffer skip. Reconnection
  provides connected and only subsequent events, without catch-up.
- Browser EventSource reconnects automatically. Do not close it on every
  temporary error if you want retries.
- `401` means fix authentication, not “no new phrases”.

## Template client

Place this inside WebView's own `index.html`. External access first opens the
page with a valid `?token=...` to receive the cookie.

```html
<div id="text-container"></div>
```

```javascript
const output = document.getElementById('text-container');
const events = new EventSource('/sse');
// Longer than the five-second typing refresh interval.
const TYPING_RESET_MS = 12000;
let typingTimeout = null;
function setTyping(typing) {
    document.documentElement.classList.toggle('is-typing', typing);
    if (typingTimeout) {
        clearTimeout(typingTimeout);
        typingTimeout = null;
    }
    if (typing) {
        typingTimeout = setTimeout(() => {
            typingTimeout = null;
            document.documentElement.classList.remove('is-typing');
        }, TYPING_RESET_MS);
    }
}
events.addEventListener('connected', () => setTyping(false));
events.onmessage = (event) => {
    const data = JSON.parse(event.data);
    if (typeof data.text === 'string') output.textContent = data.text;
};
events.addEventListener('typing', (event) => {
    const data = JSON.parse(event.data);
    if (typeof data.typing === 'boolean') setTyping(data.typing);
});
events.onerror = () => {
    setTyping(false);
    console.warn('SSE: connection lost or rejected');
};
window.addEventListener('pagehide', () => events.close());
```

Use textContent so that user text is not executed as HTML. Display duration, styling
and visible history belong to the client. To obtain the cookie explicitly:

```javascript
const token = new URLSearchParams(location.search).get('token');
const response = await fetch(`/auth?token=${encodeURIComponent(token ?? '')}`, {
    credentials: 'same-origin'
});
if (!response.ok) throw new Error(`Authentication: HTTP ${response.status}`);
const events = new EventSource('/sse');
```

Run this on the same origin in an async function/module. It is an alternative
initialization: do not run both examples together; use one EventSource per client.

## PowerShell test

```powershell
curl.exe -N http://127.0.0.1:10100/sse
```

Expect connected first. Send a WebView-routed phrase, then type with typing
delivery enabled for a separate event. Ctrl+C stops the command. External
non-browser clients call `/auth`, retain Set-Cookie and send that cookie to
`/sse`. Dispatch by event name before reading JSON: connected/typing have no text.
