# Claude Instructions

## Subagent Model Configuration

All subagents should be launched using the **glm-4.** model (use `model: "sonnet").

## When to Use Subagents vs Direct Edits

**Use direct edits (Read/Edit tools) for:**
- Simple file modifications (type fixes, small refactors)
- Adding/removing imports or constants
- Fixing typos and minor bugs
- Changes that affect 1-2 files

**Use subagents (Task tool) for:**
- Multiple independent tasks that can run in parallel
- Code exploration and research across many files
- Complex multi-step operations
- Tasks requiring autonomous decision-making
- Long-running operations (builds, tests)

**Rationale:** Direct edits are faster and provide immediate feedback for simple changes. Subagents add overhead but excel at complex, parallel, or exploratory work.

## Implementation Workflow (Claude → DeepSeek)

### Build-fix size rule

- Claude fixes minor build errors and warnings directly when the change is small and localized.
- For substantial changes, Claude writes a concrete task in `docs/deepseek/tasks/` and immediately runs it non-interactively with `opencode run --model deepseek/deepseek-v4-pro`.
- After DeepSeek finishes, Claude independently reviews the diff and reruns the relevant checks/build; DeepSeek checklist marks are not accepted as verification.

**Claude does NOT write implementation code.** Claude's role is planning and review only.

Workflow:
1. Claude researches the problem (codebase, web search via Perplexity) and produces a detailed implementation plan.
2. The plan is written to `docs/deepseek/plan/` (one file per task/feature).
3. **DeepSeek writes the code** from the plan — this saves Claude tokens.
4. Claude may review the result (via `code-review-changes` skill), but does not author the implementation.

When the user asks to "implement", "add", "fix", or "write" a feature, Claude should:
- Form a plan in `docs/deepseek/plan/` instead of editing source files directly.
- Only do direct edits for trivial/non-implementation tasks (typos, docs, config tweaks unrelated to feature code).

Use `docs/stage/` for research/analysis notes and option comparisons that feed into plans.

### Iteration loop (task → DeepSeek → review)

DeepSeek runs **non-interactively from Claude's Bash tool** via `opencode run`
(no user clicks needed). Full mechanism, paths, and the "never trust DeepSeek's
`[x]` checklist" rule: **see [`docs/deepseek/WORKFLOW.md`](docs/deepseek/WORKFLOW.md).**
Iteration task-files live in `docs/deepseek/tasks/`, verdicts in `docs/deepseek/reviews/`.

## Research and Problem Solving

Use MCP Perplexity tools for web search:
- `mcp__perplexity__perplexity_search` - General search
- `mcp__perplexity__perplexity_search_citations` - Search with detailed citations
- `mcp__perplexity__perplexity_search_scientific` - search for scientific/academic information

## Version Management

See `.claude/skills/version-management.md` for details.

## Rules
Detailed rules in `.claude/rules/`:
- context.md - project analysis guidelines
- planning.md - planning workflow
- code-review.md - code review process
- code-review-changes.md - review only changed code

## Build Scripts (Windows)

Сборка приложения — через PowerShell-скрипты в `scripts/` (Tauri + Vite + Cargo):

- `scripts/build.ps1 -Mode debug` — debug-сборка (`tauri build --debug`): runnable
  `src-tauri/target/debug/ttsbard.exe`, **без** инсталляторов. Быстро, для проверки.
- `scripts/build.ps1 -Mode release` (по умолчанию) — полная релизная сборка: exe +
  инсталляторы (`src-tauri/target/release/bundle/{nsis,msi}/`).
- `-Clean` — очистить `src-tauri/target/` и `dist/` перед сборкой.
- `scripts/build-debug.bat` / `scripts/build-release.bat` — обёртки для двойного клика.

Скрипт проверяет toolchain (node/npm/cargo), ставит npm-зависимости при отсутствии,
после сборки показывает пути и размеры артефактов.
Для быстрых проверок без полного билда см. skill `build-validation`
(`cargo check` / `vue-tsc --noEmit`).

> Примечание: `build.ps1` сохранён как **UTF-8 с BOM** — требование PS 5.1 для
> кириллицы. При правках скрипта не удаляйте BOM.

## Release Tags и GitHub Actions

Для релизной сборки тег нужно отправлять одновременно с веткой, чтобы GitHub
создал событие `push` для workflow:

```powershell
git push origin master refs/tags/v0.14.0
```

Заменяйте `v0.14.0` на актуальную версию. Не ограничивайтесь отдельным
`git push origin vX.Y.Z`: после переименования или пересоздания тегов GitHub
может не запустить `Build and Release`. В таком случае workflow запускается
вручную через **Actions → Build and Release → Run workflow**, выбрав нужный тег.

## Skills
Available skills in `.claude/skills/`:
- css-development - CSS conventions, variables, theming
- rust-development - Tauri commands, state, error handling
- build-validation - TypeScript/Rust checks, build commands
- code-review-changes - review only modified files (invoke: `/code-review-changes`)
