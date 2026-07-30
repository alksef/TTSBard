---
id: ROADMAP-061
status: completed
created: 2026-07-30
updated: 2026-07-31
related_tasks: []
---

# ROADMAP-061 — Настройка действия и замена custom INPUT VTube Studio

## Контекст

Панель VTube Studio позволяет редактировать и сохранять способ действия при
наборе, даже когда VTube Studio не подключена. Для режимов Hotkeys и Item
доступные значения зависят от текущей модели и сцены, а для режима Event
сохранение имени имеет внешний side effect: TTSBard создаёт в VTube Studio
custom INPUT через `ParameterCreationRequest`.

Существующий путь сохранения Event-настройки сначала персистит новое имя, а
затем пытается создать параметр на активном соединении. Поэтому ошибка VTube
Studio может оставить сохранённой настройку, которая фактически не существует
и не работает. При успешном переименовании новый INPUT создаётся, но прежний
custom INPUT остаётся в конфигурации VTube Studio и постепенно засоряет список
параметров.

[ROADMAP-060](../completed/060-vtube-studio-parameter-input-lifecycle.md) обеспечивает
`ensure-before-inject` и корректную регистрацию текущего INPUT. Этот item не
дублирует runtime lifecycle: он определяет доступность настройки, контракт
сохранения нового имени и управляемую очистку заменённого INPUT.

## Цель

Разрешать настройку действия только на живом подключении к VTube Studio и
сделать переименование Event-параметра безопасной заменой: новое имя становится
сохранённым только после успешного создания соответствующего custom INPUT, а
прежний принадлежащий TTSBard INPUT после переключения удаляется.

## Продуктовый контракт

1. Когда VTube Studio не находится в состоянии `Connected`, блок «Действие при
   наборе» доступен только для просмотра сохранённой настройки.
2. В отключённом состоянии нельзя изменить способ действия, имя INPUT,
   Hotkeys или предмет и нельзя сохранить действие. Рядом показывается явная
   подсказка «Подключитесь к VTube Studio, чтобы настроить действие».
3. Ограничение проверяется не только в UI: IPC-команда сохранения отклоняет
   запрос, если к моменту выполнения живое аутентифицированное соединение уже
   отсутствует. Race `Connected → disconnect → Save` не меняет persistence или
   runtime.
4. При сохранении Event с новым именем TTSBard сначала успешно создаёт новый
   custom INPUT через `ParameterCreationRequest`. Если создание отклонено или
   соединение потеряно, пользователь получает ошибку, а сохранённая и runtime-
   настройка остаются прежними.
5. После успешного создания нового INPUT TTSBard персистит новое действие,
   применяет его в runtime и только затем удаляет прежний custom INPUT через
   `ParameterDeletionRequest`.
6. Ошибка persistence после создания нового INPUT не меняет runtime. Новый
   несохранённый INPUT можно best-effort удалить как компенсацию; неудачная
   компенсация отражается в диагностике, но не выдаётся за успешное сохранение.
7. Ошибка удаления прежнего INPUT после успешного переключения не откатывает
   рабочее новое действие. Пользователь получает предупреждение с именами нового
   и неудалённого параметров; соединение остаётся `Connected` при semantic API
   error.
8. Прежний INPUT удаляется только при фактическом переименовании
   `Event(old) → Event(new)`. Повторное сохранение того же имени не выполняет
   удаление.
9. Переключение `Event → Hotkeys/Item` не удаляет Event-параметр: пользователь
   может временно сменить режим и вернуться без разрушения уже настроенного
   INPUT → OUTPUT mapping.
10. Переключение `Hotkeys/Item → Event` обеспечивает выбранный INPUT, но не
    удаляет сохранённое ранее имя только на основании смены режима.
11. Удаляется только custom INPUT, принадлежащий аутентифицированной plugin
    identity TTSBard. Системные параметры и параметры других плагинов не должны
    затрагиваться; отказ VTube Studio трактуется как semantic error.

## Порядок замены Event-параметра

Для `old_name != new_name` на живом аутентифицированном соединении:

1. Проверить и нормализовать новое имя до любых внешних или локальных изменений.
2. Сериализовать операцию с typing/runtime-операциями VTube Studio, чтобы
   keepalive не продолжал отправлять старое имя параллельно удалению.
3. Если старый Event сейчас активен, остановить старый keepalive и best-effort
   отправить старому INPUT значение `0`.
4. Выполнить `ParameterCreationRequest(new_name)` и дождаться подтверждения VTS.
5. При любой ошибке шага 4 вернуть ошибку и оставить старые persistence и
   runtime без изменений.
6. Персистить новое действие. Только после успеха обновить runtime-настройку и
   оповестить frontend об изменении settings.
