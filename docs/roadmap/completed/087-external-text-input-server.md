---
id: ROADMAP-087
status: completed
created: 2026-08-31
updated: 2026-08-31
related_tasks: []
---

# ROADMAP-087 — Входной сервер текста для audio-only озвучивания

## Контекст

Внешняя программа, например LunaTranslator, уже умеет получать игровой текст
через HOOK/OCR, фильтровать его и при необходимости переводить. TTSBard не
должен повторять этот upstream-функционал. Ему нужен узкий входной контракт:
принять готовый текст и провести его через собственные TTS, очередь и
аудиовыходы.

Пользовательский сценарий:

```text
LunaTranslator / другая локальная программа
                     │ POST text
                     ▼
        TTSBard Input Server (loopback)
                     │ audio-only job
                     ▼
       SpeechQueue → TTS → DSP → PlaybackManager
                     │
                     ├─ speaker / virtual microphone
                     └─ НЕ WebView / НЕ Twitch
```

Это не ещё один TTS provider и не endpoint, возвращающий аудиофайл клиенту.
TTSBard остаётся владельцем синтеза и воспроизведения; HTTP-ответ подтверждает
приём текста и возвращает идентификатор задачи.

## Рекомендуемая граница MVP

1. В Sidebar раздела «Интеграция» появляется пункт `Входной сервер` сразу после
   `Twitch Chat`.
2. Сервер принимает текст только на `127.0.0.1` и использует отдельный
   настраиваемый порт; рекомендуемый default — `10101` рядом с WebView `10100`.
3. Runtime-кнопки `Запустить` / `Остановить` управляют фактическим listener;
   галка `Запускать при старте приложения` — только persisted preference для
   следующего запуска. Остановленное состояние не оставляет active endpoint,
   возвращающий фиктивный success.
4. Принятый текст сразу становится обычной задачей существующей `SpeechQueue`.
5. Задача использует активные provider и voice, выбранные в TTSBard, а также
   существующие preprocessing, stress, DSP, effects и audio outputs.
6. Destination policy задаётся backend-ом как `audio_only`: задача никогда не
   отправляет текст в WebView или Twitch независимо от текущего editor route и
   содержимого текста.
7. В разделе `Текст` первой закреплённой вкладкой становится `Входящие (N)`.
   Она показывает внешний поток, а не создаёт отдельное окно.
8. Управление после принятия остаётся в существующем окне воспроизведения:
   pause, stop, cancel, retry, skip и порядок очереди не дублируются в панели
   сервера.

Активный provider/voice и влияющие настройки фиксируются в snapshot в момент
приёма запроса, как для редакторской задачи. Последующее переключение provider
не должно менять уже принятую задачу.

## Почему отдельный порт, а не WebView server

| Свойство | Отдельный Input Server | Общий порт с WebView |
| --- | --- | --- |
| Назначение | Входящая команда, создающая speech-job | Исходящий browser-source и SSE |
| Bind MVP | Только `127.0.0.1` | Может быть `0.0.0.0`, LAN и UPnP |
| Lifecycle | Независимый toggle приёма | Listener должен жить, если включён хотя бы один модуль |
| Безопасность | Локальная узкая attack surface | Stateful POST случайно наследует публичность WebView |
| Ошибка/рестарт | Не влияет на OBS browser source | Общий restart прерывает обе интеграции |
| Цена | Ещё один лёгкий Tokio/Axum listener и порт | Рефакторинг WebView в общий HTTP gateway |

Для MVP отдельный listener является более чистой границей. Дополнительный
локальный TCP listener несравнимо дешевле по сложности, чем объединение
независимых desired state, runtime status, bind scope, authentication и restart
policy.

Один порт имеет смысл только после отдельного решения о полноценном
`IntegrationHttpGateway`: сервер запускается, пока включён хотя бы один модуль,
каждый route имеет собственные enable/auth policy, а WebView перестаёт владеть
listener lifecycle. Такое переустройство не требуется для одного входного
endpoint и не входит в MVP.

## HTTP-контракт

Минимальный versioned API:

```text
GET  /health
POST /v1/speech
Content-Type: application/json

{
  "text": "Готовая реплика"
}
```

