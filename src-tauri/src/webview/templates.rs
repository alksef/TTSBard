/// Default HTML template with {{CSS}} placeholder and inline SSE JavaScript
pub fn default_html() -> String {
    r#"<!DOCTYPE html>
<html lang="ru">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>TTSBard WebView</title>
    <style>{{CSS}}</style>
</head>
<body>
    <div class="wrapper">
        <div id="connection-indicator" class="connection-indicator" title="Подключение"></div>
        <div id="text-container" class="text-container">
            <div class="text-content"></div>
        </div>
    </div>
    <script>
        // Get token from URL query parameter
        const urlParams = new URLSearchParams(window.location.search);
        const token = urlParams.get('token');

        // Create SSE connection with credentials for cookie support
        const evtSource = new EventSource('/sse', {
            withCredentials: true
        });
        const container = document.getElementById('text-container');
        const textContent = container.querySelector('.text-content');
        const connectionIndicator = document.getElementById('connection-indicator');
        let hideTimeout = null;
        let reconnectAttempts = 0;
        const MAX_RECONNECT_ATTEMPTS = 5;
        const DISPLAY_DURATION = 8000;

        // Connection status
        function updateConnectionStatus(status) {
            connectionIndicator.className = 'connection-indicator ' + status;
        }

        evtSource.onopen = () => {
            console.log('SSE connection established');
            updateConnectionStatus('connected');
        };

        evtSource.onmessage = (event) => {
            const data = JSON.parse(event.data);
            showText(data.text);
            reconnectAttempts = 0;
            updateConnectionStatus('connected');
        };

        function showText(text) {
            if (hideTimeout) clearTimeout(hideTimeout);

            // Split text into words for better animation
            const words = text.split(' ');
            textContent.innerHTML = words.map(word => `<span class="word">${word}</span>`).join(' ');

            container.classList.remove('visible');
            container.classList.add('updating');

            // Trigger reflow
            void container.offsetWidth;

            container.classList.remove('updating');
            container.classList.add('visible');

            // Animate words sequentially
            const wordElements = textContent.querySelectorAll('.word');
            wordElements.forEach((word, index) => {
                setTimeout(() => {
                    word.classList.add('animate');
                }, index * 30);
            });

            hideTimeout = setTimeout(() => {
                container.classList.remove('visible');
            }, DISPLAY_DURATION);
        }

        evtSource.onerror = async (error) => {
            console.error('SSE error:', error);
            updateConnectionStatus('disconnected');

            if (evtSource.readyState === EventSource.CLOSED) {
                reconnectAttempts++;

                if (reconnectAttempts <= MAX_RECONNECT_ATTEMPTS && token) {
                    console.log('Attempting to authenticate and reconnect...');
                    updateConnectionStatus('reconnecting');

                    try {
                        const resp = await fetch('/auth?token=' + encodeURIComponent(token), {
                            credentials: 'include'
                        });

                        if (resp.ok) {
                            console.log('Authentication successful, reloading page...');
                            window.location.reload();
                        } else {
                            console.error('Authentication failed:', await resp.text());
                            textContent.textContent = 'Ошибка авторизации';
                            container.classList.add('visible');
                            updateConnectionStatus('error');
                        }
                    } catch (e) {
                        console.error('Auth request error:', e);
                    }
                } else if (reconnectAttempts > MAX_RECONNECT_ATTEMPTS) {
                    console.error('Max reconnection attempts reached');
                    textContent.textContent = 'Не удалось подключиться';
                    container.classList.add('visible');
                    updateConnectionStatus('error');
                }
            }
        };
    </script>
</body>
</html>"#.to_string()
}

/// Default CSS for centered white text with shadow and fade animation
pub fn default_css() -> String {
    r#"* {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
}

body {
    margin: 0;
    padding: 0;
    background: transparent;
    display: flex;
    justify-content: center;
    align-items: center;
    min-height: 100vh;
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
    overflow: hidden;
}

.wrapper {
    position: relative;
    width: 100%;
    height: 100vh;
    display: flex;
    justify-content: center;
    align-items: center;
}

/* Connection indicator */
.connection-indicator {
    position: fixed;
    top: 16px;
    right: 16px;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.3);
    border: 2px solid rgba(255, 255, 255, 0.5);
    transition: all 0.3s ease;
    z-index: 1000;
}

.connection-indicator.connected {
    background: #4ade80;
    border-color: #22c55e;
    box-shadow: 0 0 10px rgba(74, 222, 128, 0.5);
}

.connection-indicator.disconnected {
    background: #f87171;
    border-color: #ef4444;
    animation: pulse 1s ease-in-out infinite;
}

.connection-indicator.reconnecting {
    background: #fbbf24;
    border-color: #f59e0b;
    animation: pulse 0.5s ease-in-out infinite;
}

.connection-indicator.error {
    background: #ef4444;
    border-color: #dc2626;
}

@keyframes pulse {
    0%, 100% {
        opacity: 1;
    }
    50% {
        opacity: 0.5;
    }
}

/* Text container */
.text-container {
    max-width: 90vw;
    width: 100%;
    padding: 40px;
    opacity: 0;
    transform: translateY(20px) scale(0.95);
    transition: all 0.4s cubic-bezier(0.4, 0, 0.2, 1);
}

.text-container.updating {
    transform: translateY(20px) scale(0.95);
    opacity: 0;
}

.text-container.visible {
    opacity: 1;
    transform: translateY(0) scale(1);
}

.text-content {
    font-size: clamp(32px, 5vw, 72px);
    font-weight: 700;
    color: #ffffff;
    text-align: center;
    line-height: 1.3;
    text-shadow:
        0 2px 4px rgba(0, 0, 0, 0.5),
        0 4px 8px rgba(0, 0, 0, 0.3),
        0 8px 16px rgba(0, 0, 0, 0.2);
}

/* Word animation */
.word {
    display: inline-block;
    opacity: 0;
    transform: translateY(10px);
    transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}

.word.animate {
    opacity: 1;
    transform: translateY(0);
}

/* Responsive adjustments */
@media (max-width: 768px) {
    .text-container {
        padding: 24px;
    }

    .text-content {
        font-size: clamp(24px, 6vw, 48px);
    }

    .connection-indicator {
        top: 12px;
        right: 12px;
        width: 10px;
        height: 10px;
    }
}

/* Long text handling */
.text-content:has(.word:nth-child(10)) {
    font-size: clamp(24px, 4vw, 56px);
}

.text-content:has(.word:nth-child(20)) {
    font-size: clamp(20px, 3vw, 42px);
}"#.to_string()
}
