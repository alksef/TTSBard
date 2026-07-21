# AI-assisted development workflow

Этот файл — нормативный процесс постановки задач DeepSeek через OpenCode.
Задания, промежуточные ревью и логи являются локальными рабочими материалами и
не коммитятся в репозиторий.

## Роли

- Codex/Claude исследует код, ограничивает scope, ставит задачу и независимо
  проверяет результат.
- DeepSeek пишет implementation code по конкретному task-файлу.
- Пользователь утверждает значимые архитектурные решения и коммиты.

Мелкие правки документации, конфигурации, опечаток и локальных build errors
ведущий агент может выполнять напрямую. Для продуктовой реализации используется
цикл task → DeepSeek → review.

## Постоянные и временные материалы

В Git хранятся только материалы, полезные после завершения конкретной сессии:

- направление продукта — `docs/roadmap/`;
- принятое решение и его причины — `docs/decisions/`;
- отобранное исследование — `docs/research/`;
- самостоятельная долгосрочная implementation task — `docs/tasks/`.

В `.work/ai/<work-id>/` хранятся prompts, corrective tasks, ревью, вывод команд
и скриншоты. Локальная задача DeepSeek не становится постоянной документацией
автоматически.

## Создание рабочей области

Из корня репозитория:

```powershell
$workId = "YYYY-MM-DD-short-name"
$work = ".work/ai/$workId"
New-Item -ItemType Directory -Force `
  "$work/tasks", "$work/reviews", "$work/logs", "$work/screenshots" |
  Out-Null
git rev-parse HEAD | Set-Content "$work/baseline-sha.txt"
git status --short | Set-Content "$work/baseline-status.txt"
```

В `context.md` рядом фиксируются цель, разрешённые пути, известные ограничения и
команды приёмки. Baseline нужен, чтобы не принять существующие пользовательские
изменения за результат агента.

## Размер задачи

Не передавайте DeepSeek широкий roadmap item целиком. Один task должен иметь
одну цель, ограниченный набор файлов и собственные критерии приёмки. Предпочтён
один implementation concern либо один composable/module; несколько независимо
проверяемых файлов не объединяются только потому, что относятся к одному этапу.

Не смешивайте без необходимости:

- UI restructuring;
- backend API;
- settings persistence;
- data migration/export;
- packaging и CI.

Если изменение нельзя чисто разделить, сначала создайте небольшой end-to-end
skeleton, а затем расширяйте его отдельными задачами.

## Выбор модели

- `deepseek/deepseek-v4-flash` — один файл или 1–2 тесно связанных файла,
  известный паттерн, локальный тест либо исправление без архитектурного выбора.
- `deepseek/deepseek-v4-pro` — несколько слоёв/файлов, новый seam/API, рефакторинг
  или нетривиальный риск регрессии.
- `deepseek/deepseek-reasoner` — алгоритмический или research-heavy анализ, где
  сравнение подходов важнее скорости.

Для Pro-задачи сначала составляется план декомпозиции в локальной рабочей
области. Для небольшой Flash-задачи искусственный отдельный plan не нужен.

## Task-файл

`tasks/001.md` должен содержать:

1. цель и наблюдаемое ожидаемое поведение;
2. исходный контекст и ограничения;
3. разрешённые и запрещённые пути;
4. существующий source of truth или похожую реализацию;
5. edge cases и regression risks;
6. точные команды проверки;
7. критерии готовности без требования самостоятельно коммитить изменения.

Для UI указываются конкретный эталонный компонент/CSS-класс, состояния success,
cancel, error, retry и stale async result, узкая ширина, обе темы и отсутствие
горизонтального overflow. Icon-only controls требуют `title` и `aria-label`.

## Запуск OpenCode в PowerShell

```powershell
$repo = (Get-Location).Path
$task = ".work/ai/$workId/tasks/001.md"
$log = ".work/ai/$workId/logs/001-opencode.log"
$prompt = Get-Content -LiteralPath $task -Raw

& opencode run `
  --model deepseek/deepseek-v4-pro `
  --dir $repo `
  --log-level ERROR `
  $prompt 2>&1 | Tee-Object -FilePath $log
$openCodeExit = $LASTEXITCODE
```

Путь к checkout не фиксируется в документации. Перед продолжением проверяется
`$openCodeExit`, а не только текст ответа модели.

## Независимая проверка

После каждого запуска ведущий агент:

1. сравнивает `git status --short` с baseline;
2. проверяет diff только разрешённых task paths;
3. читает изменённый код и трассирует end-to-end поведение;
4. запускает проверки, соответствующие риску;
5. записывает verdict в `.work/ai/<work-id>/reviews/`;
6. при замечаниях создаёт следующий небольшой task, а не дописывает старый.

Минимальная матрица проверок:

| Изменение | Проверка |
|---|---|
| TypeScript/Vue | `npm test` при затронутых тестах, затем `npm run build` |
| Rust | целевые тесты и `cargo check --manifest-path src-tauri/Cargo.toml` |
| UI/runtime | релевантный ручной сценарий и состояния error/cancel/retry |
| Packaging/native deps | `scripts/build.ps1 -Mode debug`, при необходимости release |

Чекбоксы и финальный ответ DeepSeek не являются доказательством. Готовность
подтверждают diff, проверки и фактический сценарий. Обычный предел — пять
corrective iterations; затем задача декомпозируется заново или эскалируется
пользователю.

## Завершение

- Обновить roadmap/decision/research только если появился долговечный результат.
- Подготовить scoped commit без чужих изменений.
- Не добавлять `.work/` в Git даже принудительно.
- После принятия результата локальную рабочую область можно удалить; если она
  нужна для продолжения, она остаётся только на машине разработчика.