Успешный ответ — `202 Accepted`:

```json
{
  "job_id": "uuid",
  "status": "queued"
}
```

Контракт должен:

- отклонять пустой и превышающий лимит текст до создания задачи;
- использовать ограничение body size и существующую ёмкость `SpeechQueue`;
- различать invalid request, queue full и отсутствие готового provider/output;
- не логировать полный текст на `info`-уровне;
- не включать CORS и LAN bind в MVP;
- возвращать success только после атомарного добавления задачи в очередь.

`POST /v1/tts/synthesize`, возвращающий audio bytes, является другим продуктовым
контрактом: при нём внешняя программа владеет playback, а TTSBard теряет свою
очередь, dual output и управление. Для текущего сценария этот endpoint не нужен.

## Backend seam

HTTP handler не должен вызывать Tauri command или копировать его правила. Нужен
общий application-level submit seam, который используют и `submit_speech`, и
Input Server:

```text
submit(source, raw_text, delivery_policy) -> AcceptedJob
```

Минимальные typed-значения:

- `source`: `editor` или `external`;
- `delivery_policy`: существующий editor route либо принудительный
  `audio_only`;
- необязательный внешний `request_id` можно добавить вместе с idempotency на
  этапе надёжности.

Для external submit нельзя подставлять синтетический префикс `!!`: это смешает
пользовательский текст с transport policy и сломает строки, которые сами
начинаются с `!`. Audio-only должен быть полем snapshot/job, а preprocessing
должен получать исходный внешний текст без route-синтаксиса.

`source` следует отдать в queue/activity DTO, чтобы окно управления могло
ненавязчиво пометить задачу как `Сервер`, не создавая вторую очередь.

## Автоматический режим

Это единственный режим первого MVP:

1. HTTP request валидируется.
2. Backend снимает текущий provider/pipeline snapshot с `audio_only` policy.
3. Задача атомарно добавляется в общую FIFO `SpeechQueue`.
4. Клиент получает `202` и `job_id`.
5. Существующий worker генерирует и воспроизводит аудио.
6. Задача появляется в текущем окне управления воспроизведением.

Успешную внешнюю фразу можно сохранять в общей phrase history: история не
является destination и полезна для replay. WebView и Twitch не получают её.

## Первая вкладка редактора: `Входящие (N)`

`Входящие` — закреплённая первая вкладка раздела `Текст`, а не вкладка панели
сервера и не ещё одно окно. Её нельзя случайно закрыть или переставить после
обычных текстовых вкладок.

Вкладка показывает список внешних элементов и является проекцией двух
существующих состояний, а не третьей playback queue:

- в автоматическом режиме — external jobs из общей `SpeechQueue`;
- в режиме подтверждения — pending items из `IncomingTextInbox`, а после
  принятия те же элементы связываются с обычными speech jobs.

`N` — количество отображаемых активных элементов: pending, queued, generating,
ready, playing или failed. Завершённые элементы не должны бесконечно увеличивать
счётчик; политику короткого recent-list нужно определить при реализации UI.

В заголовке/toolbar вкладки находится быстрый toggle `Автовоспроизведение`. Это
дубль основной persisted-настройки Input Server, а не независимый локальный
флаг: изменение в любом месте немедленно отражается в обоих UI.

В automatic mode список показывает источник и статус задачи, но использует те
же backend actions, что единый playback activity list. Он не реализует второй
набор правил очереди.

## Режим подтверждения

Если автоматическое воспроизведение неудобно, запрос нельзя сразу помещать в
`SpeechQueue`: иначе синтез может начаться раньше решения пользователя. Нужен
отдельный ограниченный `IncomingTextInbox` со статусом `pending_review`.

Во вкладке `Входящие (N)` каждый pending item имеет три действия:

- `Озвучить` — принять неизменённый текст как audio-only speech-job;
- `Редактировать` — создать новый черновик/вкладку с текстом, не перезаписывая
  существующий пользовательский черновик;
- `Отклонить` — удалить pending item без истории и синтеза.

Inbox имеет собственный небольшой capacity; при заполнении endpoint возвращает
backpressure, а не молча теряет или озвучивает текст. Pending items в первой
версии режима могут быть неперсистентными.

