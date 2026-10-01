---
id: ROADMAP-118
status: completed
created: 2026-10-01
updated: 2026-10-01
related_tasks: []
---

# ROADMAP-118 — Доступ к Input Server из локальной сети с токеном

## Контекст

Input Server (порт по умолчанию 10101) принимает внешний текст только с той
же машины: `bind_loopback` жёстко биндит `127.0.0.1:<port>`
(`src-tauri/src/servers/input_server.rs`), а middleware `host_gate`
отклоняет 403 любой запрос с `Host`, кроме `127.0.0.1:<port>` /
`localhost:<port>` (`src-tauri/src/input_server/server.rs`). Аутентификации
нет: loopback-бинд + host-gate — вся модель безопасности (анти-DNS-rebinding).
Отправить текст с телефона/планшета в домашней сети сейчас невозможно, хотя
веб-форма оверлея (ROADMAP-108) для этого идеально подходит.

Механика для LAN-доступа уже отработана в WebView-сервере (порт 10100):

- `bind_address` в настройках, дефолт `0.0.0.0` (`src-tauri/src/webview/mod.rs`);
- `is_local_network` / `validate_token` (constant-time, crate `subtle`) в
  `src-tauri/src/webview/security.rs`;
- флоу «токен в query → HttpOnly cookie → рабочие запросы с cookie»
  (`auth_handler`/`index` в `src-tauri/src/webview/server.rs`);
- команды `generate_webview_token` / `regenerate_webview_token` (uuid v4),
  masked-отображение и копирование полного токена отдельной командой
  (`src-tauri/src/commands/webview.rs`);
- команда `get_local_ip` для LAN-адреса (`src-tauri/src/commands/webview.rs`).

Решения обсуждения 2026-10-01:

1. **Без переключателя режимов.** Слушатель на `0.0.0.0` принимает loopback
   и LAN одновременно — «два режима» технически не существуют. Loopback и LAN
   — это один бинд и два URL.
2. **Токен для не-loopback с первого дня.** Input Server — write-эндпоинт:
   внешний клиент может заставить машину произнести текст, а при роуте не
   `audio_only` — вывести его в Twitch. Это жёстче политики WebView-сервера
   (тот доверяет RFC1918 без токена), но оправдано асимметрией чтение/запись;
   токен также закрывает сценарии неизолированного гостевого Wi-Fi и
   скомпрометированных IoT-устройств.
3. **Два поля подключения в UI:** локальный URL (без токена) и LAN-URL с
   встроенным токеном, с копированием и кнопкой «Обновить токен».

## Цель

Один слушатель `0.0.0.0:<port>`: подключения с loopback работают как сейчас
без токена; подключения с любого другого адреса проходят только с валидным
токеном; в настройках видны два готовых URL и управление токеном.

## Решения по дизайну

- **Классификация по адресу подключения**: `ConnectInfo(addr)` →
  `addr.ip().is_loopback()` пропускается без токена, остальное — 401 без
  валидного токена. Не «настройка режима», а свойство запроса.
- **Приём токена**: query `?token=` или `Authorization: Bearer`; cookie не
  используется (follow-up 2026-10-01 ниже). Сравнение — `validate_token`
  (constant-time).
- **Область токена**: `/v1/speech` и `/overlay` требуют токен на не-loopback;
  `/health` остаётся открытым (не раскрывает ничего кроме `{"status":"ok"}`).
- **Overlay**: валидный `?token=` при отдаче `/overlay` вшивает токен в
  страницу JSON-литералом (с экранированием `<`); POST формы — same-origin
  fetch с `Authorization: Bearer`; ответ помечен `Cache-Control: no-store`.
- **Токен**: uuid v4, как у WebView-сервера. Автогенерация при первой
  загрузке настроек, если поля нет (иначе LAN молча не заработает после
  апгрейда). Ротация обновляет токен живьём (middleware читает актуальные
  settings), без рестарта слушателя; старый токен перестаёт действовать
  сразу, сохранённые ссылки на телефонах нужно перекопировать.
- **Токен не отображается в UI целиком**: masked-вид + команда копирования
  готового LAN-URL с полным токеном (образец — `copy_webview_token`).
- **`bind_address` в модели настроек остаётся** (дефолт `0.0.0.0`, без поля в
  UI): конфиг-люк для сужения до `127.0.0.1` без релиза.
- **Host-gate сохраняется как defense-in-depth**: принимает loopback-имена и
  приватные адреса (`is_local_network`-подобный разбор Host), чужие hostname —
  403. Токен — основная защита от DNS-rebinding на LAN-IP (у чужой страницы
  токена нет); JSON-POST кросс-доменно и так режется preflight.

## Этапы

### 1. Backend

- `InputServerSettings`: поля `bind_address` (дефолт `0.0.0.0`) и
  `access_token` (дефолт автогенерация при миграции), десериализация
  legacy-конфигов без этих полей, расширение `set_input_server_section`.
- Замена `bind_loopback` на резолв адреса с IPv6-скобками по образцу
  `webview/server.rs`; супервизор уже умеет rebound по смене настроек.
