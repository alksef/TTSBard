# Code Review Rules

## When to Review
After implementing any feature, fix, or significant code change, **review only the changed code** using the **code-review-changes** skill.

See: `.claude/rules/code-review-changes.md` for detailed review process.

## How to Review
Use `code-review-changes` skill when:
- Completing tasks
- Implementing major features
- Before merging

## Usage
Invoke with: `/code-review-changes`

## What Gets Reviewed
- Only files modified in the current git diff
- New files added
- Deleted files (for verification)

The review focuses on:
1. Correctness - Does the change do what it's supposed to do?
2. Project conventions - Does it follow the project's coding standards?
3. Best practices - Is the code clean and maintainable?
4. Security - Are there any vulnerabilities introduced?
5. Testing - Are tests adequate for the changes made?
