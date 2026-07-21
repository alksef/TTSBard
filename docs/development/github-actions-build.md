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

## Кэширование

Кэш npm уже включён в `actions/setup-node` через `cache: npm`. Для Rust следует
добавить после шага `Setup Rust`:

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
