# Локальный ONNX Runtime для debug-сборки Piper

## Вывод

Падение связано не с моделью Piper и не с `piper.exe`. Его вызывает зависимость
`ort = 2.0.0-rc.12`, через которую встроенный Piper runtime использует ONNX Runtime.
По умолчанию `ort` скачивает статическую Windows-сборку ONNX Runtime во время
Cargo build. Для текущего target это ONNX Runtime **1.24.2**, около 306 МБ.

Локальный runtime следует подключать только из ветки `debug` в
`scripts/build.ps1` через `ORT_CACHE_DIR`. Это сохраняет текущий официальный
механизм `ort`: он сам линкрует нужные Windows-библиотеки и копирует
`DirectML.dll` рядом с debug-exe. Release-сборка и GitHub Actions при этом не
меняются.

Не рекомендуется задавать `ORT_LIB_PATH` для файлов из автоматического кэша.
Этот режим переключает `ort-sys` в вариант «пользовательская библиотека» и
обходит часть обработки поставляемой сборки, включая копирование `DirectML.dll`.

## Что именно скачивает `ort`

Версия и URL зафиксированы в исходниках установленного пакета
`ort-sys-2.0.0-rc.12`:

```text
https://cdn.pyke.io/0/pyke:ort-rs/ms@1.24.2/x86_64-pc-windows-msvc.tar.lzma2
```

Ожидаемый SHA-256:

```text
b685bfc8d336e0ba95c066a7a982c03aa6dedd528a492eb99ca4ccb7f3af9e7a
```

После распаковки нужны два файла:

```text
onnxruntime.lib
DirectML.dll
```

## Подготовить локальный кэш один раз

Ниже каталог выбран вне репозитория, чтобы 306-МБ бинарники не попали в Git:

```powershell
$ortCacheRoot = Join-Path $env:LOCALAPPDATA 'TTSBard\ort-cache'
$hash = 'b685bfc8d336e0ba95c066a7a982c03aa6dedd528a492eb99ca4ccb7f3af9e7a'
$target = 'x86_64-pc-windows-msvc'
$runtimeDir = Join-Path $ortCacheRoot "dfbin\$target\$hash"
New-Item -ItemType Directory -Force $runtimeDir | Out-Null
```

Скачать архив и обязательно сверить контрольную сумму:

```powershell
$archive = Join-Path $env:TEMP 'ort-1.24.2-x86_64-pc-windows-msvc.tar.lzma2'
Invoke-WebRequest `
  -Uri 'https://cdn.pyke.io/0/pyke:ort-rs/ms@1.24.2/x86_64-pc-windows-msvc.tar.lzma2' `
  -OutFile $archive

(Get-FileHash $archive -Algorithm SHA256).Hash.ToLower()
```

Значение хэша должно в точности совпасть с указанным выше. Распаковать архив
можно 7-Zip (первый вызов создаёт `.tar`, второй извлекает файлы):

```powershell
$sevenZip = 'C:\Program Files\7-Zip\7z.exe'
& $sevenZip x $archive "-o$env:TEMP\ort-1.24.2" -y
& $sevenZip x "$env:TEMP\ort-1.24.2\ort-1.24.2-x86_64-pc-windows-msvc.tar" `
  "-o$runtimeDir" -y
```

Если архив распакован в промежуточную вложенную папку, перенести
`onnxruntime.lib` и `DirectML.dll` в корень `$runtimeDir`. Проверка:

```powershell
Get-Item "$runtimeDir\onnxruntime.lib", "$runtimeDir\DirectML.dll"
```

## Предлагаемое изменение debug-ветки скрипта

В блоке `if ($Mode -eq 'debug')` файла `scripts/build.ps1` добавить настройку
кэша до запуска `npm run tauri`:

```powershell
$ortCacheRoot = Join-Path $env:LOCALAPPDATA 'TTSBard\ort-cache'
$env:ORT_CACHE_DIR = $ortCacheRoot
Write-Ok "ONNX Runtime cache for debug: $ortCacheRoot"
```

Эта переменная существует только в дочернем процессе текущего PowerShell:

- `scripts/build-debug.bat` и `scripts/build.ps1 -Mode debug` используют
  заранее подготовленную локальную копию;
- `scripts/build.ps1 -Mode release` её не задаёт и сохраняет текущую схему;
- `Cargo.toml`, lockfile, CI и распространяемый installer не меняются.

Для предсказуемого офлайн-режима в этот же блок следует добавить раннюю
проверку `$runtimeDir\onnxruntime.lib`. При отсутствии файла скрипт должен
сообщать путь из раздела подготовки и завершаться до запуска долгой сборки;
иначе `ort` попытается скачать архив сам.

## Проверка

После внесения изменения запустить:

```powershell
.\scripts\build.ps1 -Mode debug
```

В подробном логе `ort-sys` не должно быть строки о скачивании. В
`src-tauri\target\debug` должен появиться `DirectML.dll` рядом с
`ttsbard.exe`.

Во время исследования `ort-sys` успешно собрался с локальными файлами. Полная
сборка затем остановилась на независимой зависимости `espeak-rs-sys`: bindgen
не нашёл `libclang.dll`. Это устраняется установкой LLVM или
`LIBCLANG_PATH`; диагностикой и автопоиском уже занимается `scripts/build.ps1`.

## Когда обновляется `ort`

При изменении версии `ort` или его feature-набора нужно заново взять URL, target
и SHA-256 из `build/download/dist.txt` соответствующего `ort-sys` в Cargo
registry. Старый путь кэша намеренно не следует переиспользовать: его хэш
привязан к точному архиву runtime.