7. Выполнить `ParameterDeletionRequest(old_name)` после успешного переключения.
8. Вернуть успех при успешной очистке либо предупреждение, если новое действие
   уже работает, но старый INPUT удалить не удалось.

Переименование во время активного набора не должно оставлять старый keepalive,
получающий `453`, и не должно автоматически запускать новое действие: следующий
обычный `typing=true` использует уже сохранённое новое имя.

## Этапы

### P0 — Read-only UI без подключения

1. Ввести единый computed-признак возможности редактировать действие:
   `currentStatus === 'Connected' && !busy`.
2. Заблокировать выбор способа и все относящиеся к нему контролы при отсутствии
   подключения. Сохранённые значения остаются видимыми.
3. Блокировать «Сохранить действие» и показать причину рядом с блоком, а не
   только в `title` кнопки.
4. Не сбрасывать draft и сохранённое действие при disconnect/reconnect.
5. Проверить узкую ширину, обе темы, клавиатурную навигацию и отсутствие
   возможности изменить disabled-контролы.

### P1 — Backend guard и транзакционное создание

1. В начале `save_vtube_studio_typing_action` проверить фактические
   connection/auth/socket prerequisites до persistence.
2. Для изменённого Event-имени выполнять создание нового INPUT до записи
   settings.
3. При semantic и transport error создания не менять настройки ни на диске, ни
   в runtime и не отправлять ложный `settings-changed`.
4. Сохранить существующий persist-before-runtime контракт после подтверждения
   VTS; обработать ошибку persistence без runtime mutation.
5. Не добавлять pending-очередь для сохранения или создания при отключённой VTS.

### P2 — Удаление заменённого INPUT

1. Добавить типизированные payload/request/response для
   `ParameterDeletionRequest` и `ParameterDeletionResponse`.
2. Реализовать service-level удаление на том же последовательном WebSocket-
   пути, что создание и inject.
3. Удалять только прежнее имя при `Event(old) → Event(new)` после успешного
   persist/runtime switch.
4. Разделить semantic отказ удаления и transport failure. Semantic отказ не
   разрушает socket; transport failure обновляет connection/auth/socket state по
   фактическому контракту service.
5. При ошибке удаления вернуть предупреждение об оставшемся старом INPUT без
   отката нового действия.
6. При ошибке persistence после создания нового INPUT попытаться удалить именно
   новый INPUT как компенсацию, не затрагивая старый рабочий параметр.

### P3 — Исполнимое доказательство и документация

1. Добавить frontend tests для disconnected read-only состояния, подключения,
   reconnect, busy и race со статусным событием.
2. Добавить Rust tests на точный порядок:
   `create(new) → persist/apply(new) → delete(old)`.
3. Покрыть отказ создания, отказ persistence, успешную и неуспешную компенсацию,
   semantic/transport ошибку удаления и неизменённое имя.
4. Покрыть переходы `Event → Event`, `Event → Hotkeys/Item`,
   `Hotkeys/Item → Event` и убедиться, что удаление выполняется только в первом
   случае при различающихся именах.
5. Покрыть переименование при активном Event keepalive: reset старого INPUT,
   остановку task и отсутствие последующих inject старого имени.
6. Обновить руководство VTube Studio: настройка доступна после подключения,
   неуспешно созданный INPUT не сохраняется, при переименовании прежний INPUT
   удаляется после успешного переключения.
7. Выполнить ручной сценарий с реальным VTube Studio и двумя именами, включая
   наблюдение списка custom parameters и INPUT → OUTPUT mapping.

## Рекомендуемая декомпозиция реализации

Не передавать весь roadmap одним implementation task.

1. Отдельный frontend task: read-only состояние панели и frontend tests.
2. Отдельный backend task: connection guard и create-before-persist без
   удаления.
3. Отдельный backend task: typed deletion protocol, компенсация и lifecycle
   активного keepalive.
4. Отдельный verification/docs task после независимого review предыдущих
   изменений.

Backend tasks затрагивают несколько слоёв и последовательность побочных
эффектов, поэтому для них предпочтителен `deepseek/deepseek-v4-pro` по процессу
из [AI-assisted workflow](../../development/ai-workflow.md).

## Критерии завершения

- без подключения сохранённая настройка видна, но способ и параметры действия
  нельзя изменить или сохранить;
- backend отклоняет save после race-disconnect без изменения disk/runtime;
- ошибка создания нового Event INPUT показывает ошибку и сохраняет прежнее
  действие без изменений;
- успешное переименование создаёт новый INPUT до persistence и удаляет старый
  только после успешного переключения;
- ошибка удаления не ломает новое действие и отображается как частичный успех с
  предупреждением;
