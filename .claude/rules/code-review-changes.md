# Code Review Rules (Changes Only)

## When to Review

After implementing any feature, fix, or significant code change, **review only the changed code** using the **code-review-changes** skill.

## When to Use This Rule

Use this review process when:
- Completing implementation tasks
- Implementing major features
- Fixing bugs
- Before committing or creating PRs

## What Gets Reviewed

**Only the changed code** is reviewed, not the entire codebase. This includes:
- Files modified in the current git diff
- New files added
- Deleted files (for verification)

Review focuses on:
1. **Correctness** - Does the change do what it's supposed to do?
2. **Project conventions** - Does it follow the project's coding standards?
3. **Best practices** - Is the code clean and maintainable?
4. **Security** - Are there any vulnerabilities introduced?
5. **Testing** - Are tests adequate for the changes made?

## How to Invoke

Use the skill: `/code-review-changes`

Or invoke via:
```
Skill: code-review-changes
```

## Review Checklist

The review verifies:
- [ ] Code compiles without errors (TypeScript/Rust)
- [ ] No console.log or debug statements left
- [ ] Error handling is appropriate
- [ ] No hardcoded values where variables should be used
- [ ] Theme support (light/dark) for any UI changes
- [ ] Proper TypeScript types (no `any` abuse)
- [ ] Rust code follows Tauri patterns (if applicable)
- [ ] Git changes are staged appropriately

## Output

The review produces:
1. Summary of changes reviewed
2. List of any issues found (if any)
3. Approval or request for changes
