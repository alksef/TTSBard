---
id: ROADMAP-041
status: completed
created: 2026-07-22
updated: 2026-07-30
related_tasks: []
---

# ROADMAP-041 — Устранение замечаний review-021

## Контекст

Review-021 проверял диапазон `v0.12.0..7c48b4f` и сообщил о восьми critical,
пяти noncritical findings и одной оптимизации. Сам session-specific review не
был сохранён в tracked sources. Матрица ниже восстановлена из исторических
task-файлов `134-round1-01..08`, corrective iterations, исправляющих коммитов и
текущих regression tests. Исходная привязка severity к отдельным строкам не
восстанавливается надёжно; aggregate `8 + 5 + 1` сохранён только как контекст.

## Матрица findings

| ID | Восстановленное требование | Статус и доказательство |
|---|---|---|
| R021-01 | Восстановить обязательные Rust fmt/Clippy gates без изменения поведения. | Закрыто в `b6bfc3f`; текущие `cargo fmt --check` и строгий Clippy проходят. |
| R021-02 | Не терять Telegram `PasswordToken` после timeout/network error и разрешать корректный retry/restart. | Закрыто в `b6bfc3f`; Telegram client tests и явный `RestartRequired` покрывают flow. |
| R021-03 | Не публиковать устаревшие async-результаты Telegram auth после cancel/reset/close. | Закрыто в `b6bfc3f`; operation generation и `useTelegramAuth` tests фиксируют idle/non-loading state. |
| R021-04 | Quick modes не должны ждать полного synthesis; повторный Enter не создаёт вторую отправку. | Закрыто в `b6bfc3f`; `InputPanel` освобождает UI после принятия job и защищён in-flight guard. |
| R021-05 | Предыдущий HWND нельзя терять при transient foreground failure или конкурентной замене. | Закрыто в `b6bfc3f`; `commands::window::tests` проверяют success, invalid, retry и replacement. |
| R021-06 | Piper phoneme sequence использует все IDs токена и точный `BOS/PAD/.../EOS` contract. | Закрыто в `d136e33`; runtime tests проверяют multi-ID, greedy token, unknown и empty input. |
| R021-07 | Полный `espeak-ng-data` должен попадать в portable package и разрешаться без build-machine path. | Закрыто в `edab9fb`; Tauri resources, runtime resolution и staging smoke test сохранены. |
| R021-08 | Lazy Piper model/config/session публикуются атомарно и не видны частично конкурентным callers. | Закрыто в `edab9fb`; единый `Arc<Mutex<Option<ModelState>>>` остаётся owner состояния. |
| R021-09 | Provider/voice identity снимается вместе с synthesis snapshot и разделяет history/cache keys. | Закрыто последующей speech queue архитектурой: `build_snapshot`, immutable `Snapshot` и identity/cache tests. |
| R021-10 | Сохранение credentials/reconfigure неактивного provider не должно активировать его. | Закрыто: `init_*_tts` только регистрируют entry; state/registry tests сохраняют прежний active ID. |
| R021-11 | Выбор concrete provider выполняется как сериализованный `prepare → persist → publish` с rollback semantics. | Закрыто: `AppState::select_tts_provider` и focused tests доказывают prepare/persist failure и concurrent selection. |
| R021-12 | UI не публикует ложный active provider, не запускает prepare после select и не глушит selection error. | Закрыто: `TtsPanel` использует один adapter command; frontend tests фиксируют один IPC вызов без rollback/prepare. |
| R021-13 | Реальный Piper inference test должен быть явным opt-in fixture, а не зелёным silent skip по абсолютному пути. | Закрыто: ignored test требует `TTSBARD_PIPER_TEST_MODEL` и `TTSBARD_PIPER_TEST_CONFIG` и падает при неверном fixture. |
| R021-14 | Sync config/eSpeak/inference/WAV pipeline и ожидание session mutex не должны блокировать async executor. | Закрыто: весь pipeline выполняется через blocking pool; single-thread Tokio test доказывает responsiveness. |

## Outcome

- Telegram auth, quick-mode/focus, Rust quality gates и Piper phoneme/packaging
  исправления сохранены и покрыты regression tests.
- Provider/voice metadata заморожены в immutable speech snapshot, поэтому смена
  active provider во время synthesis не меняет history/cache identity job.
- Register/reconfigure отделён от activation. Выбор provider сериализован,
  подготавливает runtime до записи, атомарно сохраняет concrete ID и legacy type,
  публикует runtime selection и settings event только после успешного persist.
- Piper synthesis больше не выполняет sync inference на async executor. Реальный
  inference и installer staging оформлены как явные opt-in проверки.
- ROADMAP-041 больше не зависит от отсутствующего review-файла: полный scope,
  статус и verification доступны в tracked документе.

## Проверки завершения

```powershell
npm test
npm run build
npm run check:ipc
npm run test:ipc
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --locked --manifest-path src-tauri/Cargo.toml
cargo check --locked --manifest-path src-tauri/Cargo.toml
./scripts/build.ps1 -Mode debug
./scripts/check-docs.ps1
git diff --check
```

Opt-in checks:

```powershell
$env:TTSBARD_PIPER_TEST_MODEL='<fixture.onnx>'
$env:TTSBARD_PIPER_TEST_CONFIG='<fixture.onnx.json>'
cargo test --locked --manifest-path src-tauri/Cargo.toml `
  test_local_model_tts_synthesize_with_fixture -- --ignored

$env:TTSBARD_STAGING_DIR='<installer-staging-dir>'
cargo test --locked --manifest-path src-tauri/Cargo.toml `
  packaged_staging_contains_espeak_data -- --ignored
```
