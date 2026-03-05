/// Default HTML template with {{CSS}} and {{JS}} placeholders
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
    <div id="text-container"></div>
    <script>{{JS}}</script>
</body>
</html>"#.to_string()
}

/// Default CSS for centered white text with shadow
pub fn default_css() -> String {
    r#"body {
    margin: 0;
    padding: 0;
    background: transparent;
    display: flex;
    justify-content: center;
    align-items: center;
    min-height: 100vh;
}

#text-container {
    font-family: 'Arial', sans-serif;
    font-size: 48px;
    color: #ffffff;
    text-shadow: 2px 2px 4px rgba(0, 0, 0, 0.8);
    text-align: center;
    padding: 20px;
}"#.to_string()
}

/// Default JavaScript with WebSocket client and typewriter effect
pub fn default_js() -> String {
    r#"const ws = new WebSocket(`ws://${location.host}/ws`);
let currentText = '';
let charIndex = 0;
let timeoutId = null;

ws.onmessage = (event) => {
    const data = JSON.parse(event.data);
    if (data.type === 'text') {
        typeWriter(data.text);
    }
};

function typeWriter(text) {
    // Stop current animation
    if (timeoutId) {
        clearTimeout(timeoutId);
    }

    currentText = text;
    charIndex = 0;

    const container = document.getElementById('text-container');
    container.textContent = '';

    function type() {
        if (charIndex < currentText.length) {
            container.textContent += currentText.charAt(charIndex);
            charIndex++;
            timeoutId = setTimeout(type, {{SPEED}});
        }
    }

    type();
}

ws.onclose = () => {
    console.log('WebSocket disconnected, attempting to reconnect...');
    setTimeout(() => {
        location.reload();
    }, 2000);
};

ws.onerror = (error) => {
    console.error('WebSocket error:', error);
};"#.to_string()
}
