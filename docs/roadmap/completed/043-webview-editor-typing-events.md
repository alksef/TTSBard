# ROADMAP-043 — Единый typing-state для VTube Studio и WebView

**Статус:** `completed` — typing-state раздаётся независимым consumers

## Цель

Сделать состояние набора доменным сигналом редактора, а не особенностью VTube
Studio:

```text
CodeMirror user-edit
        ↓
useTypingBurst: idle ↔ typing
        ↓
   ┌────┴─────┐
   ↓          ↓
VTube Studio  WebView SSE
```

Источник создаёт переходы всегда. Список потребителей может быть пустым:
доставка тогда является дешёвым no-op, но семантика burst и таймер не зависят
от включённости конкретной интеграции.

## Семантика typing

- Первая пользовательская правка в idle-состоянии создаёт `typing=true`.
- Последующие правки только перезапускают общий idle timeout из
  «Настройки → Редактор».
- Timeout, Enter, Escape/hide и unmount создают `typing=false`.
- Программные `ExternalUpdate` CodeMirror не запускают typing.
- Наружу передаётся только boolean; текст черновика не передаётся.
- Typing отправляется в WebView независимо от prefix-флага исключения готовой
  фразы. Prefix продолжает влиять только на `TextSentToTts`.

## Независимые потребители

`useTypingBurst` принимает список consumers. У каждого consumer собственная
promise-очередь:

- `true/false` одного consumer всегда упорядочены;
- медленное или сломанное подключение VTube Studio не задерживает WebView;
- ошибка одного consumer не останавливает остальные;
- отсутствие consumers не считается ошибкой.

Frontend не проверяет настройки VTube Studio или WebView перед созданием
сигнала. Каждый backend consumer самостоятельно выполняет no-op, когда его
интеграция выключена или сервер не запущен.

## WebView SSE wire contract

Текст сохраняет существующий безымянный формат без каких-либо изменений:

```text
data: {"text":"Готовая фраза"}
```

Typing передаётся именованным SSE-событием:

```text
event: typing
data: {"typing":true}
```

и:

```text
event: typing
data: {"typing":false}
```

Именованное событие не попадает в `EventSource.onmessage`. Поэтому старые
шаблоны и внешние клиенты, ожидающие обязательный `data.text`, продолжают
работать без изменений.

Запрещено отправлять `{"typing":...}` как безымянный event: текущий шаблон
вызовет `showText(undefined)` и упадёт на `text.split(...)`.

## Шаблоны

Новый встроенный default template подписывается через:

```javascript
evtSource.addEventListener('typing', (event) => {
    const data = JSON.parse(event.data);
    document.documentElement.classList.toggle('is-typing', data.typing === true);
});
```

Stage не задаёт обязательный визуальный индикатор. Класс `is-typing` является
hook для пользовательского CSS/JS.

Существующий `%APPDATA%\ttsbard\webview\index.html` не перезаписывается:

- старый шаблон продолжает показывать текст и игнорирует `typing`;
- пользовательские изменения сохраняются;
- для использования typing в старом шаблоне пользователь вручную добавляет
  listener из документации;
- `reload_templates` только перечитывает файлы и не выполняет миграцию.

На `open`/`error` новый шаблон снимает `is-typing`, чтобы reconnect или
пропущенный `false` не оставлял индикатор зависшим.

## Backend

- Канал `SseSender` становится типизированным:
  `WebViewSseEvent::Text(String) | Typing(bool)`.
- `broadcast_text()` сохраняет старый wire-format.
- Добавляется `broadcast_typing(bool)`.
- WebView server принимает отдельное внутреннее typing-событие и рассылает его
  всем SSE clients, если сервер работает.
- При выключенном сервере событие потребляется и игнорируется без ошибки.
- Добавляется Tauri command WebView consumer; он не проверяет HTML-шаблон.

## Non-goals

- передача текста черновика;
- изменение формата существующего text event;
- автоматическая перезапись пользовательского `index.html`;
- отдельная настройка интервала или отдельный WebView debounce;
- typing из Twitch, interception или других источников;
- обязательный визуальный дизайн индикатора.

## Приёмка

1. Один burst создаёт один `true` и один `false` для каждого consumer.
2. Зависший VTube consumer не задерживает WebView consumer.
3. Ошибка одного consumer не ломает его очередь и остальные consumers.
4. Ноль consumers работает как no-op.
5. Text SSE остаётся byte-compatible по JSON payload: `{"text":...}` и без
   имени события.
6. Typing SSE имеет имя `typing` и payload `{"typing":bool}`.
7. Старый `onmessage`-шаблон не получает typing events.
8. Existing template files не перезаписываются.
9. Новый default template предоставляет CSS hook `html.is-typing` и безопасно
   сбрасывает его при disconnect/reconnect.
10. Focused Vitest/Rust tests, `vue-tsc`, `cargo fmt --check`, `cargo check`,
    production frontend build и debug Tauri build проходят.
