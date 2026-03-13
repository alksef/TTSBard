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
    <div id="text-container"></div>
    <script>
        const evtSource = new EventSource('/sse');
        const container = document.getElementById('text-container');
        let hideTimeout = null;

        evtSource.onmessage = (event) => {
            const data = JSON.parse(event.data);
            showText(data.text);
        };

        function showText(text) {
            if (hideTimeout) clearTimeout(hideTimeout);
            container.classList.remove('visible');
            void container.offsetWidth;
            container.textContent = text;
            requestAnimationFrame(() => {
                container.classList.add('visible');
            });
            hideTimeout = setTimeout(() => {
                container.classList.remove('visible');
            }, 5000);
        }

        evtSource.onerror = (error) => {
            console.error('SSE error:', error);
            // EventSource will automatically attempt to reconnect
        };
    </script>
</body>
</html>"#.to_string()
}

/// Default CSS for centered white text with shadow and fade animation
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
    opacity: 0;
    transition: opacity 0.5s ease-in-out;
}

#text-container.visible {
    opacity: 1;
}"#.to_string()
}
