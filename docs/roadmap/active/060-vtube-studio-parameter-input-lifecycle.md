---
id: ROADMAP-060
status: in_progress
created: 2026-07-30
updated: 2026-07-30
related_tasks: []
---

# ROADMAP-060 — Корректный lifecycle параметра печати VTube Studio

## Контекст

Режим `Event` фактически не публикует событие VTube Studio. Он отправляет значения `0` и `1` через
`InjectParameterDataRequest` во входной tracking/custom parameter. Этот INPUT затем вручную
сопоставляется в настройках модели с выходным Live2D-параметром, например:

```text
TTSBard INPUT TTSBardTyping → Live2D OUTPUT ParamTyping
```

Наличие `ParamTyping` в Live2D-модели само по себе не делает его доступным для
`InjectParameterDataRequest`. Прямая отправка как `Typing`, так и `ParamTyping` возвращает VTS API
error `453` (`InjectDataParamNameNotFound`), если одноимённый INPUT не был зарегистрирован через
`ParameterCreationRequest`.

ROADMAP-045 зафиксировал правильное требование: в parameter-mode custom INPUT создаётся лениво при
первом `typing=true` или тесте. Фактический runtime соблюдает его только в ветке, где
`set_typing(true)` самостоятельно восстанавливает отсутствующий WebSocket. При уже подключённой
сессии и в `test_typing_action` код сразу выполняет inject. Обычное подключение, сохранение нового
имени и переключение с Item/Hotkeys на Event также не гарантируют регистрацию INPUT.

## Цель

Сделать parameter-mode работоспособным при любой допустимой истории подключения: до первого inject
TTSBard гарантирует существование принадлежащего ему custom INPUT, не путает его с Live2D OUTPUT и
даёт пользователю достаточную инструкцию для настройки mapping в VTube Studio.

## Продуктовый контракт

1. Настраиваемое имя означает **VTube Studio INPUT**, созданный TTSBard, а не ID параметра
   Live2D-модели.
2. Значение по умолчанию — `TTSBardTyping`, диапазон INPUT — `0..1`, default — `0`.
3. Перед первым inject в текущей аутентифицированной сессии выполняется идемпотентный
   `ParameterCreationRequest` независимо от того, как был получен WebSocket.
4. Тест и реальное `typing=true` используют один production-путь подготовки параметра; отдельная
   тестовая реализация lifecycle не допускается.
5. TTSBard не пытается напрямую изменять Live2D OUTPUT. Пользователь один раз сопоставляет
   `TTSBardTyping` с подготовленным в модели `ParamTyping` в VTS Parameter Setup.
6. Сериализованное значение режима `Event` сохраняется для совместимости настроек и IPC, пока не
   будет оформлена отдельная миграция. В пользовательском UI режим называется «Параметр VTS» или
   эквивалентно, без обещания VTS event API.
7. Повторное обеспечение уже созданного этим же plugin identity параметра безопасно. Конфликт с
   параметром другого plugin identity, невалидное имя и исчерпание лимита возвращаются как
   понятные ошибки без ложного сообщения об успешном тесте.

## Этапы

### P0 — Единый ensure-before-inject lifecycle

1. Выделить один service-level путь, который на живом аутентифицированном socket гарантирует custom
   INPUT через `ParameterCreationRequest`, а затем выполняет inject.
2. Использовать его в тестовом импульсе и обычном `set_typing(true)` при уже существующем и заново
   открытом WebSocket.
3. При смене имени или режима считать прежнее знание о подготовленном параметре устаревшим; новый
   INPUT обеспечивается перед первой отправкой без требования ручного переподключения.
4. При сохранении изменённого имени в режиме Event на живом аутентифицированном socket сразу
   обеспечить новый INPUT через `ParameterCreationRequest`. Если активного socket нет, сохранить
   настройку только локально: не заводить pending-флаг, очередь или фоновую задачу и не запрашивать
   список custom parameters; обычный ensure-before-inject при test/typing остаётся страховкой.
   Неизменённое имя и режимы Hotkeys/Item не создают сетевой side effect.
5. Определить поведение `typing=false`, если успешного старта не было: не создавать параметр только
   ради reset и не маскировать исходную ошибку.

### P1 — Ошибки и состояние соединения

1. Разделить транспортную потерю WebSocket и валидный `APIError` VTube Studio. Ошибка имени или
   конфликта параметра не должна автоматически представляться пользователю как сетевой обрыв.
2. Для известных ошибок создания/inject показывать действие: исправить имя, освободить конфликтующий
   custom parameter, переподключить plugin identity либо проверить mapping.
3. Для `453` сообщать, какое INPUT-имя отклонено, и не советовать отправлять ID Live2D OUTPUT
   напрямую.
4. После неуспешного теста состояние connection/auth/socket должно соответствовать фактическому
   состоянию транспорта и позволять безопасный повтор, когда VTS не закрывал соединение.

### P2 — Понятная настройка в UI и документации

1. Переименовать пользовательскую метку «Событие» в «Параметр VTS», сохранив wire/config enum
   `Event` на этом этапе.
2. Переименовать поле в «Имя входного параметра VTS» и показать рекомендуемое значение
   `TTSBardTyping`.
