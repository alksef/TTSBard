# Задачи реализации

Здесь хранятся только долговечные, самостоятельно исполнимые задачи, которые
слишком конкретны для roadmap, но должны пережить отдельную AI-сессию.

Task должна иметь статус, связь с roadmap/decision, ограниченный scope,
критерии приёмки и проверки. Локальные prompts, итерации и verdict хранятся в
`.work/ai/`, а не в этом каталоге.

После реализации task удаляется: итог фиксируется в связанном roadmap item,
decision или профильной документации. Каталог не является архивом выполненных
работ и не заменяет issue tracker.

## Текущие задачи

- [TASK-119 — third-party notices для release bundle](./119-third-party-license-notices.md) —
  `planned`, автоматизировать лицензионный реестр и проверку bundled resources.
- [TASK-118 — исправить неработающий online spellcheck](./118-online-spellcheck-missing-command.md) —
  `planned`, устранить несуществующую backend-команду или честно убрать режим.
- [TASK-117 — декомпозиция AppState](./117-appstate-decomposition.md) —
  `deferred`, долгосрочный backend-рефакторинг.
