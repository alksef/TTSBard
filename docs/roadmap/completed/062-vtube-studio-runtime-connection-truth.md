---
id: ROADMAP-062
status: completed
created: 2026-07-31
updated: 2026-08-03
related_tasks: []
---

# ROADMAP-062 — Достоверное runtime-состояние соединения VTube Studio

## Outcome

Соединение VTube Studio переведено на единственного долгоживущего owner actor с
непрерывным WebSocket reader-loop. Входящие `PING`, `PONG` и `CLOSE`
обслуживаются и в idle, ответы API коррелируются по `requestID`, а отдельные
операции больше не конкурируют за `recv()`.

Transport failure проходит через единый generation-guarded переход: очищаются
socket, authentication и typing state, backend получает честный `Error`, а
frontend — событие изменения статуса. Устаревшие actor, heartbeat и операции не
могут перезаписать состояние более новой сессии. Semantic VTS API errors
остаются ошибками конкретного запроса и не уничтожают живой transport.

Добавлен отдельный WebSocket runtime probe для проверки подключения,
аутентификации, idle PING/PONG и длительной работы без раскрытия токена. Целевые
автоматические проверки actor lifecycle, correlation, timeout, Close и stale
generation прошли; оставшийся ручной runtime-сценарий подтверждён пользователем
2026-08-03.

Автоматический reconnect не добавлялся: P3 был условным этапом только на случай
доказанной необходимости. Стабильная idle-сессия и явный ручной reconnect
закрывают исходный дефект без фонового reconnect worker.

## Контекст

VTube Studio может успешно принять WebSocket-подключение и аутентификацию, а
затем TTSBard теряет TCP-соединение без корректного WebSocket `Close`. В
наблюдаемом runtime-логе такие потери повторялись через 30–60 секунд и
проявлялись как Windows socket error `10053`.

Сейчас разные запросы обрабатывают transport failure неодинаково. Heartbeat
очищает socket и внутренне переводит connection status в `Error`, но не
публикует изменение во frontend. Загрузка hotkeys и списка предметов может
вернуть ошибку, не инвалидировав всё состояние соединения. В результате UI
продолжает показывать `Connected` и разрешает новые операции над уже мёртвым
socket.

[ROADMAP-060](../completed/060-vtube-studio-parameter-input-lifecycle.md) и
[ROADMAP-061](../completed/061-vtube-studio-action-configuration-and-parameter-replacement.md)
разделили semantic VTS API errors и transport errors для отдельных workflow,
но не ввели единый lifecycle-контракт для всех владельцев WebSocket.

## Подтверждённый дефект: входящие control frames не обслуживаются в idle

Диагностика 2026-07-31 локализовала первопричину разрывов:

1. Отдельный Python probe подключался к тому же `ws://127.0.0.1:8001`,
   аутентифицировался сохранённым токеном и стабильно получал
   `APIStateResponse` дольше нескольких минут одновременно с TTSBard.
2. Protocol-level trace авторизованной сессии зафиксировал входящий WebSocket
   `PING` от VTube Studio и немедленный автоматический `PONG` Python-клиента.
3. TTSBard успешно выполняет connect, authentication и обычные API requests,
   после чего первый либо второй heartbeat через 30–60 секунд получает на read
   `WSAECONNABORTED (10053)`. В то же время Python-сессия продолжает отвечать.
4. Текущая реализация TTSBard вызывает `ws.next()` только внутри
   `send_and_recv()` после собственного API request. В idle нет постоянного
   reader-loop, поэтому входящий `PING` не вычитывается из сокета и
   автоматический `PONG` `tokio-tungstenite` не срабатывает. Уточнение против
   прежней формулировки: tungstenite 0.24 действительно формирует queued `PONG`
   при разборе `Ping` и flush-ит его в начале следующего `read` (см. исходник
   `tungstenite-rs` v0.24.0 `src/protocol/mod.rs`). Дефект — не «PONG не
   формируется», а «PING не читается в idle, поэтому авто-PONG никогда не
   отправляется». `Message::Ping(_) | Message::Pong(_) => continue` в
   `recv_until_match` само по себе безопасно (tungstenite уже поставил PONG в
   очередь); ручная отправка Pong не требуется — лечится постоянный reader-loop.
5. Heartbeat не создаёт исходный разрыв: он первым обнаруживает уже aborted
   socket. Его период не гарантирует попадание в server-side Pong deadline,
   поскольку VTS отправляет control frames по собственному расписанию.

