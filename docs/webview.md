# WebView Server

The WebView Server provides real-time text display via HTTP with Server-Sent Events (SSE), designed for integration with OBS, browser sources, and custom overlays.

## Features

- **Real-time text broadcasting** - SSE endpoint for live text updates
- **Customizable HTML/CSS** - Edit templates to match your overlay style
- **Security** - Token-based authentication for external access
- **Local network bypass** - No authentication required for LAN connections
- **Auto-start** - Option to start server on application launch

## Quick Start

### For Local Use (OBS on same machine)

1. Open **WebView Source** panel
2. Enable the server (click **Start** button)
3. Copy the **Local URL (OBS)**
4. In OBS, add **Browser Source** with the URL
5. Done! Text will appear in real-time

### For External Access (Different machine/internet)

1. Open **WebView Source → Security**
2. Click **Generate Token** to create access token
3. Enable **UPnP** (if supported by your router) or set up manual port forwarding
4. Copy the **External URL**
5. Use URL in OBS or browser: `http://<your-ip>:<port>/?token=<token>`

## Configuration

### Server Settings

| Setting | Description | Default |
|---------|-------------|---------|
| **Enabled** | Server on/off status | `false` |
| **Start on boot** | Auto-start when app launches | `false` |
| **Bind address** | Network interface (0.0.0.0 = all, 127.0.0.1 = local only) | `0.0.0.0` |
| **Port** | TCP port for the server | `10100` |

### Security Settings

| Setting | Description | Default |
|---------|-------------|---------|
| **Access Token** | Token for external access (UUID v4) | None |
| **UPnP** | Automatic port forwarding on router | `false` |

## API Endpoints

### GET `/`

Serves the HTML page with embedded SSE client.

**Query Parameters:**
- `token` (optional) - Access token for external connections

**Response:** HTML page with embedded CSS and SSE JavaScript

### GET `/auth?token=<token>`

Authenticates the client and sets an HttpOnly cookie.

**Query Parameters:**
- `token` (required) - Access token

**Response:**
- `200 OK` - Authentication successful, `Set-Cookie` header included
- `401 Unauthorized` - Invalid or missing token

### GET `/sse`

SSE endpoint for receiving real-time text updates.

**Authentication:**
- **Local networks** (192.168.x.x, 10.x.x.x, 127.0.0.1): No auth required
- **External networks**: Requires valid auth cookie

**Response:**
- `200 OK` - SSE stream with `text/event-stream` content type
- `401 Unauthorized` - Authentication failed (external connections only)

**Event Format:**
```
data: {"text": "Your message here"}
```

## Security Model

### Access Control by IP Range

The server automatically detects local network IPs and bypasses authentication:

| IP Range | Type | Auth Required |
|----------|------|---------------|
| `127.0.0.1` | Loopback | ❌ No |
| `192.168.0.0/16` | Private Class C | ❌ No |
| `10.0.0.0/8` | Private Class A | ❌ No |
| `172.16.0.0/12` | Private Class B | ❌ No |
| `169.254.0.0/16` | Link-local | ❌ No |
| `::1` | IPv6 Loopback | ❌ No |
| `fc00::/7` | IPv6 Unique Local | ❌ No |
| All other IPs | Public | ✅ Yes |

### Authentication Flow

**External Access Flow:**

```
1. Client opens: http://server:10100/?token=uuid
   ↓
2. Page loads with token in URL
   ↓
3. JavaScript calls /auth?token=uuid
   ↓
4. Server validates token (constant-time compare)
   ↓
5. Server sets HttpOnly cookie: webview_auth=uuid
   ↓
6. SSE connection includes credentials
   ↓
7. Server validates cookie on each SSE connection
```

### Security Features

1. **Constant-time token comparison** - Prevents timing attacks using `subtle` crate
2. **HttpOnly cookies** - JavaScript cannot read auth cookies
3. **SameSite=Lax** - Prevents CSRF attacks
4. **UUID v4 tokens** - Cryptographically random tokens
5. **Token regeneration** - Invalidate old tokens by generating new ones
6. **No credential leakage** - Tokens stored only in local settings file

## Templates

### File Locations

