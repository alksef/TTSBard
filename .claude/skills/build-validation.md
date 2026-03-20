# Build Validation Skill

## When to Use
- After completing code changes
- Before creating commits
- After merging branches
- Verifying fixes

## Process

### Step 1: TypeScript Check
```bash
npx vue-tsc --noEmit
```
**Expected:** No errors

### Step 2: Rust Check
```bash
cargo check --manifest-path src-tauri/Cargo.toml
```
**Expected:** No errors, minimal warnings

### Step 3: Full Build (optional)
```bash
npm run build
```
**Expected:** Successful build in `dist/`

## Validation Criteria

| Check | Pass Criteria |
|-------|---------------|
| TypeScript | 0 errors |
| Rust | 0 errors |
| Build | Completes successfully |

## Troubleshooting

### TypeScript Errors
- Check type definitions in `src/types/`
- Verify Vue component props/emits
- Run `npm install` if modules missing

### Rust Errors
- Check imports and module paths
- Verify Cargo.toml dependencies
- Run `cargo clean` if cache issues

### Build Errors
- Check Vite config
- Verify all entry points exist
- Review console output for details