После `Озвучить` управление переходит в существующее окно playback queue. Таким
образом, inbox отвечает только за решение **до** синтеза, а `SpeechQueue` — за
generation/playback lifecycle после принятия.

## UI панели сервера

Панель `Входной сервер` по структуре может использовать знакомые паттерны
и визуальный стиль WebView, но не наследует его сетевые возможности. Для MVP
достаточно:

- runtime-кнопки `Запустить` / `Остановить` и достоверный runtime status;
- `Запускать при старте приложения` как отдельная persisted preference;
- поле порта;
- фактический runtime status `Остановлен / Запускается / Работает / Ошибка`;
- копируемый endpoint `http://127.0.0.1:<port>/v1/speech`;
- локальная тестовая отправка;
- краткая подсказка: используется текущий TTS provider, вывод только в аудио.

Полный toggle `Автовоспроизведение` хранится здесь и зеркалится во вкладке
`Входящие (N)`. Список входящих в панели сервера не дублируется.

Runtime status является backend truth после успешного bind. Занятый порт должен
оставлять панель в `Ошибка` и позволять изменить port; UI не выводит `Работает`
только по persisted toggle.

## Интеграция с LunaTranslator и совместимость API

Эмуляция `vits-simple-api` не даёт полезного ускорения для выбранной
архитектуры. В Luna этот adapter реализован Python-модулем
`tts/vitsSimpleAPI.py`: он вызывает `requests.get(..., stream=True)` по routes
`/voice/speakers` и `/voice/<model>`. `selfbuild` также является Python adapter;
его дополнительная стоимость — динамическая загрузка пользовательского модуля
и один слой вызова, а не отдельный Python HTTP server.

На localhost эта разница мала относительно TTS inference. При reuse
`requests.Session`/keep-alive transport overhead можно дополнительно убрать без
эмуляции чужого API.

Главное различие семантическое:

- `vits-simple-api` и другие Luna TTS adapters ждут audio bytes и передают их в
  playback Luna;
- `/v1/speech` принимает текст, возвращает `job_id`, а synthesis/playback
  выполняет TTSBard;
- при выключенном `Поведение → Озвучивать автоматически` Luna вообще не
  вызывает выбранный TTS adapter, поэтому VITS compatibility route не получает
  текст.

Следовательно, VITS/OpenAI-TTS compatibility можно рассматривать позже для
чужих клиентов, которым действительно нужен audio response, но не как основной
путь Luna → TTSBard.

У Luna уже есть встроенный text-output transport без пользовательского TTS
script: после включения её `Сетевой службы` TTSBard может подключиться как
WebSocket client к:

- `/api/ws/text/origin` — извлечённый исходный текст;
- `/api/ws/text/trans` — готовый результат перевода.

Это потенциально самый простой no-script profile: постоянный WebSocket не ждёт
audio response и сохраняет правильное владение playback. Однако текущий Luna
server bind-ится на `0.0.0.0` и вместе с WebSocket публикует другие API без
отдельного token contract. Поэтому для MVP безопасным generic default остаётся
локальный push в `127.0.0.1` Input Server; `Luna WebSocket` следует добавить как
явный connector/profile после проверки bind/firewall и пользовательской
настройки, а не маскировать под эмуляцию VITS.

## Outcome

Input Server принимает внешний текст от LunaTranslator через loopback HTTP API,
ставит audio-only задачу в существующую очередь TTSBard и не передаёт текст в
WebView или Twitch. Пользователь вручную подтвердил release-smoke: Luna отправила
текст, TTSBard его принял, а Luna не начала собственное воспроизведение.

### Поставленные блоки — 2026-08-31

1. **Ядро Input Server:** typed external submit, принудительный `audio_only`,
   loopback HTTP API, bounded review inbox и lifecycle listener
   (`8b79cc3`, `3976bfc`).
2. **Достоверность и доступность inbox:** runtime gating вкладки, корректные IPC
   идентификаторы, сохранение review-поверхности между разделами и hotkeys
   принятия/редактирования (`9b191f5`, `9b1dd72`, `4932159`, `894ef65`,
   `edecabc`).
