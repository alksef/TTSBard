# SSE Integration Guide

This document describes the Server-Sent Events (SSE) endpoint provided by the application for external integrations (e.g., OBS, browser sources, custom tools).

## Overview

The application runs a built-in HTTP server that broadcasts TTS text via SSE. This allows external tools to receive real-time text updates as they are sent to TTS.

## Server Configuration

The server can be configured in the **WebView Source** panel:

| Setting | Description | Default |
|---------|-------------|---------|
| **Enabled** | Whether the server is running | `false` |
| **Start on boot** | Auto-start server when app launches | `false` |
| **Bind address** | Network interface to bind to | `0.0.0.0` (all interfaces) |
| **Port** | TCP port for the server | `10100` |

### Network Addresses

- **`0.0.0.0`** - Binds to all network interfaces (LAN + localhost)
- **`127.0.0.1`** - Localhost only (accessible only from this machine)

## Security

### Access Control

The server implements IP-based access control:

| Network Type | Access | Authentication |
|--------------|--------|----------------|
| **Local networks** (127.0.0.1, 192.168.x.x, 10.x.x.x, 172.16-31.x.x) | ✅ Allowed | No token required |
| **External networks** (public IP) | ❌ Denied by default | Token required |

### Token Authentication

For external access, you must generate an access token in the **WebView Source → Security** section:

1. Click **"Generate Token"** to create a new access token
2. The token will be stored in your settings
3. Access external URLs using: `http://<your-ip>:<port>/?token=<your-token>`

### Security Features

- **Constant-time token comparison** - prevents timing attacks
- **HttpOnly cookies** - tokens stored in cookies are not accessible via JavaScript
- **SameSite=Lax** - prevents CSRF attacks
- **Session key regeneration** - can invalidate all external sessions at once

## SSE Endpoint

### URL

```
http://<bind_address>:<port>/sse
```

For example:
- Localhost: `http://127.0.0.1:10100/sse`
- LAN access: `http://192.168.1.100:10100/sse` (replace with your LAN IP)

### Authentication

For **local network access**, no authentication is required:

```javascript
const eventSource = new EventSource('http://127.0.0.1:10100/sse');
```

For **external access**, you must authenticate first:

```javascript
// Step 1: Authenticate with token
await fetch('/auth?token=your-token-here', { credentials: 'include' });

// Step 2: Connect to SSE with credentials
const eventSource = new EventSource('http://your-ip:10100/sse', {
    withCredentials: true
});
```

### Connection Details

| Property | Value |
|----------|-------|
| **Protocol** | HTTP/1.1 |
| **Method** | GET |
| **Content-Type** | `text/event-stream` |
| **Cache-Control** | `no-cache` |
| **Keep-Alive** | 10 seconds |

### Event Format

Each SSE event contains a JSON payload:

```json
{"text": "Hello, world!"}
```

- The event `data` field contains the JSON string
- No event names are used (default event)
- No event IDs are used

### Example Connection (JavaScript - Local)

```javascript
const eventSource = new EventSource('http://127.0.0.1:10100/sse');

eventSource.onmessage = function(event) {
    const data = JSON.parse(event.data);
    console.log('Received text:', data.text);
    // Do something with the text...
};

eventSource.onerror = function(error) {
    console.error('SSE connection error:', error);
    eventSource.close();
};
```

### Example Connection (JavaScript - External with Auth)

```javascript
const token = 'your-token-here';
const baseUrl = 'http://your-public-ip:10100';

// Step 1: Authenticate and set cookie
fetch(`${baseUrl}/auth?token=${encodeURIComponent(token)}`, {
    credentials: 'include'
})
.then(response => {
    if (response.ok) {
        console.log('Authentication successful');
        // Step 2: Connect to SSE
        const eventSource = new EventSource(`${baseUrl}/sse`, {
            withCredentials: true
        });

        eventSource.onmessage = function(event) {
            const data = JSON.parse(event.data);
            console.log('Received text:', data.text);
        };

        eventSource.onerror = function(error) {
            if (eventSource.readyState === EventSource.CLOSED) {
                console.error('Authentication failed or connection closed');
            }
        };
    } else {
        console.error('Authentication failed');
    }
});
```

### Example Connection (Python)

```python
import requests
import json

def connect_sse():
    url = 'http://127.0.0.1:10100/sse'
    with requests.get(url, stream=True) as response:
        response.raise_for_status()
        for line in response.iter_lines():
            if line:
                line_str = line.decode('utf-8')
                if line_str.startswith('data:'):
                    json_str = line_str[5:].strip()
                    if json_str:
                        data = json.loads(json_str)
                        print('Received text:', data['text'])
```

### Example Connection (cURL)

```bash
curl -N http://127.0.0.1:10100/sse
```

## HTML Page

The root endpoint (`/`) serves an HTML page with embedded CSS that can be used as a browser source in OBS or similar tools.

### URL

```
http://<bind_address>:<port>/
```

For external access with token:
```
http://<bind_address>:<port>/?token=<your-token>
```

### Customization

The HTML and CSS templates are stored in:

- **Windows**: `%APPDATA%\ttsbard\webview\`
  - `index.html` - HTML template (use `{{CSS}}` placeholder for CSS injection)
  - `style.css` - CSS stylesheet

You can edit these files to customize the appearance. Changes take effect when:
1. The server restarts, or
2. You click the **"Reload templates"** button in the WebView Source panel

## Broadcasting

The SSE endpoint broadcasts text when:

1. Text is submitted from the main input panel
2. Text is received from Twitch chat (if Twitch integration is enabled)
3. Text is received from any other integration source

### Event Flow

```
User Input / Integration
         ↓
  AppEvent::TextSentToTts
         ↓
  WebViewServer::broadcast_text()
         ↓
  SSE broadcast to all connected clients
