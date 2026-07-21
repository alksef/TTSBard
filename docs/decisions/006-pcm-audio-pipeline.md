# DECISION-006 — Единый PCM pipeline

**Статус:** `accepted`
**Заменяет:** [DECISION-016](./016-split-playback-paths.md)

## Контекст

Эффекты, ресэмплинг, очередь и dual output должны одинаково работать для разных
форматов и TTS-провайдеров.

## Решение

Аудио декодируется в PCM, проходит единый порядок enhancement/effects/boundary
processing и передаётся `PlaybackManager`. Устройства выбираются через
CPAL/rodio boundary после обработки.

## Последствия

Provider не запускает внешний player и не управляет очередью. Частота, каналы и
место каждого эффекта являются явными инвариантами pipeline.