3. Рядом с настройкой объяснить mapping: INPUT `TTSBardTyping` → OUTPUT `ParamTyping`, диапазоны
   `0..1`, smoothing `0` для дискретного индикатора.
4. После успешного создания/теста не утверждать, что модель уже настроена: отдельно напомнить о
   необходимости выбрать custom INPUT в VTS Parameter Setup.
5. Поддерживать [руководство VTube Studio](../../integrations/vtube-studio.md) как пользовательский
   source of truth: подключение, три режима, INPUT/OUTPUT mapping, диапазоны, smoothing и ошибки
   `453/454`. Добавить ссылку на него в `docs/integrations/README.md` и краткую корректную сводку в
   `docs/user/presentation-new.md`; исходный `docs/user/presentation.md` не менять, поскольку он
   принадлежит другой задаче.
6. До выпуска исправления явно отмечать в руководстве известный дефект `453` и рабочие альтернативы
   Hotkeys/Item. При закрытии roadmap удалить временное предупреждение и повторно проверить всю
   инструкцию по фактическому UI и реальному VTube Studio.

### P3 — Исполнимое доказательство

1. Добавить service tests с последовательностью `ParameterCreationRequest` → start inject `1` →
   stop inject `0` для теста и реального typing.
2. Покрыть главный regression: соединение уже установлено, INPUT отсутствует, запускается первый
   тест — создание происходит до inject и `453` не возникает.
3. Покрыть переключения Item/Hotkeys → Event, смену имени на живой сессии, повторный тест,
   идемпотентное создание и reconnect.
4. Покрыть сохранение Event-настройки: изменённое имя на живой сессии немедленно отправляет
   `ParameterCreationRequest`, неизменённое имя и отключённая сессия не отправляют запрос и не
   создают отложенное состояние.
5. Покрыть API errors `350`, `352`, `355/356`, `453`, а также transport close/timeout; проверить
   точный connection/auth/socket state после каждой ветки.
6. Добавить frontend tests для новых подписей, подсказки mapping, success/error/retry и сохранения
   совместимого `Event` wire value.
7. Выполнить ручной сценарий с реальной моделью, содержащей OUTPUT `ParamTyping`: создать INPUT,
   сопоставить его, проверить переходы `0 → 1 → 0`, повтор после перезапуска VTS и отсутствие
   горизонтального overflow в обеих темах.

## Порядок выполнения

P0 и focused backend tests выполняются первым самостоятельным task. P1 следует отдельным task после
фиксации фактических response/state contracts. P2 меняет пользовательскую терминологию только после
работающего backend lifecycle. P3 дополняет каждый этап тестами и завершается реальным VTS
acceptance-сценарием; ручной checklist агента без наблюдаемого изменения `ParamTyping` не считается
доказательством.

## Критерии завершения

- первый test в Event/parameter-mode на уже подключённой сессии создаёт custom INPUT до inject и не
  получает `453` для корректного свободного имени;
- первый реальный `typing=true` имеет тот же порядок запросов и запускает keepalive только после
  успешного создания и start inject;
- смена имени и переключение режима не требуют ручного Stop/Start соединения;
- сохранение изменённого Event-имени на активном соединении делает новый INPUT видимым в VTS без
  отдельного запуска теста; при отключённой VTS сохраняется только локальная настройка без pending;
- INPUT `TTSBardTyping`, сопоставленный с OUTPUT `ParamTyping`, наблюдаемо переводит модель
  `0 → 1 → 0`;
- ID Live2D OUTPUT нигде не описан как непосредственная цель `InjectParameterDataRequest`;
- UI использует понятие «Параметр VTS», объясняет INPUT/OUTPUT mapping и сохраняет совместимый
  wire/config contract;
- `docs/integrations/vtube-studio.md` отражает выпущенное поведение без временного предупреждения,
  доступен из индекса интеграций и из `docs/user/presentation-new.md`, а исходный
  `docs/user/presentation.md` остаётся неизменным;
- semantic API errors не разрушают живой socket, transport errors не оставляют ложный
  `Connected`;
- проходят focused Rust/frontend tests, `cargo check --locked`, `npm run build` и
  `./scripts/check-docs.ps1`.

## Не входит

- изменение Live2D-модели или автоматическое редактирование её parameter mappings;
- прямая запись в Live2D OUTPUT через неподдерживаемый `InjectParameterDataRequest`;
- изменение поведения режимов Hotkeys и Item сверх regression-проверок;
- миграция сериализованного enum `Event` без отдельного compatibility plan;
- общий рефакторинг VTube Studio service, connection manager или всех API errors;
- автоматический выбор `ParamTyping`: OUTPUT может иметь другое имя и остаётся выбором пользователя.

## Источники

- [VTube Studio API — custom input parameters и InjectParameterDataRequest](https://github.com/DenchiSoft/VTubeStudio)
- [VTube Studio API — ErrorID 453](https://github.com/DenchiSoft/VTubeStudio/blob/master/Files/ErrorID.cs)
- [VTube Studio Model Settings — INPUT/OUTPUT mapping](https://github.com/DenchiSoft/VTubeStudio/wiki/VTS-Model-Settings#vts-parameter-setup)
- [ROADMAP-045 — VTube Studio typing output modes](../completed/045-vtube-studio-typing-output-modes.md)
