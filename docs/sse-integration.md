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
| **Port** | TCP port for the server | `52704` |

### Network Addresses

- **`0.0.0.0`** - Binds to all network interfaces (LAN + localhost)
- **`127.0.0.1`** - Localhost only (accessible only from this machine)

## SSE Endpoint

### URL

```
http://<bind_address>:<port>/sse
```

For example:
- Localhost: `http://127.0.0.1:52704/sse`
- LAN access: `http://192.168.1.100:52704/sse` (replace with your LAN IP)

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

### Example Connection (JavaScript)

```javascript
const eventSource = new EventSource('http://127.0.0.1:52704/sse');

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

### Example Connection (Python)

```python
import requests
import json

def connect_sse():
    url = 'http://127.0.0.1:52704/sse'
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
curl -N http://127.0.0.1:52704/sse
```

## HTML Page

The root endpoint (`/`) serves an HTML page with embedded CSS that can be used as a browser source in OBS or similar tools.

### URL

```
http://<bind_address>:<port>/
```

### Customization

The HTML and CSS templates are stored in:

- **Windows**: `%APPDATA%\ttsbard\webview\`
  - `index.html` - HTML template (use `{{CSS}}` placeholder for CSS injection)
  - `style.css` - CSS stylesheet

You can edit these files to customize the appearance. Changes take effect when:
1. The server restarts, or
2. You click the "Reload templates" button in the WebView Source panel

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

## Troubleshooting

### Connection Refused

1. Check that the server is enabled in WebView Source settings
2. Verify the port is not already in use by another application
3. Check firewall settings if accessing from another machine

### No Events Received

1. Verify the URL is correct (including `/sse` suffix)
2. Submit some text to TTS to trigger an event
3. Check browser console for connection errors

### Firewall (Windows)

If accessing from another machine on your LAN, you may need to allow the application through Windows Defender Firewall:

1. Open Windows Defender Firewall
2. Click "Allow an app through Windows Defender Firewall"
3. Find "ttsbard" or "TTS Bard"
4. Allow on both Private and Public networks (as appropriate)

## Security Considerations

- The server has **no authentication** - anyone with network access can connect
- On `0.0.0.0`, the server is accessible from your entire LAN
- Only use `127.0.0.1` if you don't need external access
- Consider using a reverse proxy with authentication for production use

## Integration Examples

### OBS Browser Source

1. Enable WebView Source server in app settings
2. Add **Browser Source** in OBS
3. Set URL to: `http://127.0.0.1:52704/`
4. Customize width/height as needed
5. Edit `style.css` to style the text display

### Custom Overlay

Create a custom HTML page that connects to the SSE endpoint and displays text as desired:

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
        const eventSource = new EventSource('http://127.0.0.1:52704/sse');
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
