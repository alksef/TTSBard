# Version Management Skill

## When to Use
- Updating application version
- Before releases
- When version appears in UI

- When committing version changes
- When updating multiple files

## Files to Update

When updating the application version, **ALL** of the following files must be updated:

| File | Variable/Field | Purpose |
|------|------------------|---------|
| `src/version.ts` | `APP_VERSION_BASE` | Frontend version display (local dev) |
| `package.json` | `version` | Node.js package version |
| `src-tauri/Cargo.toml` | `version` | Rust crate version |
| `src-tauri/tauri.conf.json` | `version` | Tauri app version |
| `.github/workflows/build.yml` | `APP_VERSION_BASE` | CI/CD base version |

## Process

1. Update `APP_VERSION_BASE` in `src/version.ts`
2. Update `version` in `package.json`
3. Update `version` in `src-tauri/Cargo.toml`
4. Update `version` in `src-tauri/tauri.conf.json`
5. Update `APP_VERSION_BASE` in `.github/workflows/build.yml`
6. Run `npm run build` to verify

## Notes

The `scripts/set-version.cjs` script auto-generates `src/version.ts` during CI builds by combining `APP_VERSION_BASE` with the commit SHA. For local development, update `src/version.ts` manually.