Templates are stored in your config directory:

- **Windows**: `%APPDATA%\ttsbard\webview\`
- **Linux**: `~/.config/ttsbard/webview/`
- **macOS**: `~/Library/Application Support/ttsbard/webview/`

### Files

- **`index.html`** - HTML template with `{{CSS}}` placeholder
- **`style.css`** - CSS stylesheet (injected into HTML)

### Editing Templates

1. Click **Open Folder** in WebView Source panel
2. Edit HTML or CSS files in your preferred editor
3. Click **Reload** in the app to apply changes
4. Refresh the browser source in OBS

### Default Template Structure

**HTML** (`index.html`):
- Contains `{{CSS}}` placeholder for CSS injection
- Embedded JavaScript for SSE connection
- Token-based authentication support
- Auto-reconnection on connection loss

**CSS** (`style.css`):
- Centered white text with black shadow
- Fade in/out animations
- Transparent background for OBS overlays

## Customization Examples

### Different Text Styles

**Large Bold Text:**
```css
#text-container {
    font-size: 64px;
    font-weight: bold;
    color: #00ff00;
    text-shadow: 3px 3px 6px rgba(0,0,0,0.9);
}
```

**Glowing Text:**
```css
#text-container {
    font-size: 48px;
    color: #ffffff;
    text-shadow:
        0 0 10px #fff,
        0 0 20px #fff,
        0 0 30px #00ffff,
        0 0 40px #00ffff;
}
```

**Bottom-Positioned Text:**
```css
body {
    display: flex;
    justify-content: center;
    align-items: flex-end;  /* Bottom instead of center */
    min-height: 100vh;
    padding-bottom: 50px;
}
```

### Multiple Text Lines

Modify the HTML template to show multiple lines:

```html
<div id="text-history">
    <div class="line line-1"></div>
    <div class="line line-2"></div>
    <div class="line line-3"></div>
</div>

<script>
    const lines = document.querySelectorAll('.line');
    let currentIndex = 0;

    evtSource.onmessage = (event) => {
        const data = JSON.parse(event.data);

        // Shift lines up
        lines[currentIndex].textContent = data.text;
        lines[currentIndex].classList.add('visible');

        currentIndex = (currentIndex + 1) % 3;

        // Hide old line
        lines[currentIndex].classList.remove('visible');
    };
