---
id: ROADMAP-054
status: in_progress
created: 2026-07-28
updated: 2026-07-29
related_tasks: []
---

# ROADMAP-054 — Детерминированная видимость предмета VTube Studio

## Контекст

Режим VTube Studio Hotkeys подходит для произвольных действий, но не задаёт
состояние предмета явно. Действие `Toggle Item Scene` переключает текущее
состояние, поэтому пропущенный, внешний или повторный trigger может навсегда
поменять местами реакции «начало набора» и «окончание набора».

Наблюдавшийся runtime-сценарий подтверждает, что TTSBard сохраняет порядок
`false → true`, а VTube Studio подтверждает оба разных hotkey. Тем не менее
фактическая GIF может остаться включённой, после чего следующий start-hotkey
скрывает её. Универсальный `HotkeyTriggerRequest` не предоставляет TTSBard
состояние произвольного toggle-действия и не позволяет установить его в
конкретное значение.

VTube Studio API предоставляет `ItemAnimationControlRequest` с явными
`opacity`, `frame` и `animationPlayState` для обычных изображений и
анимированных предметов. Локальный spike 2026-07-28 подтвердил на реальном
GIF-предмете последовательности `hide`, `show` и быстрые циклы
`hide → show` без инверсии состояния.

## Outcome

Добавить третий output mode для typing-state: TTSBard управляет заранее
размещённым предметом VTube Studio как явным состоянием видимости, а не как
toggle-hotkey.

- `typing=true` всегда делает выбранный предмет видимым;
- `typing=false` всегда делает выбранный предмет невидимым;
- повтор одного состояния идемпотентен и не инвертирует результат;
- Event parameter и Hotkeys продолжают работать без изменения существующей
  конфигурации.

## Выбранный пользовательский контракт

1. Пользователь заранее загружает GIF, PNG, JPG или animation-folder в сцену
   VTube Studio и настраивает положение, размер, слой и привязку.
   Загрузка остаётся ручной, как и настройка Hotkeys: TTSBard не создаёт и не
   восстанавливает предмет автоматически.
2. В панели VTube Studio пользователь обновляет список предметов сцены и
   выбирает предмет для индикации набора.
3. TTSBard сохраняет точное регистрозависимое `fileName`, но не сохраняет
   runtime `instanceID` как долговечный идентификатор.
4. При подключении, переподключении и ручном refresh TTSBard получает актуальный
   `instanceID` через `ItemListRequest`.
5. Для GIF и animation-folder:
   - show устанавливает `opacity=1`, `frame=0`,
     `animationPlayState=true`;
   - hide устанавливает `opacity=0`, `animationPlayState=false`.
6. Для PNG и JPG меняется только `opacity`; неподдерживаемые animation-поля не
   отправляются.
7. При Stop/disconnect приложение best-effort скрывает активный предмет. После
   подключения предмет нормализуется в скрытое состояние, если набор сейчас не
   активен.
8. Сразу после успешной VTS-авторизации item mode проверяет выбранный предмет.
   Отсутствие предмета не меняет статус исправного WebSocket-соединения
   `Connected`, а публикует отдельное состояние typing action:
   `Ready`, `Missing`, `Ambiguous`, `Unsupported` или `Error`.
9. При `Missing` панель постоянно показывает восстанавливаемое предупреждение
   «Предмет <fileName> не найден. Загрузите его в сцену VTube Studio и нажмите
   “Обновить”». До успешной повторной проверки typing-переходы для item mode не
   отправляются и не создают повторяющийся error на каждую правку текста.

## Ограничения MVP

- Поддерживается ровно один загруженный экземпляр выбранного `fileName`.
- Если экземпляр отсутствует, UI сообщает, что предмет нужно загрузить в сцену.
- Если найдено несколько экземпляров, UI требует оставить один; случайный
  выбор запрещён.
- Live2D items не поддерживаются: `ItemAnimationControlRequest` для них
  недоступен.
- TTSBard не загружает предмет, не задаёт его координаты и не восстанавливает
  placement после удаления.
- Item Scenes не получают отдельного state API и остаются в Hotkeys mode.
- После аварийного завершения TTSBard видимый пользовательский предмет может
  остаться видимым до следующего подключения или ручного действия в VTS.

## Runtime-инварианты

1. Source of truth для typing — последнее желаемое boolean-состояние, а не
   количество пришедших переходов.
2. Для одного VTS-соединения WebSocket requests остаются последовательными, но
   очередь typing-переходов не растёт без ограничения: устаревшие ожидающие
   состояния схлопываются до последнего желаемого.
3. Завершение старого async request не может перезаписать более новое желаемое
   состояние.
4. Ошибка или timeout помечают фактическое состояние как неизвестное. Следующая
   синхронизация повторно применяет последнее желаемое состояние вместо
   toggle-компенсации.
5. Typing-запросы используют отдельный короткий bounded timeout; задержка VTS не
   должна визуально подвешивать редактор или блокировать WebView consumer.
6. В логах присутствуют mode, desired state, безопасный item filename/type,
   request duration и результат; token и содержимое пользовательского текста
   не логируются.
