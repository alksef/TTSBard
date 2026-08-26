---
id: ROADMAP-082
status: completed
created: 2026-08-23
updated: 2026-08-26
related_tasks: []
---

# ROADMAP-082 — Нативный RUAccent runtime через ruaccent-rs

## Цель

Подключить RUAccent к TTSBard как локальный Rust runtime без Python sidecar и
передавать результат Silero и Piper через provider-specific adapters.

## Итоговый backend-контракт

- `ruaccent_rs` подключён по тегу `v0.2.0`; Python и дочерний процесс для
  работы приложения не нужны.
- Discovery создаёт только descriptors и runtime slots. ONNX session появляется
  исключительно по явной команде загрузки или при включённой автозагрузке.
- В памяти остаётся только выбранная модель; смена выбора выгружает остальные.
- Runtime имеет состояния `not_loaded`, `loading`, `ready`, `failed`. Повторная
  явная загрузка после `failed` служит retry.
- Загрузка, выгрузка и inference выполняются вне async IPC thread.
- Ошибка загрузки публикует безопасное глобальное событие без абсолютных путей.
- Синтез не загружает модель скрыто: автоматический слой применяется только к
  уже готовому runtime. Ручной preview возвращает понятную ошибку, если модель
  не загружена.
- `StructuredStress` хранит исходный текст отдельно от render base, поэтому
  восстановленное RUAccent `ё` доходит до Silero и Piper. Ручная метка владеет
  всем словом, включая выбор `е/ё`.
- Старые Python/sidecar поля читаются для совместимости, но больше не
  сериализуются и не участвуют в runtime.

## Outcome

Native pipeline покрывает словари, восстановление `е/ё`, нейронные ударения и
контекстный выбор омографов. Silero получает `+`, Piper — combining acute;
исходный текст используется для провайдеров без такого контракта. Повторная
обработка ручных меток идемпотентна.

## Проверка

- Targeted `stress::contextual` — 12 tests passed.
- Targeted `stress::runtime` — 16 tests passed.
- `scripts/cargo.ps1 check` passed.
- Полный release-review и smoke фиксируются отдельно в локальном отчёте сессии.

## Связанные материалы

- [ROADMAP-083](083-ruaccent-omograph-model-selection.md) — discovery моделей и
  пользовательский lifecycle загрузки.
