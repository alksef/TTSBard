# Code Review Changes Skill

## When to Use
Use this skill after implementing code changes to review **only the modified files**, not the entire codebase.

Invoke via: `/code-review-changes`

## Process

1. **Get changes**: Run `git diff HEAD` to see what changed
2. **Read changed files**: Use Read tool on each modified file
3. **Apply project rules**: Check against:
   - `.claude/rules/` - project-specific rules
   - `.claude/skills/` - language/framework conventions
4. **Compile check**: Verify TypeScript and Rust compile
5. **Report findings**: List issues or approve changes

## What to Check

### TypeScript/Vue Files
- No `console.log` or debug statements
- Proper types (avoid `any`)
- CSS variables used (no hardcoded colors)
- Theme support for UI changes
- Error handling where appropriate

### Rust Files (src-tauri/)
- Tauri command pattern followed
- Proper error handling (`Result<T, String>`)
- State access patterns correct
- No `unwrap()` or `expect()` without context
- Commands registered in `lib.rs`

### General
- No commented-out code
- No TODO comments without issues
- Code follows existing patterns in the file
- Changes match the intended purpose

## Commands to Run

```bash
# TypeScript check
npm run check

# Rust check
cargo check --manifest-path src-tauri/Cargo.toml

# Rust linter
cargo clippy --manifest-path src-tauri/Cargo.toml
```

## Review Format

```
## Code Review: [brief description]

### Changes Reviewed
- File 1: [summary of change]
- File 2: [summary of change]

### Issues Found
[If none found, say "No issues found"]

### Approval Status
✅ APPROVED or ❌ CHANGES REQUESTED
```

## If Issues Found

1. List each issue with file:line reference
2. Explain why it's a problem
3. Suggest the fix
4. Do NOT fix it yourself - let the user decide
