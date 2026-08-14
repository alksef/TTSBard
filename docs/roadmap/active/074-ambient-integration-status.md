---
id: ROADMAP-074
status: in_progress
created: 2026-08-14
updated: 2026-08-15
related_tasks: []
---

# ROADMAP-074 — Статусы интеграций в titlebar

## Контекст

WebView, Twitch и VTube Studio имеют собственные runtime-state и экраны
настройки, но пользователь не видит их готовность во время работы в редакторе.
Проверка требует перейти на каждую панель. Одновременно маршрут текущей фразы и
runtime-готовность интеграции — разные понятия: подключённый Twitch может быть
исключён префиксом `!`, а выбранный Twitch-route не гарантирует, что соединение
установлено.

Главному окну нужен ambient status — заметный при осознанном взгляде, но не
конкурирующий с текстом и primary action. Titlebar подходит как постоянная зона,
в том числе в compact mode, однако его ширина уже используется кнопками
SoundPanel, Playback и управления окном.

## Цель

Постоянно и ненавязчиво показывать состояние WebView, Twitch и VTube Studio в
правой части titlebar, не создавая строку badges и не смешивая глобальную
готовность с маршрутом текущей фразы.

## Нужна ли индикация

Да, но только как ambient awareness, а не как ещё одна панель управления.
Индикация оправдана для состояния, которое:

- меняется независимо от действий в редакторе;
- влияет на доставку фразы или typing-state;
- может незаметно перейти из ready в reconnecting/error;
- пользователю важно знать до отправки, а не после перехода в Settings.

Этим условиям соответствуют WebView server, Twitch connection и VTube Studio
connection/authentication. Playback и speech queue сюда не относятся: у них
есть отдельное Playback Control, а их присутствие превратило бы titlebar в
dashboard.

Намеренно выключенная или никогда не настроенная интеграция остаётся видимой
серой и не воспринимается как ошибка. Стабильные позиции трёх иконок позволяют
считывать состояние без поиска и исключают скачки titlebar при connect/disconnect.

## Размещение

Основное место — отдельный status cluster, выровненный по правой стороне
titlebar перед кнопками SoundPanel/Playback/window controls:

```text
TTSBard                 [WebView] [Twitch] [VTS]      [SoundPanel] [Playback] [—] [×]
```

Между integration cluster и SoundPanel остаётся заметно больший горизонтальный
пробел, чем между иконками внутри каждой группы. Пространство, а не ещё один
divider, разделяет «состояние внешних сервисов» и «локальные floating tools».
Группа интеграций не центрируется относительно всего окна и не приближается к
редактору.

Это глобальное состояние приложения, поэтому оно не размещается:

- внутри поля редактора;
- в editor action bar рядом с route selector;
- в Sidebar, который скрыт в compact mode;
- в постоянной нижней status bar, отнимающей высоту у редактора;
- внутри route dropdown как единственный источник readiness.

Route selector может отражать недоступность конкретного destination внутри
своего списка, но это локальная проверка выбранного действия, а не замена
глобального status cluster.

## Визуальная модель

Компактная группа из трёх постоянных монохромных иконок:

- WebView/Browser Source;
- Twitch;
- VTube Studio.

Каждая иконка всегда занимает одно и то же место. Цвет определяется сочетанием
user intent и фактического runtime-state:

| Намерение и runtime | Вид |
|---|---|
| не настроено, выключено или остановлено вручную | серый |
| запуск/подключение ещё продолжается | серый; допустима только спокойная opacity-анимация |
| фактически `Running`/`Connected`/authenticated | тускло-зелёный |
| интеграция должна работать, но перешла в terminal `Error` | красный |

Красный означает не просто наличие старой ошибки, а нарушение desired-running
состояния. После явной ручной остановки desired state меняется на stopped,
актуальная ошибка перестаёт определять titlebar и иконка сразу становится серой.
Повторный ручной запуск или autostart переводит иконку через серое
starting/connecting состояние; зелёной она становится только после фактической
готовности.

