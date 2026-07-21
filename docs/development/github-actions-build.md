# Сборка Windows в GitHub Actions

## Текущий workflow

Windows-сборка настроена в `.github/workflows/build.yml` через runner
`windows-latest` и target `x86_64-pc-windows-msvc`.

На актуальном образе GitHub Windows Server 2025 доступны LLVM и CMake:

- LLVM 20.1.8;
- CMake 3.31.6;
- Visual Studio LLVM/Clang components.

Актуальный список установленного ПО: [Windows runner images](https://github.com/actions/runner-images/blob/main/images/windows/Windows2025-Readme.md#installed-software).

Версии образов GitHub обновляются, поэтому наличие `libclang.dll` нужно
проверять непосредственно в workflow.

## Релизный запуск

Workflow запускается для push тега `v*`, pull request в `master`/`main` и
вручную через `workflow_dispatch`. Только запуск по тегу создаёт GitHub Release;
PR и ручной запуск собирают dev-артефакты.

Перед тегом синхронизируйте версию штатным скриптом, проверьте diff и сборку:

```powershell
node scripts/set-version.cjs 0.14.0
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
```

После коммита версии создайте и отправьте тег, указывающий на нужный commit:

```powershell
git tag v0.14.0
git push origin v0.14.0
```

В CI версия извлекается из имени тега и повторно применяется через
`scripts/set-version.cjs`. Отдельной версии внутри workflow нет. Если требуется
повторить сборку без нового тега, используйте ручной запуск; он не создаёт
release автоматически.

## Кэширование

Кэш npm включён в `actions/setup-node` через `cache: npm`. Rust-кэш настроен
после шага `Setup Rust`:

```yaml
- name: Cache Rust
  uses: Swatinem/rust-cache@v2
  with:
    workspaces: src-tauri
```

Кэшируются Cargo registry, Git-зависимости и `src-tauri/target`. Это особенно
важно для тяжёлых зависимостей Piper и DeepFilterNet (`ort`, `espeak-rs`,
`deep_filter`). Первый запуск создаёт кэш, последующие сборки используют его.

## Проверка нативных инструментов

Перед `npm run tauri build` рекомендуется проверять CMake и `libclang.dll`:

```yaml
- name: Verify native tools
  shell: pwsh
  run: |
    cmake --version

    $paths = @(
      'C:\Program Files\LLVM\bin\libclang.dll',
      'C:\Program Files\Microsoft Visual Studio\*\*\VC\Tools\Llvm\x64\bin\libclang.dll'
    )

    $libclang = $paths |
      ForEach-Object { Get-ChildItem $_ -ErrorAction SilentlyContinue } |
      Select-Object -First 1

    if (-not $libclang) {
      throw 'libclang.dll not found'
    }

    "LIBCLANG_PATH=$($libclang.DirectoryName)" >> $env:GITHUB_ENV
    Write-Host "Using libclang: $($libclang.FullName)"
```

`espeak-rs-sys` использует bindgen и CMake, поэтому без `libclang.dll` и CMake
сборка Windows завершится до создания приложения.

## Фиксация зависимостей

В репозитории должны находиться:

- `Cargo.lock`;
- `package-lock.json`;
- `.github/workflows/build.yml`.

Это обеспечивает воспроизводимое разрешение Rust- и npm-зависимостей и делает
ключи кэша стабильными.
