# ROADMAP-018 — Архитектура рантаймов и декомпозиция AppState

**Дата:** 2026-07-11  
**Статус:** `completed` — шаги 1–2 реализованы; шаг 3 оставлен долгосрочным backlog
**Источник:** architecture-review-2026-07-11 + верификация агентами

---

## 1. Текущее состояние (факты из кода)

### 1.1 Runtime-пейзаж — 5 Tokio-рантаймов

| # | Где создаётся | Зачем | Файл:строка |
|---|---|---|---|
| RT-1 | `AppState::new()` | Главный — TTS (`runtime.spawn(...)`) | `state.rs:149` |
| RT-2 | `thread::spawn → Builder::new_multi_thread()` | `run_webview_server` | `setup.rs:450` |
| RT-3 | `thread::spawn → Builder::new_multi_thread()` | WebView autostart | `setup.rs:474` |
| RT-4 | `thread::spawn → Builder::new_multi_thread()` | `run_twitch_client` | `setup.rs:507` |
| RT-5 | `thread::spawn → Builder::new_multi_thread()` | Twitch autostart | `setup.rs:531` |

Плюс два sync-потока (не Tokio):
- **event-thread**: `std::sync::mpsc` channel → `for event in event_rx` (setup.rs:98)
- **soundpanel-thread**: `std::sync::mpsc` channel → `for event in soundpanel_rx` (setup.rs:121)

### 1.2 Паттерн RT-2…RT-5 — антипаттерн

```rust
// setup.rs:450 — один из четырёх одинаковых блоков
thread::spawn(move || {
    let rt = Builder::new_multi_thread()
        .thread_stack_size(8 * 1024 * 1024)
        .enable_all()
        .build().unwrap();
    
    rt.block_on(async move { run_webview_server(...).await });
});
```

Проблема: `new_multi_thread` создаёт **собственный thread pool** (по умолчанию
`num_cpus` потоков). 4 таких пула + RT-1 + 2 sync-потока = при 8 CPU имеем
**~8×5 + 2 = 42+ OS-потока** только для инфраструктуры. При этом RT-1 уже
существует и простаивает: сервера не используют `app_state.runtime`.

### 1.3 AppState — God Object

`state.rs:72–141` — 17 публичных полей, охватывающих TTS, Twitch, WebView,
AI, Playback, SoundPanel, устройства, хоткеи, событийные каналы и рантайм.

`commands/mod.rs` (1385 строк) принимает `State<'_, AppState>` в большинстве
команд и обращается к полям напрямую. `speak_text_internal` (строки 88–276) —
вся TTS-pipeline в одной функции: парсинг → AI → TTS → аудио → история.

---

## 2. Почему это проблема сейчас (не «технический долг»)

### 2.1 Реальные overhead уже ощутимы
- **~40 лишних OS-потоков** при старте — это RSS memory и scheduler overhead на
  каждый `new_multi_thread` runtime даже без нагрузки.
- WebView autostart (RT-3) создаёт runtime, делает **одну операцию** (читает
  `settings.read()`, пишет `settings.write()`), и **уничтожает runtime**.
  8 потоков для одного `await`.
- Twitch autostart (RT-5) — то же самое.

### 2.2 Shutdown не гарантирован
При `quit_app` (commands/mod.rs:51) отправляется `AppEvent::Quit` в webview-канал
и `app_handle.exit(0)`. Но:
- RT-2 через `webview_rx.recv_timeout` может получить Quit с задержкой 1–2 сек.
- RT-4 (Twitch) вообще не получает явного сигнала остановки — полагается на
  drop каналов или `process::exit`.
- Если UPnP mapping не снят до exit — порт остаётся открытым до таймаута роутера.

### 2.3 Тестируемость нулевая
Нельзя написать тест для WebView или Twitch поведения без поднятия всего `AppState`.
Нельзя подменить TTS-провайдер в тесте без мутации глобального состояния.

---

## 3. Предложение: «Использовать один рантайм»

### Шаг 1 — минимальный (низкий риск, большой выигрыш)

**Убрать RT-2…RT-5. Использовать RT-1 (`app_state.runtime`).**

```rust
// setup.rs — было (RT-2):
thread::spawn(move || {
    let rt = Builder::new_multi_thread()...build().unwrap();
    rt.block_on(async { run_webview_server(...).await });
});

// стало:
app_state.runtime.spawn(async move {
    run_webview_server(...).await;
});
```

То же для RT-3, RT-4, RT-5.

Дополнительно: RT-3 и RT-5 (autostart) можно полностью убрать — логику
авто-старта перенести в начало соответствующих async-функций до основного цикла:

```rust
// run_webview_server — в самом начале, вместо отдельного потока:
{
    let settings = webview_settings.read().await;
    if settings.start_on_boot && !settings.enabled {
        drop(settings);
        webview_settings.write().await.enabled = true;
    }
}
// дальше основной loop...
```

