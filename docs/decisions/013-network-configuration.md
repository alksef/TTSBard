# DECISION-013 — Сетевая конфигурация

**Статус:** `accepted`

## Контекст

Несколько TTS/AI и messaging providers требуют proxy, timeout и валидации URL.

## Решение

Общие proxy primitives и validation находятся в `config/` и сетевых helpers;
provider хранит только флаг использования и специфичные параметры. Telegram
MTProxy остаётся отдельным протоколом.

## Последствия

Provider не создаёт несовместимый parser proxy URL. Credentials проходят через
DTO без диагностического вывода значений.
