# DECISION-004 — Service-owned state

**Статус:** `accepted`
**Связано:** [TASK-117](../tasks/117-appstate-decomposition.md)

## Контекст

Публичные `Arc<Mutex<_>>` в общем состоянии связывают команды с внутренним
устройством доменов.

## Решение

Доменный service владеет своими settings, status, channels и блокировками.
`AppState` остаётся composition container для service handles и lifecycle
primitives. Переход выполняется постепенно.

## Последствия

Команды обращаются к API владельца, lock guards не пересекают длительные
операции и `await`. До завершения TASK-117 часть legacy-полей остаётся.
