# Claude Instructions

## Subagent Model Configuration

All subagents should be launched using the **glm-4.5** model.

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