```

## API Endpoints

### GET `/`

Serves the HTML page with embedded SSE client.

### GET `/auth?token=<token>`

Authenticates the client and sets an HttpOnly cookie.

**Parameters:**
- `token` - Access token (required for external connections)

**Response:**
- `200 OK` - Authentication successful, cookie set
- `401 Unauthorized` - Invalid or missing token

### GET `/sse`

SSE endpoint for receiving real-time text updates.

**Headers:**
- Local connections: No special headers needed
- External connections: Must include auth cookie

**Response:**
- `200 OK` - SSE stream established
- `401 Unauthorized` - Authentication failed (external connections only)

## Troubleshooting

### Connection Refused

1. Check that the server is enabled in WebView Source settings
2. Verify the port is not already in use by another application
3. Check firewall settings if accessing from another machine

### No Events Received

1. Verify the URL is correct (including `/sse` suffix)
2. Submit some text to TTS to trigger an event
3. Check browser console for connection errors

### 401 Unauthorized

For external connections:
1. Make sure you have generated an access token
2. Include the token in the URL: `/?token=<your-token>`
3. Call `/auth?token=<your-token>` before connecting to SSE
4. Include `credentials: 'include'` when fetching and creating EventSource

### Firewall (Windows)

If accessing from another machine on your LAN, you may need to allow the application through Windows Defender Firewall:

1. Open Windows Defender Firewall
2. Click "Allow an app through Windows Defender Firewall"
3. Find "ttsbard" or "TTS Bard"
4. Allow on both Private and Public networks (as appropriate)

## UPnP Port Forwarding

The application supports automatic UPnP port forwarding for external access:

1. Enable UPnP in **WebView Source → Security**
2. The application will attempt to forward the configured port on your router
3. **Note:** UPnP must be enabled on your router for this to work

### Limitations

- UPnP implementation is currently a stub - manual port forwarding may be required
- Check your router's admin panel to verify the port forwarding rule was created

## Integration Examples

### OBS Browser Source (Local)

1. Enable WebView Source server in app settings
2. Add **Browser Source** in OBS
3. Set URL to: `http://127.0.0.1:10100/`
4. Customize width/height as needed
5. Edit `style.css` to style the text display

### OBS Browser Source (External)

1. Generate access token in **WebView Source → Security**
2. Add **Browser Source** in OBS
3. Set URL to: `http://your-public-ip:10100/?token=<your-token>`
4. OR use local URL and set up port forwarding on your router

### Custom Overlay (Local)

Create a custom HTML page that connects to the SSE endpoint:

```html
<!DOCTYPE html>
<html>
<head>
    <title>TTS Overlay</title>
    <style>
        body { background: transparent; }
        #tts-text {
            font-family: Arial, sans-serif;
            font-size: 32px;
            color: white;
            text-shadow: 2px 2px 4px black;
            text-align: center;
        }
    </style>
</head>
<body>
    <div id="tts-text"></div>
    <script>
        const eventSource = new EventSource('http://127.0.0.1:10100/sse');
        const display = document.getElementById('tts-text');

        eventSource.onmessage = function(event) {
            const data = JSON.parse(event.data);
            display.textContent = data.text;
            // Auto-clear after 5 seconds
            setTimeout(() => display.textContent = '', 5000);
        };
    </script>
</body>
</html>
```

### Custom Overlay (External with Auth)

```html
<!DOCTYPE html>
<html>
<head>
    <title>TTS Overlay</title>
    <style>
        body { background: transparent; }
        #tts-text {
            font-family: Arial, sans-serif;
            font-size: 32px;
            color: white;
            text-shadow: 2px 2px 4px black;
            text-align: center;
        }
    </style>
</head>
<body>
    <div id="tts-text"></div>
    <script>
        const token = new URLSearchParams(window.location.search).get('token');
        const baseUrl = window.location.origin;

        if (token) {
            // Authenticate first
            fetch(`${baseUrl}/auth?token=${encodeURIComponent(token)}`, {
                credentials: 'include'
            })
            .then(response => {
                if (response.ok) {
                    // Connect to SSE
                    const eventSource = new EventSource(`${baseUrl}/sse`, {
                        withCredentials: true
                    });
                    const display = document.getElementById('tts-text');

                    eventSource.onmessage = function(event) {
                        const data = JSON.parse(event.data);
                        display.textContent = data.text;
                        setTimeout(() => display.textContent = '', 5000);
                    };

                    eventSource.onerror = function(error) {
                        display.textContent = 'Connection error';
                    };
                }
            });
        } else {
            document.getElementById('tts-text').textContent = 'No token provided';
        }
    </script>
</body>
</html>
```

## Security Best Practices

1. **Local use only** - Bind to `127.0.0.1` if you don't need external access
2. **Use strong tokens** - Generate tokens are UUID v4 (sufficiently random)
3. **Regenerate tokens periodically** - Use the "Regenerate token" button
4. **Regenerate session key** if you suspect unauthorized access
5. **Keep software updated** - Security patches are important
6. **Use HTTPS in production** - Consider a reverse proxy with SSL termination

---

*Last updated: 2026-04-15*
