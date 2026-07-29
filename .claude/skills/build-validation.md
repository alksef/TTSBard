# Build Validation Skill

Используйте после изменения кода и перед коммитом. Набор проверок выбирается по
риску, а не запускается механически целиком.

## Матрица

| Изменение | Минимальная проверка |
|---|---|
| TypeScript/Vue | `npm test` для затронутой логики, затем `npm run build` |
| Rust | целевые тесты, затем `cargo check --manifest-path src-tauri/Cargo.toml` |
| Rust lint | `cargo clippy --manifest-path src-tauri/Cargo.toml` для существенного изменения |
| UI/runtime | релевантный ручной сценарий, включая error/cancel/retry |
| Packaging/native deps | `./scripts/build.ps1 -Mode debug` |
| Release | `./scripts/build.ps1 -Mode release` |

`npm run build` уже включает `vue-tsc --noEmit`. Полную Tauri-сборку не нужно
запускать для документации или изолированной правки, не затрагивающей runtime и
packaging.

## Локальная Windows-среда

- Debug/release сборку приложения запускать через `scripts/build.ps1`, а не
  напрямую через `cargo build` или `npm run tauri build`: скрипт настраивает
  native dependencies, `LIBCLANG_PATH` и `espeak-ng-data`.
- Перед прямыми Cargo-проверками сначала использовать поиск `libclang.dll` из
  `scripts/build.ps1`. В текущей локальной среде штатный путь —
  `D:\LLVM\bin`; пример:

  ```powershell
  $env:LIBCLANG_PATH = 'D:\LLVM\bin'
  cargo test --manifest-path src-tauri/Cargo.toml --locked
  ```

- Не объявлять Rust-проверку заблокированной после проверки только стандартных
  путей на `C:`: сначала проверить `D:\LLVM\bin` и актуальную логику
  `scripts/build.ps1`.

## Оценка результата

- Проверить exit code и полный вывод, а не только последнюю строку.
- Не исправлять несвязанные baseline warnings в рамках текущей задачи.
- Если проверка не запускалась или среда её блокирует, явно указать это в
  verdict.
- Не использовать `cargo clean` без доказанной проблемы кэша.

Каноническая матрица и порядок независимой проверки описаны в
[`docs/development/ai-workflow.md`](../../docs/development/ai-workflow.md).