- переключение на Hotkeys/Item не удаляет существующий Event INPUT;
- активный keepalive не отправляет старое имя после его удаления;
- semantic API errors не создают ложный сетевой disconnect, а transport errors
  не оставляют ложный `Connected`;
- проходят focused frontend/Rust tests, `npm run build`, `cargo check --locked`
  и `./scripts/check-docs.ps1`;
- ручная проверка подтверждает, что после `old → new` новый INPUT существует,
  старый отсутствует, а неуспешное создание оставляет старую настройку рабочей.

## Не входит

- удаление Event INPUT при переключении на Hotkeys или Item;
- массовый поиск и удаление всех custom parameters TTSBard;
- удаление параметров других плагинов или default parameters VTube Studio;
- автоматическое изменение INPUT → OUTPUT mapping модели;
- фоновая очередь создания/удаления после disconnect;
- изменение wire/config enum `Event`;
- общий рефакторинг VTube Studio connection manager.

## Outcome

Первая итерация реализована (коммиты `3152a66` backend, `ce355d9` frontend,
документация `8672615`). Независимое ревью (`reviews/final-codex-2026-07-31.md`)
обнаружило четыре дефекта контракта сохранения; все они исправлены в
корректирующей итерации. ROADMAP-061 закрыта по решению пользователя 2026-07-31.

Что уже работает (первая итерация):

- единый признак редактируемости `is_live_authenticated_connection()`
  (`desired_running && Connected && authenticated`);
- порядок замены `Event(old) → Event(new)`: `create(new) → persist → runtime →
  emit → delete(old)`; frontend read-only без подключения.

### Findings независимого ревью (исправлены)

1. **P1 — race no-socket:** `is_live_authenticated_connection()` не проверяет
   `inner.ws`; `ensure_event_parameter_if_connected` возвращает `Ok(false)` при
   no-socket, а команда принимает `Ok(_)` как успех → save без создания INPUT.
2. **P1 — Hotkeys/Item → Event с тем же именем** пропускает создание
   (`should_create` не учитывает `old_output_mode`) → нарушает п.10 контракта.
3. **P1 — компенсация переудаляет:** `ParameterCreationRequest` идемпотентен;
   при persist-fail компенсация безусловно удаляет `new_name` и может разрушить
   ранее существовавший INPUT и его mappings.
4. **P2 — silent skip удаления:** `delete_event_parameter_if_connected` вернёт
   `Ok(())` при no-socket → команда выдаёт полный успех, маскируя пропущенное
   удаление старого INPUT.

### Corrective implementation

Кодовые findings исправлены в незакоммиченном corrective diff 2026-07-31:

- `EnsureOutcome { Ensured, Skipped(reason) }` и
  `DeleteOutcome { Deleted, NotFound, Skipped(reason) }` отличают выполненный
  VTS-запрос от пропуска из-за состояния соединения; `Ensured` намеренно не
  утверждает, был ли параметр новым, поскольку VTS API этого не сообщает;
- orchestration повторно проверяет наличие живого socket и не персистит действие
  после `Skipped` ensure;
- переходы `Hotkeys/Item → Event` обеспечивают INPUT даже при неизменном имени;
- persist-failure не удаляет `new_name`: без preflight невозможно безопасно
  отличить новый параметр от ранее существовавшего;
- `Skipped` удаления возвращается пользователю как частичный успех с
  предупреждением, а `401 NotFound` считается уже выполненной очисткой;
- production-команда и записывающий test-double используют одну orchestration-
  функцию; исполнимый regression-тест проверяет точный порядок
  `live → ensure(new) → persist → runtime → emit → reset(old) → delete(old)`.

Автоматически проверено: 304 focused VTube Rust tests, `cargo check --locked`,
`cargo clippy --tests` (только предсуществующий `speech_queue::test_job`
`too_many_arguments`), `npm run build`, `scripts/check-docs.ps1`.

Ручной сценарий с реальным VTube Studio остаётся post-close проверкой
пользователя и не отмечен как выполненный.

## Источники

- [VTube Studio API — создание и удаление custom parameters](https://github.com/DenchiSoft/VTubeStudio#adding-new-tracking-parameters-custom-parameters)
- [VTube Studio API — удаление custom parameters](https://github.com/DenchiSoft/VTubeStudio#delete-custom-parameters)
- [VTube Studio Model Settings — INPUT/OUTPUT mapping](https://github.com/DenchiSoft/VTubeStudio/wiki/VTS-Model-Settings#vts-parameter-setup)
- [ROADMAP-060 — lifecycle параметра печати VTube Studio](../completed/060-vtube-studio-parameter-input-lifecycle.md)
- [ROADMAP-045 — typing output modes](../completed/045-vtube-studio-typing-output-modes.md)