</script>
```

## Tauri Commands

The WebView server exposes the following Tauri commands:

### Server Management

- `get_webview_settings()` - Get all server settings
- `get_webview_enabled()` - Get server enabled status
- `get_webview_start_on_boot()` - Get auto-start on boot status
- `get_webview_port()` - Get server port
- `get_webview_bind_address()` - Get bind address
- `save_webview_settings(settings)` - Save and apply settings
- `get_local_ip()` - Get local IP address
- `open_template_folder()` - Open templates folder in file explorer
- `reload_templates()` - Reload templates without server restart
- `send_test_message(text)` - Send test message to SSE

### Security

- `generate_webview_token()` - Generate new access token
- `get_webview_token()` - Get masked access token (first 8 chars)
- `copy_webview_token()` - Copy access token to clipboard
- `regenerate_webview_token()` - Regenerate access token
- `set_webview_upnp_enabled(enabled)` - Enable/disable UPnP
- `get_webview_upnp_enabled()` - Get UPnP status
- `get_external_ip()` - Get public IP address

## Port Forwarding

### Manual Port Forwarding

If UPnP is not available or not working:

1. Find your router's gateway IP (usually `192.168.1.1` or `192.168.0.1`)
2. Login to router admin panel
3. Find **Port Forwarding** or **NAT** settings
4. Add rule:
   - **External port**: `10100` (or your configured port)
   - **Internal port**: `10100`
   - **Protocol**: TCP
   - **Internal IP**: Your computer's LAN IP (check in app settings)
5. Save and apply

### UPnP (Automatic)

The UPnP feature automatically forwards the configured port on UPnP-enabled routers.

**Requirements:**
- UPnP enabled on router
- Router supports UPnP IGD protocol
- Device on same network as router

**To enable:**
1. Toggle **UPnP** switch in WebView Source → Security
2. Port forwarding is applied immediately (no server restart required)
3. Verify in router admin panel that rule was created

**Notes:**
- Port mapping uses 1-hour lease and is refreshed automatically
- Mapping is removed when server stops or UPnP is disabled
- Falls back gracefully if UPnP is unavailable

## Troubleshooting

### Server Won't Start

**"Address already in use":**
- Another application is using the port
- Try a different port in settings
- Check for existing server instances

**"Permission denied":**
- Port < 1024 requires administrator privileges
- Use port 1024 or higher

### SSE Connection Issues

**"401 Unauthorized":**
- External connection without token
- Generate token in Security section
- Include token in URL: `/?token=<your-token>`

**Connection drops:**
- Check network connectivity
- Verify firewall settings
- External connections require valid cookie

### OBS Display Issues

**No text appearing:**
- Verify server is running
- Check OBS browser source URL
- Send test message from app
- Check OBS browser source width/height

**Text not styled:**
- Verify `style.css` exists in template folder
- Click **Reload** button after editing templates
- Refresh OBS browser source

### Template Changes Not Applying

1. Click **Reload** button in WebView Source panel
2. Right-click OBS browser source → **Refresh**
3. If still not working, restart server

## Firewall Configuration

### Windows Defender Firewall

1. Open **Windows Defender Firewall**
2. Click **Allow an app through Windows Defender Firewall**
3. Find **ttsbard** or **TTS Bard**
4. Allow on:
   - **Private** (for LAN access)
   - **Public** (only if needed for external access)

### Third-Party Firewall

Allow inbound connections on:
- Port: `10100` (or your configured port)
- Protocol: TCP
- Application: `ttsbard.exe`

## Advanced Configuration

### Custom HTML Templates

The `{{CSS}}` placeholder allows dynamic CSS injection:

```html
<!DOCTYPE html>
<html>
<head>
    <title>TTSBard WebView</title>
    <style>{{CSS}}</style>
    <link rel="stylesheet" href="https://example.com/custom.css">
</head>
<body>
    <div id="text-container"></div>
    <script src="https://example.com/custom.js"></script>
    <script>
        // Custom SSE handling
        const evtSource = new EventSource('/sse');
        // ... custom logic
    </script>
</body>
</html>
```

### Reverse Proxy (HTTPS)

For production use, consider a reverse proxy with SSL:

**Nginx Example:**
```nginx
server {
    listen 443 ssl;
    server_name tts.example.com;

    ssl_certificate /path/to/cert.pem;
    ssl_certificate_key /path/to/key.pem;

    location / {
        proxy_pass http://localhost:10100;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

## Architecture

### Server Structure

```
WebViewServer
├── Settings (Arc<RwLock<WebViewSettings>>)
├── SSE Sender (broadcast::Sender<String>)
├── Templates (TemplateCache)
└── UPnP Manager (Option<Arc<UpnpManager>>)
```

### Event Flow

```
User Input / Integration
        ↓
TextSentToTts event
        ↓
WebViewServer::broadcast_text()
        ↓
SSE broadcast
        ↓
Connected clients receive {"text": "..."}
```

### Security Flow

```
External Connection Request
        ↓
Check IP address
        ↓
Is local network?
    ├── YES → Allow connection
    └── NO → Check cookie
            ├── Has valid cookie? → Allow
            └── No/Invalid cookie → 401 Unauthorized
```

## Best Practices

### Security

1. **Local only when possible** - Use `127.0.0.1` bind address if external access not needed
2. **Strong tokens** - Generated UUID v4 tokens are cryptographically random
3. **Token regeneration** - Regenerate tokens if unauthorized access is suspected
4. **Firewall rules** - Only allow necessary IPs/networks

### Performance

1. **Reasonable keep-alive** - SSE keep-alive set to 10 seconds
2. **Broadcast limit** - Channel capacity of 100 concurrent clients
3. **Template caching** - Templates loaded once and cached in memory

### Reliability

1. **Auto-reconnection** - Built-in retry logic in JavaScript client
2. **Graceful degradation** - Failed auth shows error message
3. **Hot reload** - Templates reload without server restart

---

*Last updated: 2026-04-15*
