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

## Skills
Available skills in `.claude/skills/`:
- css-development - CSS conventions, variables, theming
- rust-development - Tauri commands, state, error handling
- build-validation - TypeScript/Rust checks, build commands
- code-review-changes - review only modified files (invoke: `/code-review-changes`)
