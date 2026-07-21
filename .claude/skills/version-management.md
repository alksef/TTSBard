# Version Management Skill

Версия меняется штатным скриптом из корня репозитория:

```powershell
node scripts/set-version.cjs <version> [commit-sha]
```

Скрипт синхронизирует:

- `package.json`;
- `src-tauri/Cargo.toml`;
- `src-tauri/tauri.conf.json`;
- `src/version.ts`.

После запуска просмотрите scoped diff и выполните `npm run build` и
`cargo check --manifest-path src-tauri/Cargo.toml`. Не редактируйте версию в
`.github/workflows/build.yml`: CI получает её из тега `vX.Y.Z` и вызывает тот
же скрипт.

Создание и публикация релиза описаны в
[`docs/development/github-actions-build.md`](../../docs/development/github-actions-build.md).