- `into_make_service_with_connect_info` + auth-middleware перед host-gate;
  live-чтение токена из settings.
- Переаботка `host_matches_listener` для приватных адресов.

### 2. Команды и UI

- Команды: `generate_input_server_token`, `regenerate_input_server_token`,
  masked-получение и копирование LAN-URL с токеном (по образцу
  `commands/webview.rs`).
- `InputServerPanel.vue` + `useInputServer.ts`: два поля подключения
  (`http://127.0.0.1:<port>/overlay` и
  `http://<lan-ip>:<port>/overlay?token=…`) с копированием, токен masked,
  кнопка «Обновить токен». LAN-IP через `get_local_ip`. Подписи ru/en
  (каталог локализации ROADMAP-096).

### 3. Тесты

Зеркально существующим тестам роутера:

- loopback без токена: все текущие сценарии не регрессировали;
- LAN-адрес (`ConnectInfo` с приватным IP): без токена 401, с валидным
  токеном 202/страница, с чужим 401; `/health` на LAN без токена 200;
- `/overlay?token=…` валидным токеном вшивает токен без `Set-Cookie`; POST
  с cookie-заголовком получает 401;
- ротация: старый токен 401, новый работает, без рестарта слушателя;
- host-gate: loopback-хосты и приватные IP в Host принимаются, чужой
  hostname и неверный порт — 403;
- миграция настроек: legacy-конфиг без новых полей → дефолты + токен
  сгенерирован.

### 4. Документация и релиз

Release note: после апгрейда Windows Firewall один раз спросит разрешение
для приватных сетей — ожидаемо. Проверка `scripts/check-docs.ps1`.

## Затрагиваемые места

- `src-tauri/src/input_server/server.rs` — host-gate → auth;
- `src-tauri/src/servers/input_server.rs` — бинд, ConnectInfo;
- `src-tauri/src/input_server/mod.rs`, `src-tauri/src/config/settings.rs` —
  настройки и миграция;
- `src-tauri/src/commands/input_server.rs`, `src-tauri/src/lib.rs` — команды;
- `src/components/InputServerPanel.vue`, `src/composables/useInputServer.ts`;
- переиспользуются: `webview/security.rs`, `get_local_ip`, паттерны
  токен-команд из `commands/webview.rs`.

## Порядок реализации

По решению пользователя реализация выполнена ведущим агентом напрямую,
без DeepSeek/OpenCode, промежуточными коммитами: настройки и миграция,
слушатель и auth-gate, команды, фронтенд. Команды проверок — через
`scripts/cargo.ps1` и `scripts/build.ps1`.

## Outcome

Реализовано 2026-10-01 (коммиты `98afe14`…`e0f44a1` и docs-коммит):

- Один слушатель на `bind_address` (по умолчанию `0.0.0.0`) с портом
  без изменений; `bind_address` остался конфиг-люком без поля в UI.
- Loopback-подключения работают как раньше, без токена. Не-loopback:
  обязательный токен (query `?token=` или `Authorization: Bearer`),
  сравнение constant-time; отсутствующий или пустой сохранённый токен
  запрещает не-loopback доступ (fail-closed).
- Host-gate как defense-in-depth: принимает loopback-имена, IPv6 loopback
  и приватные IP-литералы, чужие hostname отклоняет 403.
- `/health` остался открытым; `/overlay` и `/v1/speech` — под токеном.
- `GET /overlay?token=…` валидным токеном вшивает токен в страницу
  (JSON-литерал), форма POST отправляет `Authorization: Bearer`.
- Токен uuid v4; автогенерация при загрузке настроек (легаси-миграция и
  свежая установка), ротация живьём без рестарта слушателя — middleware
  читает актуальные настройки на каждый запрос.
- UI: в панели Input Server два поля подключения (локальный и LAN-URL
  с токеном) с копированием, masked-токен и кнопка ротации с confirm-
  диалогом; локали ru/en синхронизированы по всем трём каталогам.
- Открытые вопросы resolved по плану из документа: автогенерация — да;
  ротация живьём — да; `bind_address` в UI — нет.

Проверки: Rust `--lib` 2080 тестов (включая 16 новых auth/host-тестов
роутера, супервизор и миграции настроек), `cargo check`, vitest 1233,
`vue-tsc`, контракты IPC и локалей, `check-docs.ps1`, debug-сборка
`scripts/build.ps1 -Mode debug`.

Follow-up 2026-10-01: сессионный cookie убран после полевого репорта —
мобильный браузер держал сессию днями, и LAN-страница открывалась без
токена. `/overlay` больше не использует cookie: валидный `?token=`
вшивается в страницу (JSON-литерал с экранированием `<`, ответ
`Cache-Control: no-store`), форма несёт `Authorization: Bearer`; любое
не-loopback открытие без токена — всегда 401, включая запрос со старым
cookie-заголовком. Loopback-вход остался без токена. Оценка по OWASP/
CWE-598: токен в query при входе — осознанный компромисс (точка входа
продукта и до правки), Bearer-схема исключает CSRF по построению.

Release note: после обновления Windows Firewall один раз спросит разрешение
для приватных сетей — это ожидаемый запрос самого приложения.
