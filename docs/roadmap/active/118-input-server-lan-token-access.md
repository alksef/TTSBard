---
id: ROADMAP-118
status: planned
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
- **Приём токена**: query `?token=`, HttpOnly cookie (SameSite=Lax) или
  `Authorization: Bearer`. Сравнение — `validate_token` (constant-time).
  Cookie-имя своё (не `AUTH_COOKIE_NAME` WebView): cookies не изолируются по
  порту, а `localhost:10100` и `localhost:10101` делят домен.
- **Область токена**: `/v1/speech` и `/overlay` требуют токен на не-loopback;
  `/health` остаётся открытым (не раскрывает ничего кроме `{"status":"ok"}`).
- **Overlay без изменений JS**: валидный `?token=` при отдаче `/overlay`
  ставит cookie; POST формы — same-origin fetch, cookie уходит автоматически.
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
- `/overlay?token=…` валидным токеном ставит cookie; POST с cookie проходит;
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

Стандартный процесс: перед задачей baseline и разрешённые пути в
`.work/ai/<work-id>/`, реализация через DeepSeek/OpenCode, независимая
проверка ведущим агентом по diff и прогону команд. Команды — через
`scripts/cargo.ps1` и `scripts/build.ps1`.

## Критерии результата

- С `127.0.0.1` всё работает без токена ровно как раньше (регрессий по
  сценариям ROADMAP-108/087 нет).
- С LAN-адреса: без токена `/v1/speech` и `/overlay` — 401, `/health` — 200;
  с валидным токеном оверлей открывается и форма отправляет текст (202).
- Оверлей, открытый с `?token=…`, работает дальше без токена в каждом
  запросе (cookie), JS оверлея не менялся.
- «Обновить токен» инвалидирует старый немедленно, слушатель не
  перезапускается.
- Host-gate пропускает loopback-имена и приватные адреса, чужие hostname
  отклоняет.
- Проходят Rust-тесты input-server, `scripts/cargo.ps1 check`, debug-сборка
  и `scripts/check-docs.ps1`.

## Не входит

- IPv6 dual-stack (`[::]`): остаёмся IPv4-only; fallback браузеров с
  `localhost`→`::1` на `127.0.0.1` не регрессия.
- UPnP: интернет-экспозиция — отдельное направление с другим уровнем угрозы.
- TLS/HTTPS на LAN: токен защищает от несанкционированного доступа, но не
  от активного перехвата (ARP-spoofing); фиксируем границу, не решаем.
- Выравнивание политики WebView-сервера (LAN без токена у него остаётся):
  ужесточение сломает существующие LAN-оверлеи OBS, требует отдельного
  решения.
- QR-коды и mDNS-имена для LAN-поля.

## Открытые вопросы

1. Автогенерация токена при миграции (план) vs генерация по явной кнопке —
   во втором случае LAN-поле до генерации показывает placeholder и 401 без
   объяснения; план — автогенерация.
2. Ротация токена живьём через чтение settings в middleware (план) vs рестарт
   слушателя по образцу WebView — план дешевле по UX, но middleware получает
   зависимость от сервиса настроек.
3. Показывать ли `bind_address` расширенным полем в UI (план — нет, только
   конфиг-люк).