3. **Панель и интеграция:** runtime status/titlebar, уточнение терминов и
   компоновки панели, selfbuild-скрипт LunaTranslator (`3976bfc`, `e769ba5`,
   `e7efc84`, `9f083dc`, `60cb97a`).

### Release-smoke — пройден 2026-08-31

LunaTranslator отправляет текст через подготовленный selfbuild-скрипт, TTSBard
принимает его во входящий поток, а Luna не воспроизводит текст самостоятельно.

## Этапы

### P0–P3 — реализовано 2026-08-31

Реализованы общий typed submit contract, `audio_only` delivery policy,
loopback-supervisor и HTTP API, настройки/панель Input Server, первая вкладка
`Входящие (N)` и bounded in-memory review inbox. `auto_play` — persisted
переключатель automatic/review behaviour. Вкладка показывает только pending и
активные external jobs; действия playback не дублируются.

Автоматическая проверка подтвердила HTTP status/error mapping, lifecycle
listener и запрет WebView/Twitch delivery. Ручной release-smoke с LunaTranslator
подтвердил доставку в TTSBard без двойного playback.

### P4 — Надёжность при подтверждённой потребности

1. Добавить optional idempotency/request id против повторного озвучивания после
   client retry.
2. Определить correlation/status API, только если внешнему producer нужен
   lifecycle после `202`.
3. Проверить отдельный `Luna WebSocket` connector как no-script transport.
4. Рассматривать token/LAN bind только отдельным security decision; не
   расширять loopback автоматически.

Первый полезный релиз — P0–P2 в автоматическом режиме. P3 не должен задерживать
интеграцию с Luna и проверку реального стримерского сценария.

## Критерии MVP

- остановленный Input Server не принимает TCP/HTTP requests;
- свободный port даёт фактический `Running`, занятый — устойчивый `Error`;
- валидный request возвращает `202` и существующий `job_id`;
- первая вкладка раздела `Текст` показывает `Входящие (N)`, а её auto-toggle
  синхронизирован с основной настройкой сервера;
- задача сразу видна в общей очереди и управляется существующими действиями;
- provider/voice берутся из TTSBard и фиксируются при приёме;
- текст проходит существующий TTS/PCM/DSP/dual-output pipeline;
- external job не вызывает WebView broadcast и Twitch send даже при включённых
  интеграциях;
- очередь full и неготовый provider дают честную ошибку без принятой задачи;
- restart/shutdown не оставляют orphan listener;
- ручной сценарий Luna подтверждает отсутствие двойного playback.

## Не входит

- OCR, game hooks, выбор области, перевод и управление игрой;
- возврат аудиофайла внешнему клиенту;
- отдельный TTS provider для Input Server;
- выбор provider/voice в каждом HTTP request первого MVP;
- публичный internet/LAN API, UPnP и CORS;
- отдельное окно очереди входящего текста;
- эмуляция `vits-simple-api` в качестве Luna integration path;
- замена существующего WebView/SSE protocol.

## Принятые границы MVP

- отдельный loopback port `10101`, а не общий WebView listener;
- MVP с автоматическим приёмом и первой вкладкой `Входящие`; review actions —
  следующий этап;
- применение к external jobs текущих preprocessing/AI/audio settings вместе с
  активным provider snapshot;
- запись успешно озвученных external jobs в общую phrase history.

## Связанные материалы

- [Интеграция с LunaTranslator — готовый selfbuild-скрипт](../../integrations/lunatranslator/README.md)
- [ROADMAP-047 — очередь задач озвучивания](../completed/047-speech-job-queue.md)
- [ROADMAP-050 — единый список управления воспроизведением](../completed/050-unified-playback-activity-list.md)
- [ROADMAP-073 — маршрут фразы и результат доставки](../completed/073-readable-message-routing-and-delivery-outcomes.md)
- [DECISION-006 — единый PCM pipeline](../../decisions/006-pcm-audio-pipeline.md)
- [DECISION-010 — WebView/SSE server](../../decisions/010-webview-sse.md)
- [DECISION-011 — lifecycle интеграций](../../decisions/011-integration-lifecycle.md)
