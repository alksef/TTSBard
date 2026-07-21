# ROADMAP-041 — Устранение замечаний review-021

**Статус:** `in_progress` — часть замечаний закрыта, полный scope требует финальной сверки

Диапазон: `v0.12.0..7c48b4f`.

## Цель

Закрыть восемь критических, пять некритических замечаний и одну оптимизацию, сохранив текущие пользовательские изменения в рабочем дереве. Работы выполняются малыми задачами DeepSeek с независимой проверкой каждого результата.

## Разбиение и порядок

### Параллельный раунд 1 (непересекающиеся области)

1. **Rust CI gate** — форматирование и Clippy в файлах, перечисленных review; не менять поведение.
2. **Telegram 2FA** — сохранение/retry `PasswordToken`, корректная обработка timeout/network error и invalidation закрытых async-операций.
3. **Быстрые режимы и возврат фокуса** — in-flight guard, раннее освобождение текста/окна и безопасное хранение HWND.

### Последовательные раунды (есть пересечения)

4. **Piper phoneme contract** — multi-ID mapping, BOS/PAD/EOS, тесты.
5. **Piper packaging/runtime** — eSpeak resources в installer, runtime path, атомарная lazy initialization и staging smoke test.
6. **Provider identity/history** — snapshot provider/voice вместе с synthesis, реальные built-in voice IDs и cache identity.
7. **Provider selection/settings transaction** — отделить upsert от activation, atomic validate/prepare/persist/select с rollback; UI не меняет state до подтверждения backend.
8. **Piper integration/performance** — реальный opt-in Windows fixture/inference test и перенос синхронного inference в blocking worker после стабилизации поведения.

## Приёмка

- после каждой задачи: diff review против task-файла и релевантные `cargo test`/`cargo check`/`vue-tsc`;
- перед завершением stage: `cargo fmt --all -- --check`, Clippy в CI-режиме, frontend typecheck/build и полный релевантный Rust test suite;
- не принимать чек-листы DeepSeek без независимой проверки фактического diff и поведения.
