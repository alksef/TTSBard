# Codex Instructions

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

## Implementation Workflow (Codex → DeepSeek)

**Codex does NOT write implementation code.** Codex's role is planning and review only.

Temporary plans, tasks, reviews and logs live only in gitignored
`.work/ai/<work-id>/`. Durable product direction belongs in
`docs/roadmap/active/`; a local DeepSeek prompt is not committed as project
documentation.

Follow [`docs/development/ai-workflow.md`](docs/development/ai-workflow.md) for
task decomposition, model selection, the PowerShell `opencode run` command,
independent review and validation. Codex may directly edit only trivial
non-implementation work and localized build fixes allowed by these instructions.

## Research and Problem Solving

Use MCP Perplexity tools for web search:
- `mcp__perplexity__perplexity_search` - General search
- `mcp__perplexity__perplexity_search_citations` - Search with detailed citations
- `mcp__perplexity__perplexity_search_scientific` - search for scientific/academic information

## Version Management

See `.Codex/skills/version-management.md` for details.

## Rules
Detailed rules in `.Codex/rules/`:
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

## Skills
Available skills in `.Codex/skills/`:
- css-development - CSS conventions, variables, theming
- rust-development - Tauri commands, state, error handling
- build-validation - TypeScript/Rust checks, build commands
- code-review-changes - review only modified files (invoke: `/code-review-changes`)