Таким образом, `10053` в воспроизведённом сценарии — не общий сбой VTube Studio,
Windows, токена, Item API или повторного Restart. Это дефект архитектуры клиента
TTSBard: отсутствие непрерывного обслуживания входящих WebSocket frames.

## Цель

Сделать `Connected` доказуемым runtime-состоянием живого аутентифицированного
WebSocket: непрерывно обслуживать text/control frames, коррелировать ответы с
запросами, а любую подтверждённую transport failure атомарно отражать в backend
и UI. После исправления reader lifecycle отдельно определить recovery policy и
не маскировать архитектурный дефект одним только reconnect.

## Продуктовый контракт

1. `Connected` означает одновременно: соединение желаемо, socket существует,
   сессия аутентифицирована и transport не признан потерянным.
2. Любая non-semantic ошибка send/read/timeout/close инвалидирует текущую
   connection generation ровно один раз: socket удаляется, authentication и
   typing state сбрасываются, status становится `Error`.
3. Semantic `APIError` VTube Studio не уничтожает живой transport и остаётся
   ошибкой конкретной операции.
4. Один connection owner постоянно читает WebSocket и немедленно обслуживает
   `PING`, `PONG` и `CLOSE`, независимо от наличия пользовательских API requests.
5. Ответы API направляются ожидающим операциям по `requestID`; unsolicited
   events и control frames не теряются и не присваиваются чужому запросу.
6. Каждое изменение connection status публикуется через один owner-path;
   heartbeat, hotkeys, items, parameter и typing flows не меняют status в обход
   уведомления frontend.
7. После transport failure UI запрещает операции, требующие живого соединения,
   сохраняет пользовательские настройки и предлагает повторное подключение.
8. Устаревшая операция прежней connection generation не может вернуть socket,
   status или item state после disconnect/restart/reconnect.
9. Автоматический reconnect не включается до появления ограниченного policy с
   отменой, backoff и доказанной причиной либо воспроизводимым классом разрывов.

## Этапы

### P0 — Connection owner и постоянный reader-loop

1. Передать владение WebSocket одному долгоживущему connection actor вместо
   последовательного `send_and_recv()` под общим mutex.
2. В actor одновременно обслуживать входящие frames и очередь исходящих
   requests. `PING`/`PONG`/`CLOSE` обрабатываются независимо от application
   heartbeat и пользовательской активности.
3. Коррелировать text responses по `requestID` через pending map/oneshot и
   поддержать несколько вызывающих workflow без конкурирующих `recv()`.
4. На timeout/cancel удалять только соответствующий pending request; не
   отменять reader и не оставлять ответ доступным следующей операции.
5. Ввести owner operation для transport failure с входными generation,
   контекстом операции и исходной ошибкой. Атомарно очищать socket/auth/typing и
   переводить status в `Error` только для текущей generation.
6. Сохранить классификацию semantic API errors: они завершают конкретный pending
   request и не вызывают transport invalidation.
7. Не лечить дефект одним уменьшением `HEARTBEAT_INTERVAL`: частый polling может
   случайно обслуживать Ping, но не является контрактом WebSocket reader.

**Status P0:** реализован и покрыт автоматическими regression tests. Connection
actor непрерывно читает WebSocket; transport failure проходит через generation
guard. Item sync полагается на owner-controlled invalidation, а typing keepalive
передаёт generation исходного actor. Heartbeat захватывает generation actor, не
изменяя её. Ручная runtime-проверка idle-сессии и PING/PONG подтверждена
2026-08-03.

### P1 — Синхронизация backend и frontend

**Status P1:** выполнен. Изменения runtime-status публикуются owner-path, а UI
получает актуальный переход состояния и блокирует операции без живого
аутентифицированного соединения.

1. Сделать публикацию `vtube-studio-status-changed` частью единого перехода
   состояния либо добавить supervisor, публикующий каждое изменение owner state.
2. Устранить прямые записи status из heartbeat и фоновых workers без события.
3. На ошибке hotkeys/items возвращать контекст операции, но одновременно
   показывать актуальный connection status.
4. Привязать доступность кнопок к runtime-status, не стирая сохранённые hotkey
   IDs, item filename и Event parameter draft.
5. Обработать race начальной загрузки и подписки так, чтобы frontend не мог
   пропустить последний status transition.