Не следует добавлять отдельный зелёный кружок рядом с каждой иконкой: он
увеличивает визуальный шум и заставляет сопоставлять два символа. Tooltip и
accessible label дают полную формулировку, например `Twitch — подключён` или
`WebView — ошибка запуска: порт занят`.

## Продуктовые границы

1. Status strip показывает только runtime readiness, не маршрут текущей фразы.
2. Для WebView `ready` означает только `WebViewServerStatus::Running`, а не
   persisted `enabled=true`.
3. Для Twitch `ready` означает фактическое `Connected`, а не включённый autostart.
4. Для VTube Studio `ready` означает соединение и подтверждённую authentication,
   а не только открытый socket.
5. Ошибка видима, только если интеграция должна была работать; ручной Stop
   переводит состояние в серое и очищает визуальный error независимо от
   последней diagnostic записи.
6. Группа остаётся читаемой в compact width и не вытесняет SoundPanel, Playback
   или window controls.
7. Цвет не является единственным носителем смысла: tooltip, accessible label и
   форма/opacity состояния остаются различимыми.
8. Статусы загружаются по схеме subscribe-before-snapshot либо эквивалентному
   race-safe контракту.
9. Cluster не показывает route текущей фразы, speech queue, playback и typing
   on/off.
10. В compact mode используются те же status icons без текстовых labels.
11. Tooltip/accessible label сообщает точный сервис и состояние; icon shape и
    state styling позволяют отличить ready, connecting и error без чтения
    tooltip.

## Этапы

### P0 — Truthful snapshots

1. Зафиксировать pure mapping `desired state × runtime state → gray/green/red`
   для трёх owners.
2. Переиспользовать существующие snapshot commands и typed events.
3. Добавить race-safe frontend aggregation без persisted shadow state.
4. Доказать тестом, что ручной Stop после Error даёт gray, а не stale red.

### P1 — Titlebar status group

1. Реализовать три постоянных icon slots, tooltips и accessibility labels.
2. Выровнять cluster справа и отделить увеличенным gap от SoundPanel/Playback.
3. Адаптировать группу к full/compact widths без скрытия отдельных интеграций.
4. Ограничить motion и поддержать `prefers-reduced-motion`.

### P2 — Проверка состояний и доступности

1. Проверить gray для never-configured, disabled и manual Stop.
2. Проверить dim green только после фактической ready/authenticated границы.
3. Проверить red при startup failure и позднем runtime failure при сохранённом
   desired-running.
4. Проверить full/compact widths, системный scaling, dark/light themes и
   различимость без цветового зрения через tooltip/accessible state.
5. Не включать/выключать сервис и не переходить в Settings кликом по status icon
   в начальной версии: это индикатор, а не control.

## Критерии завершения

- статус интеграций считывается без открытия Sidebar;
- выбранный route нельзя спутать с readiness;
- titlebar не выглядит панелью мониторинга;
- compact mode сохраняет все основные действия;
- UI использует фактические runtime statuses, а не desired settings;
- ошибки доступны без постоянного текста в titlebar;
- три слота не меняют позиции при start/stop/error;
- ручная остановка всегда возвращает серый цвет;
- зеленый не показывается до фактического `Running`/`Connected`/authentication;
- красный невозможен для намеренно остановленной интеграции;
- проходят frontend tests, `npm run build` и `./scripts/check-docs.ps1`.

## Не входит

- выбор маршрута текущей фразы;
- per-message delivery outcome;
- счётчик очереди и playback status;
- управление подключениями непосредственно из titlebar;
- общий backend enum, стирающий различия lifecycle интеграций.

## Связанные материалы

- [ROADMAP-044 — VTube Studio connection lifecycle](../completed/044-vtube-studio-connection-lifecycle-ui.md)
- [ROADMAP-062 — VTube Studio runtime truth](../completed/062-vtube-studio-runtime-connection-truth.md)
- [ROADMAP-063 — WebView runtime status](../completed/063-webview-runtime-server-status.md)
- [ROADMAP-073 — маршрут фразы и delivery outcomes](../completed/073-readable-message-routing-and-delivery-outcomes.md)
