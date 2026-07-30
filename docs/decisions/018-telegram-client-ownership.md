# DECISION-018 — Владение Telegram client state: контракт разделения Arc

**Статус:** `accepted` (контракт разделения client Arc зафиксирован)
**Связано:** [ROADMAP-059](../roadmap/completed/059-integration-state-ownership-and-settings-atomicity.md),
[DECISION-004](004-service-owned-state.md), [TASK-117](../tasks/117-appstate-decomposition.md)

> **TL;DR.** `TelegramState.client: Arc<Mutex<Option<TelegramClient>>>` остаётся
> разделяемым (`pub(crate)`) — Arc по контракту хранят долгоживущие движки
> (`SileroTts`, AI). Строгий private mutex **не нужен** и **не преследуется**:
> технического вреда (гонок/паник) от share нет, а закрытие mutex потребовало бы
> перерезки контракта hot path TTS. Вместо private вводится обязательное правило
> доступа `lock → clone → drop guard → await` и запрет прямого лок-доступа из
> command adapter.

## Контекст

`TelegramState.client` (`commands/telegram.rs`) — единственное публичное поле,
но его `Arc` **не локализован в command adapter**. Он утёк в долгоживущих
потребителей, которые держат его и лочат самостоятельно:

| Потребитель | Где | Характер использования |
|---|---|---|
| **SileroTts** (TTS-движок) | `tts/silero.rs` | Хранит `Arc<Mutex<Option<TelegramClient>>>` в `self.client`; **лочит при каждом `synthesize`** (hot path) |
| **setup / AppState** | `setup.rs` | `init_silero_tts(Arc::clone(&telegram_state.client))` |
| **AI flow** | `commands/ai.rs` | `Arc::clone(&telegram_state.client)` |
| **commands** | `telegram.rs`, `proxy.rs` | lock → clone/take/store → drop → await |

