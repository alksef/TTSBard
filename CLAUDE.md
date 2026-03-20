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
