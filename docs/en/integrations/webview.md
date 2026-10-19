**English | [Русский](../../ru/integrations/webview.md)**

# WebView: OBS overlay

The WebView server displays TTSBard phrases in a browser/OBS source. It sends
text/typing; audio uses TTSBard devices. See [SSE](sse.md) for custom clients.

## Connect OBS on this PC

1. Open **WebView** in the integrations section of the sidebar.
2. Set the port if needed (default `10100`); stop a running server first.
3. Click Start; edited settings are saved before starting. Wait for running status.
4. Copy **On this PC**. Add an OBS Browser source with
   `http://127.0.0.1:10100/` and set source dimensions.
5. Send an editor phrase using a route including WebView.

Voice only excludes the overlay. For HTTP/OCR choose a WebView-enabled
[Incoming route](../user/incoming.md#automatic-acceptance); their default is
audio only. The panel also offers a test send.

## Server settings

| Setting | Behavior |
|---|---|
| Port | Default `10100`; must be free |
| Start with the app | Off by default; enables startup on the next launch |
| Status | Starting, running, stopped or error; only running confirms the port opened |

`0.0.0.0` is a bind address, not a browser URL. Another PC uses TTSBard's LAN IP,
for example `http://192.168.1.10:10100/`. The server always binds `0.0.0.0`;
there is no address setting. Legacy `bind_address` is ignored and disappears
on the next save. Stop to edit the disabled port field. Stopping does not save
draft edits; the next start applies/saves them.

## Network access

### Local network

Local ranges connect without a token: IPv4 loopback `127.0.0.0/8`, private
`10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`, link-local `169.254.0.0/16`,
and IPv6 `::1` / `fc00::/7` if the request reaches the listener.
Allow incoming TTSBard connections in Windows Firewall for the network profile
if needed. The panel shows local and LAN URLs separately; retry LAN address
detection if unavailable. Local access remains usable.

### External access

1. Start the server.
2. Generate an access token in the external-access block.
3. Forward the TCP port manually on the router or enable UPnP.
4. Use `http://<external-IP>:10100/?token=<token>` from the panel.

Public clients need a valid token for page/SSE access. A valid page query token
sets a cookie for SSE; there is no separate session key. The token is saved in
settings and sent in the URL/cookie. HTTP is unencrypted; use HTTPS on a reverse
proxy for protected external access and do not publish token-bearing links.
Locality is determined from the immediate TCP client's address: a local proxy
must enforce its own external authentication, or it can fall under the LAN bypass.

### UPnP

UPnP requires a token. TTSBard discovers a gateway and requests the same TCP
port for one hour, without automatic renewal. Check the router for long sessions
or configure forwarding manually. A checked box expresses intent, not confirmed
forwarding. Notifications distinguish success, saved-but-failed forwarding with
a reason, and saved-until-server-start. Failures include missing gateway,
`add_port` rejection or timeout; forwarding retries at the next server start.
Disabling UPnP/stopping the server attempts to remove the rule. Unsupported
routers, double NAT or network restrictions can require different routing;
the checkbox alone does not prove internet reachability.

## Overlay appearance

Use Open folder in WebView:

```text
%APPDATA%\ttsbard\webview\
  index.html
  style.css
```

`index.html` contains HTML/client JavaScript; `{{CSS}}` is replaced with
`style.css`. After editing, refresh templates in TTSBard, then reload the OBS
source/browser page. Existing templates are not overwritten at update/restart;
defaults are created only when missing. Add new template capabilities manually.

```css
#text-container {
    font-size: 48px;
    color: #ffffff;
    text-shadow: 2px 2px 4px #000000;
}
```

The default toggles `is-typing` on `<html>`, without imposing a visual effect.
Use it in CSS; [SSE's template example](sse.md#template-client) supplies a handler
for old templates. While typing, the server repeats true at least every five
seconds. The default removes the class after 12 seconds without refreshes so that a
lost false event does not leave it stuck. Add this timeout manually to older copies.

## Troubleshooting

| Symptom | Check |
|---|---|
| Server error | Port occupied: stop the other listener or choose a free port |
| OBS cannot load | Running status, address/port/firewall; localhost on another PC points to that PC |
| Page but no phrase | WebView-enabled editor/Incoming route; Voice only excludes it |
| No typing indicator | Editor typing switch, handler and template CSS |
| External `401` | Reopen with current token; custom clients use SSE cookie authentication |
| Custom HTML gets CORS errors | Serve it from WebView's origin; arbitrary cross-origin access is not allowed |
| CSS changes missing | Refresh in TTSBard, then reload OBS |

See [SSE contract](sse.md) for endpoints, delivery limits and examples.
