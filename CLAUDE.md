# Claude Instructions

## Subagent Model Configuration

All subagents should be launched using the **glm-4.5** model.

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

## Project Analysis Guidelines

When analyzing this project:
- **Look in:** `docs\` directory for project documentation
- **Ignore:** `docs\plans\` and `docs\reviews\` directories (these contain generated planning and review artifacts)

## Version Management

When updating the application version, **ALL** of the following files must be updated:

| File | Variable/Field | Purpose |
|------|----------------|---------|
| `src/version.ts` | `APP_VERSION_BASE` | Frontend version display (local dev) |
| `package.json` | `version` | Node.js package version |
| `src-tauri/Cargo.toml` | `version` | Rust crate version |
| `src-tauri/tauri.conf.json` | `version` | Tauri app version |
| `.github/workflows/build.yml` | `APP_VERSION_BASE` | CI/CD base version |

**Note:** The `scripts/set-version.cjs` script auto-generates `src/version.ts` during CI builds by combining `APP_VERSION_BASE` with the commit SHA. For local development, update `src/version.ts` manually.

## Research and Problem Solving

When searching for solutions, documentation, or troubleshooting information, you can use the **MCP Perplexity** tools available:

- `mcp__perplexity__perplexity_search` - General web search with real-time information and cited sources
- `mcp__perplexity__perplexity_search_citations` - Search with detailed citations for fact-checking and research
- `mcp__perplexity__perplexity_search_scientific` - Search for scientific and academic information

Use these tools when you need:
- Up-to-date information on libraries, frameworks, or APIs
- Solutions to specific technical problems
- Documentation or examples
- Academic or scientific references

## Code Review

After implementing any feature, fix, or significant code change, **you must review the written code** using the **context7** skill/code-review process:

1. Use the code-review skill to validate the implementation against requirements
2. Ensure code follows project conventions and best practices
3. Verify all tests pass before marking work as complete

Use `superpowers:requesting-code-review` skill when completing tasks, implementing major features, or before merging.

## Planning

When writing implementation plans to `docs\plans\`:
1. **Read the counter** from `docs\plans\counter.txt` before writing a plan
2. Use the counter value as the plan number (e.g., if counter is 38, next plan is `39-YYYY-MM-DD-description.md`)
3. **Increment the counter** after writing the plan file

This ensures sequential plan numbering without conflicts.
