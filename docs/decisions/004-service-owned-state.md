# DECISION-004 — Service-owned state

**Статус:** `accepted`
## Контекст

Публичные `Arc<Mutex<_>>` в общем состоянии связывают команды с внутренним
устройством доменов.

## Решение

Доменный service владеет своими settings, status, channels и блокировками.
`AppState` остаётся composition container для service handles и lifecycle
primitives. Переход выполняется постепенно.

## Последствия

Команды обращаются к API владельца, lock guards не пересекают длительные
операции и `await`.

Новый owner/service выделяется только под наблюдаемый дефект или конкретное
изменение, которому мешает прямой mutable access, сложная изоляция теста либо
нарушение атомарного transition. Размер `AppState` и наличие
`Arc<Mutex<_>>`/`Arc<RwLock<_>>` сами по себе не являются основанием для
рефакторинга.
