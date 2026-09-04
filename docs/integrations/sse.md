# SSE: контракт для клиентов

WebView-сервер передаёт текст и состояние набора через Server-Sent Events.
Настройка listener, OBS, шаблонов, сетевого доступа и UPnP описана в
[руководстве WebView](./webview.md). Здесь приведён контракт для собственного клиента.

## HTTP endpoints

| Запрос | Результат |
|---|---|
| `GET /` | HTML-шаблон с подставленным CSS. Для публичного клиента требуется cookie или query-параметр `token`; иначе `401`. Правильный query-токен также устанавливает cookie. |
| `GET /auth?token=...` | При совпадении с настроенным токеном — `200` и cookie `webview_auth`; при отсутствующем или неверном токене — `401`. |
| `GET /sse` | При разрешённом доступе — `200`, `Content-Type: text/event-stream`; иначе `401`. |

Cookie имеет `HttpOnly; Path=/; SameSite=Lax`. `/sse` проверяет cookie для
публичного клиента; передача `?token=...` прямо в SSE URL не заменяет её.
Локальные адреса освобождены от этой проверки по правилам WebView.

Браузерный клиент должен работать на том же origin: схема, хост и порт совпадают.
Сервер не добавляет CORS-заголовков. Для другого origin нужен отдельно настроенный
proxy; простого `withCredentials: true` недостаточно.

## События

Каждое событие завершается пустой строкой. JSON передаётся в `data:`.

### Подключение

Сразу после открытия потока приходит именованное событие:

```text
event: connected
data: {}

```

Это подтверждение подключения, а не текстовая реплика.

### Текст

Текст приходит без поля `event:` и обрабатывается через `onmessage`:

```text
data: {"text":"Привет!"}

```

Поле `text` содержит строку для отображения. Событие относится к маршруту
WebView и не подтверждает завершение синтеза или воспроизведения. Входящие
HTTP/OCR не транслируются автоматически; Twitch не служит источником входящих
реплик для этого потока.

### Набор текста

```text
event: typing
data: {"typing":true}

```

`true` означает активный набор, `false` — его окончание. Событие зависит от
настройки передачи набора в редакторе. Именованные события не вызывают
`onmessage`, поэтому старый текстовый клиент продолжает работать.

## Доставка и переподключение

- При простое сервер отправляет keep-alive комментарии с интервалом 10 секунд;
  их не нужно разбирать как JSON.
- История, номера `id` и восстановление по `Last-Event-ID` не реализованы.
  Новый клиент получает `connected`, затем только новые события; текущий текст
  и последнее состояние набора отдельным snapshot не отправляются.
- Медленный клиент может потерять соединение при переполнении внутреннего
  broadcast-буфера. После переподключения пропущенные события не повторяются.
- Браузерный `EventSource` поддерживает переподключение. Не вызывайте `close()`
  при каждой временной ошибке, если хотите сохранить эту возможность.
- `401` требует исправить авторизацию; это не сигнал об отсутствии новых реплик.

## Клиент в шаблоне

Этот пример размещается внутри `index.html`, обслуживаемого самим WebView.
Для внешнего доступа сначала откройте страницу с правильным `?token=...`,
чтобы получить cookie. Элемент для текста:

```html
<div id="text-container"></div>
```

Клиентский код:

```javascript
const output = document.getElementById('text-container');
const events = new EventSource('/sse');

events.addEventListener('connected', () => {
    document.documentElement.classList.remove('is-typing');
});

events.onmessage = (event) => {
    const data = JSON.parse(event.data);
    if (typeof data.text === 'string') output.textContent = data.text;
};

events.addEventListener('typing', (event) => {
    const data = JSON.parse(event.data);
    document.documentElement.classList.toggle('is-typing', data.typing === true);
});

events.onerror = () => {
    document.documentElement.classList.remove('is-typing');
    console.warn('SSE: соединение потеряно или отклонено');
};

window.addEventListener('pagehide', () => events.close());
```

Используйте `textContent` для реплики, чтобы текст пользователя не исполнялся
как HTML. Таймер скрытия, оформление и история отображения принадлежат клиенту.

Если клиенту нужно явно получить cookie перед подключением:

```javascript
const token = new URLSearchParams(location.search).get('token');
const response = await fetch(`/auth?token=${encodeURIComponent(token ?? '')}`, {
    credentials: 'same-origin'
});
if (!response.ok) throw new Error(`Авторизация: HTTP ${response.status}`);
const events = new EventSource('/sse');
```

Этот вариант также предполагает тот же origin и выполняется в async-функции
или модульном скрипте. Не добавляйте его параллельно первому примеру: нужен
один `EventSource` на клиент.

## Проверка из PowerShell

На компьютере с TTSBard:

```powershell
curl.exe -N http://127.0.0.1:10100/sse
```

Сначала должен появиться `connected`. Затем отправьте фразу с маршрутом WebView;
набор текста при включённой передаче даст отдельное событие `typing`.
Остановите команду через `Ctrl+C`.

Небраузерный внешний клиент сначала вызывает `/auth`, сохраняет `Set-Cookie`,
а затем отправляет эту cookie в `/sse`. Он должен разбирать имя события до
обработки JSON: поле `text` отсутствует у `connected` и `typing`.
