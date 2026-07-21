# TASK-117 — Декомпозиция AppState

**Статус:** `deferred` — выполнять по одному сервису после стабилизации текущих
runtime-функций
**Связано:** [ROADMAP-018](../roadmap/completed/018-runtime-architecture-and-appstate.md),
[архитектура](../development/architecture.md)

## Контекст

Первоначальная задача включала разделение `commands/mod.rs`, выделение сервисов
и декомпозицию TTS pipeline. Эти части в основном выполнены:

- команды разнесены по доменным файлам в `src-tauri/src/commands/`;
- WebView, Twitch, VTube Studio и editor имеют отдельные сервисы;
- этапы синтеза выделены в `commands/tts_pipeline.rs`;
- приложение использует общий Tokio runtime и cancellation token.

Оставшаяся проблема — `AppState` всё ещё предоставляет несколько публичных
`Arc<Mutex<_>>` / `Arc<RwLock<_>>` и совмещает состояние разных доменов. Это
усложняет локальные тесты и позволяет командам обходить API владельца.

## Цель

Сделать `AppState` тонким composition container: доменные данные и блокировки
принадлежат сервисам, а команды работают через их методы.

## Предлагаемые границы

Рефакторинг выполняется независимо, по одному владельцу состояния:

1. `AiService` — cached client и hash настроек.
2. `TtsService` — provider registry и runtime TTS configuration.
3. `InputRuntime` — interception/hotkey state, active window и lifecycle hook.
4. Уточнить владельца playback handle и cached audio devices; не переносить их
   механически без проверки потоков воспроизведения.

WebView, Twitch, VTube Studio и editor повторно не перерабатываются без
конкретной найденной проблемы.

## Правила выполнения

- Один service extraction — одна отдельная implementation task в `.work/ai/`.
- Сигнатуры Tauri commands и имена frontend events сохраняются.
- Сначала добавляется API владельца и тесты, затем мигрируют callers, после чего
  прямое публичное поле закрывается.
- Lock guard не удерживается через сетевой вызов, аудиооперацию или `await`.
- Не проводить big-bang rewrite `state.rs`, `commands/` и `setup.rs` одним diff.

## Критерии готовности

- У каждого затронутого домена есть один явный владелец mutable state.
- Команды не обращаются напрямую к внутренним mutex/RwLock этого домена.
- `AppState` хранит service handles, lifecycle primitives и минимальный
  междоменный runtime state.
- Существующие IPC signatures и сериализуемые DTO не изменены без отдельного
  decision.
- Добавлены тесты на новый service API и сохранены существующие сценарии.
- Проходят `cargo test --manifest-path src-tauri/Cargo.toml` и
  `cargo check --manifest-path src-tauri/Cargo.toml`.

## Не входит в задачу

- смена TTS-провайдеров или аудиоалгоритмов;
- редизайн frontend state management;
- переименование всех исторических типов;
- абстракции «на будущее», не уменьшающие текущий прямой доступ к state.