Известны два **lock-through-await дефекта** (нарушение ROADMAP-059 инвариант #3):
`telegram_select_voice` (lock через `set_speaker().await`) и `reconnect_telegram`
(lock через `old_client.disconnect().await`, блокирующий все Telegram-команды на
время disconnect).

`client.client.lock()` в `telegram/bot.rs` и `telegram/client.rs` — **другое**
поле (внутренний `Arc<Mutex<Option<Client>>>` типа `TelegramClient` из grammers),
не `TelegramState.client`. Этот seam не затрагивается.

## Почему строгий private mutex не нужен

Инкапсуляция мьютекса обычно защищает от: (1) гонок данных, (2) паник на `Option`
под гонкой, (3) внешних модулей, нарушающих инварианты владельца. Разбор кода
показывает, что **ни одной из этих угроз здесь нет**:

- **Гонок данных нет** — `Arc<Mutex<_>>` уже даёт synchronisation; каждый
  потребитель локает корректно.
- **Clone дешёвый и безопасный** — `TelegramClient` это набор
  `Arc<Mutex<Option<...>>>` (`#[derive(Clone)]`). SileroTts **не** держит «старый
  клиент»: он держит state-контейнер и на каждом `synthesize` делает
  `lock → clone → drop` (`tts/silero.rs`).
- **Гонка reconnect vs synthesize graceful** — `take()` атомарно вынимает старый
  клиент под локом; идущий в этот момент `synthesize` получает `None` и падает в
  ошибку через `ok_or_else(...)?`, **без паники**.
- **`.unwrap()` на client в прод-коде нет** — везде `Option` + `?`. Паник-векторов нет.
- `disconnect()` на другом клоне того же Arc инвалидирует внутренний
  grammers-client — это **намеренное** поведение connection reset, не баг.

### Цена, которую private mutex потребовал бы

Чтобы сделать mutex строго private, нужно сменить контракт hot path
`SileroTts::synthesize`: вместо «держу Arc, локаю сам» — «прошу client у владельца
через handle/trait». Все три пути плохи:

- **Дать SileroTts `Arc<TelegramState>`** → циклическая зависимость tts ↔ commands;
  движок привязывается к IPC-layer типу.
- **Вынести client в `TelegramService` и раздавать Arc из него** → формальная
  перерезка; меняется только имя владельца, mutex остаётся `pub(crate)`,
  инвариант #4 **не закрывается**.
- **Сменить контракт SileroTts на handle/trait** → переписать **hot path** синтеза,
  риск регрессии TTS-латентности и ROADMAP-041 serializer-инвариантов.

Итог: цена private mutex — перерезка hot path — **превышает пользу**, потому что
технического вреда от share нет.

## Решение: зафиксировать контракт разделения Arc

1. `client` остаётся `pub(crate)` (не private). `pub(crate)`-видимость означает,
   что внешний (за пределами крэйта) доступ невозможен; внутри крэйта допустимые
   потребители — `TelegramState` (владелец lifecycle) и long-lived движки
   (`SileroTts`, AI), которым клиент нужен по контракту.
2. **Обязательное правило доступа** для всех, включая движки:
   `lock → clone Option → drop guard → await`. Guard **никогда** не удерживается
   через network request / filesystem I/O / другой длительный `await`
   (ROADMAP-059 инвариант #3).
3. **Command adapter не владеет client-state и не пишет `state.client.lock()`
   напрямую** (ROADMAP-059 инвариант #2): команды делегируют owner-методам
   `TelegramState` (`current_client` / `set_client` / `clear_client` /
   `swap_client` / `with_client`).
4. Прямой locker `client` оправдан **только** для долгоживущего движка без доступа
   к owner-API.

## Где это зафиксировано, чтобы подтягиваться при ревью

Контракт дублирован в трёх местах, чтобы его нельзя было пропустить при работе с
этим seam'ом:

- **Код** — контрактный doc-комментарий у поля `TelegramState.client`
  (`src-tauri/src/commands/telegram.rs`) и ссылка на DECISION-018 у
  `SileroTts.client` (`src-tauri/src/tts/silero.rs`).
- **Roadmap** — ROADMAP-059 инвариант #4 переформулирован под контракт разделения
  (не «private mutex», а «правило доступа + запрет прямого лок-доступа из команд»).
- **Настоящий decision** — обоснование «почему не private».

## Область применения в ROADMAP-059 P1 (вариант A)

P1 = закрыть прямые локи/мутации из command adapter через owner-методы + устранить
два lock-through-await дефекта (`swap_client`, `with_client`), оставив `client`
`pub(crate)` по контракту. Это полностью закрывает инварианты #2 и #3 и часть #4
(`pub` → `pub(crate)`). Срезы для DeepSeek:

1. owner-методы `current_client/set_client/clear_client` + миграция команд-читателей
   и простых мутаций (init/auto_restore/disconnect/sign_out/request_code/sign_in/
   check_password/get_status/get_user/speak_text_silero/get_current_voice/
   get_limits/set_speaker). `client` → `pub(crate)`.
2. `swap_client` + `with_client`, миграция `reconnect_telegram` (proxy.rs) и
   `telegram_select_voice`; доказать тестами отсутствие лок-через-await.

Между срезами — независимая проверка (`cargo check/clippy --lib`, targeted
`cargo test --lib telegram`, ROADMAP-041 tests зелёные).

## Условия пересмотра

Контракт пересматривается, если появится новый потребитель client, которому
**аргументированно** нельзя дать owner-API (тогда нужна перерезка hot path), либо
если обнаружится реальная гонка/паника, не покрываемая `Option` + `?`. До этого
момента private mutex для этого seam'а **не является целью**.

## Не входит

- Перерезка контракта SileroTts (handle вместо Arc) — не преследуется.
- Изменение auth UX / Telegram protocol.
- IPC signatures и serialized DTO.
- Touch внутреннего `client.client` mutex в `telegram/bot.rs`, `telegram/client.rs`.