7. Проверка item action запускается после connect/reconnect, после сохранения
   item-настройки и по ручной команде refresh. Успешный refresh снимает warning
   без перезапуска TTSBard.

## Этапы

### P1 — API messages и config contract

1. Добавить структуры запросов/ответов для `ItemListRequest` и
   `ItemAnimationControlRequest` с корреляцией по `requestID` и обработкой
   `APIError`.
2. Расширить typing output enum новым item-режимом и сохранить выбранный
   `fileName` вместе с безопасным display metadata.
3. Провести новые поля через persisted settings, backend DTO и TypeScript
   contract с backward-compatible defaults.
4. Покрыть serialization, deserialization и старый JSON без item-полей.

### P2 — Item visibility service

1. Получать загруженные экземпляры и поддерживаемые типы через существующее
   авторизованное VTS WebSocket-соединение.
2. Разрешать `fileName → instanceID` с явными ошибками для нуля, дубликатов и
   неподдерживаемого типа.
3. Реализовать идемпотентные show/hide payloads с раздельной семантикой для
   GIF/animation-folder и PNG/JPG.
4. Встроить item mode в `set_typing`, Stop, disconnect и reconnect без изменения
   поведения Event и Hotkeys.
5. Схлопывать накопившиеся typing-переходы до последнего desired state и
   ограничить ожидание ответа.
6. Отделить connection status от item-action status; missing/ambiguous item не
   должен ложно обозначать VTS как отключённую или сломанную.

### P3 — Настройка и диагностика в UI

1. Добавить item mode в существующий выбор «Действие при наборе».
2. Загружать только предметы текущей сцены поддерживаемых типов и показывать
   filename, type и признак дубликата.
3. Сохранение запрещено при пустом выборе, неподдерживаемом типе или
   неоднозначном экземпляре.
4. Сохранённый предмет остаётся видимым в настройках, даже если временно исчез
   из сцены; UI предлагает refresh/retry и не затирает настройку.
5. Расширить существующий тест действия так, чтобы он выполнял несколько
   show/hide циклов и завершался скрытым состоянием.
6. После connect/reconnect показывать отдельный постоянный warning для
   `Missing`, `Ambiguous`, `Unsupported` и runtime `Error`; warning содержит
   следующий шаг и исчезает после успешного refresh.

### P4 — Lifecycle и ручная приёмка

1. Проверить остановку набора по idle timeout, Enter, Escape, закрытию editor
   component и отключению VTS.
2. Проверить быстрый `false → true`, потерю фокуса и возврат в главное окно.
3. Проверить reconnect с новым `instanceID`, отсутствующий предмет, дубликаты,
   открытое блокирующее меню VTS, timeout и повтор после ошибки.
4. Подтвердить, что Event, Hotkeys и выключенный WebView не получили регрессий.

## Вне текущего направления

- автоматический `ItemLoadRequest`/`ItemUnloadRequest` и управление placement;
- загрузка произвольной GIF через `customDataBase64` и новые VTS permissions;
- поддержка нескольких экземпляров одного файла;
- управление Live2D items, pinning, sorting и движением;
- удаление Hotkeys mode или автоматическая миграция существующих настроек.

Эти возможности оцениваются отдельно после эксплуатации visibility mode.

## Проверка

- Rust unit tests для JSON payloads, response correlation, APIError и timeout;
- service tests для разрешения нуля/одного/нескольких экземпляров и фильтрации
  типов;
- state-machine tests для повторного состояния, быстрого `false → true`,
  coalescing и stale async completion;
- settings/DTO/TypeScript backward-compatible round-trip tests;
- frontend tests выбора режима, refresh, validation и сохранённого missing item;
- `npm test` и `npm run build`;
- focused Rust tests и `cargo check --manifest-path src-tauri/Cargo.toml`;
- `./scripts/check-docs.ps1`;
- ручной сценарий с реальным GIF в VTube Studio, включая stress-цикл и reconnect.

## Условия завершения

- 30 последовательных и быстрых show/hide циклов не приводят к инверсии;
- первый ввод после idle, смены фокуса или reconnect всегда показывает предмет;
- окончание набора и штатный Stop оставляют предмет скрытым;
- отсутствие/дубликат предмета дают понятную восстанавливаемую ошибку и не
  запускают другой экземпляр;
- после подключения с отсутствующим предметом VTS остаётся `Connected`, панель
  сразу показывает item warning, а загрузка предмета и ручной refresh переводят
  action в `Ready` без перезапуска приложения;
- задержка или отсутствие ответа VTS не подвешивает ввод и не создаёт
  неограниченную очередь переходов;
- существующие Event и Hotkeys configurations продолжают десериализоваться и
  работать как раньше;
- фактический outcome и результаты проверок внесены перед переносом item в
  `completed/`.

## Источники

- [VTube Studio API — ItemListRequest](https://github.com/DenchiSoft/VTubeStudio#requesting-list-of-items-in-scene-or-available-to-load)
- [VTube Studio API — ItemAnimationControlRequest](https://github.com/DenchiSoft/VTubeStudio#controling-items-and-item-animations)
- [VTube Studio API — HotkeyTriggerRequest](https://github.com/DenchiSoft/VTubeStudio#requesting-execution-of-hotkeys)