### P2 — Regression proof и наблюдаемость

**Status P2:** выполнен. Добавлены regression tests и отдельный runtime probe;
ручной сценарий с реальным VTube Studio подтверждён.

1. Добавить безопасные структурированные поля: generation, operation,
   request/response type, elapsed time, close/read/send/timeout class и время с
   последнего успешного ответа.
2. Не логировать authentication token, тексты сообщений и полные чувствительные
   payloads.
3. Добавить transport-level test: server отправляет `PING` между application
   requests, клиент отвечает `PONG` до deadline и остаётся `Connected`.
4. Добавить тесты correlation для out-of-order responses, unsolicited event,
   request timeout, Close и stale generation.
5. Проверить отдельно idle connection, hotkeys, items и typing, а также
   повторный Connect/Restart.
6. Сохранить Python probe и protocol trace как session-local диагностический
   инструмент; не включать токен или auth payload в tracked артефакты.

### P3 — Контролируемое восстановление

**Status P3:** не потребовался. Автоматический reconnect намеренно не добавлен;
сохранён явный ручной reconnect с новой generation.

1. Сначала обеспечить явный ручной reconnect, который всегда создаёт новую
   generation и не наследует broken socket.
2. Если P2 подтверждает пользу auto-reconnect, добавить один сериализованный
   reconnect worker с bounded exponential backoff и jitter.
3. Отменять worker при Stop, изменении настроек, новом ручном Connect/Restart и
   shutdown приложения.
4. После reconnect повторно аутентифицироваться и восстанавливать только
   безопасное runtime-состояние; не запускать typing и не менять видимость item
   без актуального desired state.
5. После исчерпания лимита оставить честный `Error` и понятное ручное действие.

## Порядок выполнения

P0 — обязательное исправление подтверждённой причины `10053`. P1 входит в тот же
первый выпуск либо следует сразу после него: ложный `Connected` не должен
сохраняться при других transport failures. P2 даёт исполнимое доказательство, а
не повторяет уже завершённую диагностику. P3 начинается только после независимой
проверки P0–P2 и отдельного решения о recovery policy.

## Критерии завершения

- send/read/timeout/close в любом VTS request path оставляет согласованные
  `socket=None`, `authenticated=false`, `typing_active=false`, `status=Error`;
- авторизованная idle-сессия принимает server `PING`, отвечает `PONG` до
  deadline и остаётся рабочей через несколько server liveness cycles;
- ни один application request не вызывает собственный конкурирующий `recv()`,
  а ответы доставляются строго по `requestID`;
- semantic API error сохраняет живой socket и `Connected`;
- frontend получает фоновый heartbeat transition без дополнительной команды и
  не позволяет загрузить hotkeys/items после потери transport;
- повторные и устаревшие failures не перезаписывают состояние новой generation;
- ручной reconnect после `10053` создаёт новую рабочую сессию и возвращает UI в
  `Connected` только после успешной аутентификации;
- логи позволяют отличить peer close, local IO abort, send/read error и timeout
  без раскрытия секретов;
- auto-reconnect, если добавлен, имеет bounded policy, отмену и тесты гонок;
- проходят focused Rust/frontend tests, `cargo check --locked`, `npm run build`
  и `./scripts/check-docs.ps1`;
- ручной сценарий подтверждает одинаковое состояние backend/UI при idle,
  hotkeys, items, typing, Stop/Start и реальном разрыве VTube Studio.

## Не входит

- изменение VTube Studio API protocol или устранение дефекта в самой VTS;
- уменьшение heartbeat interval как окончательное исправление;
- маскировка semantic API errors автоматическим reconnect;
- бесконечные reconnect-циклы без лимита и пользовательского контроля;
- изменение контрактов INPUT/OUTPUT, Hotkeys или Item сверх transport lifecycle;
- общий рефакторинг всех интеграций одним diff.

## Связанные материалы

- [ROADMAP-044 — VTube Studio connection lifecycle UI](../completed/044-vtube-studio-connection-lifecycle-ui.md)
- [ROADMAP-060 — lifecycle параметра VTube Studio](../completed/060-vtube-studio-parameter-input-lifecycle.md)
- [ROADMAP-061 — настройка действия VTube Studio](../completed/061-vtube-studio-action-configuration-and-parameter-replacement.md)
- [DECISION-019 — integration settings Arc contract](../../decisions/019-integration-settings-arc-contract.md)
