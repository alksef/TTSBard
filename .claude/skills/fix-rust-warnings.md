# Fix Rust Build Warnings

This skill fixes all Rust compiler warnings in the Tauri backend.

## When to use

- After making Rust code changes
- Before committing changes
- When `cargo check` shows warnings
- When the build has dead_code, unused_variables, or other warnings

## What it does

1. Runs `cargo clippy` to identify all warnings
2. Analyzes each warning type
3. Applies fixes:
   - **Removes dead code** (unused structs, fields, functions, imports)
   - Removes unused imports and variables
   - Fixes deprecated patterns
   - Adds missing trait imports
   - Removes redundant field initializers
4. Re-runs `cargo check` to verify all warnings are fixed
5. Runs `cargo test` to ensure fixes don't break functionality

## How to invoke

```
/fix-rust-warnings
```

## Common fixes

### Dead code warnings
```rust
// DELETE unused code - do NOT use #[allow(dead_code)]
// Unused code should be removed, not suppressed
```

### Unused variables
```rust
let _x = 5;  // Prefix with underscore to intentionally ignore
```

### Unused imports
```rust
// Remove the entire import line
```

### Missing trait imports
```rust
use std::fmt::Debug;  // Add required trait
```

## Warning categories handled

- `dead_code` - **DELETE unused code** (items not used)
- `unused_variables` - Variables intentionally unused (prefix with `_`)
- `unused_imports` - Imports that can be removed
- `warnings` - General compiler warnings
- `deprecated` - Use of deprecated items
- `missing_docs` - Missing documentation (if enabled)

## Post-fix verification

After fixing warnings, the skill ensures:
- ✅ `cargo check` runs without warnings
- ✅ `cargo clippy` returns no issues
- ✅ `cargo test` still passes
- ✅ No new warnings introduced

## Notes

- **DELETE unused code** - do NOT use `#[allow(dead_code)]` to suppress warnings
- Dead code should be removed, not hidden with attributes
- Use `#[allow(...)]` ONLY for intentionally unused variables (prefixed with `_`)
- Follow Rust best practices: clean code over code volume