**Результат Шага 1:**
- 5 рантаймов → 1
- ~40 лишних потоков → ~8 (один пул RT-1)
- Autostart-логика становится inline и тестируемой

### Шаг 2 — shutdown (средний риск)

Добавить `tokio_util::sync::CancellationToken` в AppState:

```rust
pub struct AppState {
    pub shutdown: CancellationToken,  // вместо разнородных каналов
    // ...
}
```

Каждый server-loop делает `select!`:

```rust
loop {
    tokio::select! {
        _ = shutdown.cancelled() => {
            server.stop();  // UPnP cleanup
            return;
        }
        event = webview_rx.recv() => { ... }
        _ = tokio::time::sleep(Duration::from_secs(1)) => { /* check settings */ }
    }
}
```

`quit_app` вместо `app_handle.exit(0)` вызывает `shutdown.cancel()` и ждёт
завершения всех задач через join-handles.

**Результат Шага 2:**
- Гарантированный UPnP cleanup перед exit
- Явный lifecycle у каждого сервера
- Twitch и WebView останавливаются детерминированно

### Шаг 3 — декомпозиция AppState (высокий риск, долгосрочно)

Разбить 17 полей на 4–5 доменных сервиса:

```
AppState (тонкий контейнер)
├── TtsService     { provider, config, playback_manager }
├── WebViewService { settings, event_channel, task_handle }
├── TwitchService  { settings, connection_status, event_tx }
├── AiService      { client, settings_hash }
└── EditorRuntime  { preprocessor, history, spellcheck }
```

Каждый сервис владеет своим состоянием и shutdown. `commands/mod.rs` обращается
к сервисам через их публичный API, а не к полям AppState напрямую.

**Результат Шага 3:**
- Изменение WebView не требует трогать Twitch-код
- Каждый сервис можно тестировать изолированно
- `speak_text_internal` распадается на: `preprocessor.process()` →
  `ai.correct()` → `tts.synthesize()` → `playback.queue()`

---

## 4. Оценка выгод

| Метрика | Сейчас | После Шага 1 | После Шага 2 |
|---------|--------|--------------|--------------|
| Tokio runtime-ов | 5 | 1 | 1 |
| OS-потоки при старте | ~42+ | ~10 | ~10 |
| UPnP cleanup при exit | ❌ ненадёжен | ❌ | ✅ гарантирован |
| Тест WebView изолированно | ❌ | ❌ | ⚠️ частично |
| Тест TTS-pipeline | ❌ | ❌ | ❌ → Шаг 3 |
| Риск регрессий | — | 🟡 средний | 🟡 средний |
| Объём изменений | — | ~50 строк | ~150 строк |

---

## 5. Приоритизация

**Делать сейчас (вместе с task 113):**  
→ Шаг 1: убрать лишние рантаймы. Это **не каскадный** рефактор —
меняется только `setup.rs` (4 места) + убираем 2 autostart-потока.
Риск: низкий. Время DeepSeek: ~1 итерация.

**Бэклог (следующий спринт):**  
→ Шаг 2: CancellationToken + shutdown. Затрагивает `state.rs`, `setup.rs`,
`servers/webview.rs`, `servers/twitch.rs`, `commands/mod.rs`. Средний каскад.

**Долгосрочно (плановый рефактор):**  
→ Шаг 3: декомпозиция AppState. Большой рефактор. Делать по одному сервису,
начиная с `WebViewService` (наиболее изолированный).

---

## 6. План коммитов (исполнение)

Каждый шаг = отдельный коммит. Порядок фиксирован — следующий шаг только
после `cargo check` + `npx vue-tsc --noEmit` без ошибок на предыдущем.

```
# review-001 fixes (tasks 110-113)
commit: fix: rate_limiter with_config panic + WebViewPanel type safety
commit: fix: SoundPanelTab Tauri listener cleanup on unmount
commit: fix: audio commands migrate to managed SettingsManager
commit: fix: replace blocking recv_timeout with tokio async channel

# stage-18 Шаг 1 — single runtime
commit: refactor: consolidate WebView/Twitch to single Tokio runtime

# stage-18 Шаг 2 — shutdown
commit: refactor: unified CancellationToken shutdown for all servers
```

План DeepSeek-задач для Шагов 1–2: `docs/deepseek/plan/114-...` и `115-...`.

---

## 7. Что НЕ нужно делать

- **Не выбрасывать `app_state.runtime`** — он используется для TTS-задач
  в event_loop.rs и работает корректно.
- **Не переходить на `tokio::main`** — Tauri сам управляет main thread.
  `block_on` в setup — нормально. Проблема только в дополнительных блоках
  внутри `thread::spawn`.
- **Не трогать sync event-thread и soundpanel-thread** — они обрабатывают
  sync-операции (window API, file I/O) и не должны быть async.
