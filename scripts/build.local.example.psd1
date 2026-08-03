# build.local.example.psd1 — пример локальной конфигурации сборки.
#
# Скопируйте этот файл в scripts/build.local.psd1 и отредактируйте
# под свою машину. scripts/build.local.psd1 игнорируется Git.
#
# Все ключи опциональны. Допустимые ключи: CargoTargetDir, RustBinDir.
# %NAME% в путях заменяется на значение переменной окружения.

@{
    # Альтернативная директория Cargo target (по умолчанию src-tauri\target).
    # Внешние target-директории требуют файла-маркера .ttsbard-build-target
    # перед очисткой. Рекомендуется сначала выполнить одну не-clean сборку.
    CargoTargetDir = '%USERPROFILE%\cargo-target\app-tts-v2'

    # Дополнительный каталог с Rust toolchain (имеет приоритет выше PATH).
    # Раскомментируйте, если нужно явно указать rustup bin.
    # RustBinDir = '%USERPROFILE%\.cargo\bin'
}
