---
id: ROADMAP-099
status: completed
created: 2026-09-08
updated: 2026-09-08
related_tasks: []
---

# ROADMAP-099 — Notices непосредственно включённых сторонних компонентов

## Контекст

TTSBard распространяется по GPL-3.0-only и уже включает корневой `LICENSE` в
NSIS bundle. Вместе с приложением также распространяются сторонние файлы,
которые не полностью описываются Cargo/npm metadata: русский Hunspell-словарь,
сгенерированные данные eSpeak NG и vendored Signalsmith Stretch/Linear.

Проверка установила происхождение текущего словаря: локальные `ru.aff` и
`ru.dic` побайтно совпадают с `ru_RU/ru_RU.{aff,dic}` из LibreOffice
`dictionaries` commit `32b006a2c22a4ac7e8ed3f03346f7b3d85a970a4`.
Условия распространения и copyright находятся в upstream
`ru_RU/README_ru_RU.txt`. eSpeak NG включается через pinned `piper-rs` revision
`346f10f6e8520b2ee21e4d0117c1ca0e38e5cd0f`, где submodule зафиксирован на
eSpeak NG commit `724808c5a83f9ef95fdd0db886ba7ba537ff224a`.

## Цель

Сделать права и происхождение непосредственно включённых сторонних компонентов
видимыми получателю installer и защитить эти сведения от случайного удаления
простым reproducible check.

## Scope

1. Добавить один читаемый `THIRD_PARTY_NOTICES.md` с источниками, revisions,
   copyright и лицензиями русского словаря, eSpeak NG, Signalsmith Stretch и
   Signalsmith Linear.
2. Сохранить upstream license notice русского словаря рядом с файлами словаря.
   Существующие MIT license texts Signalsmith остаются source of truth в
   `src-tauri/vendor/` и включаются в bundle.
3. Добавить notice и необходимые license texts в Tauri NSIS resources; GPLv3
   eSpeak ссылается на уже включённый корневой `LICENSE` с тем же полным текстом.
4. Добавить `scripts/check-third-party-notices.ps1`, который проверяет hashes
   словаря, наличие notice/license files, pinned revisions и Tauri resource
   mappings.
5. Запускать check в CI и описать его в development/release documentation.

## Не входит

- полный реестр всех транзитивных Rust/npm dependencies;
- собственный SPDX parser или генератор license inventory;
- блокировка любой новой dependency только по неполной package metadata;
- UI-экран лицензий;
- изменение лицензии проекта или сторонних компонентов;
- проверка доступности внешних URL при каждой сборке.

## Outcome

Добавлен узкий и проверяемый контур notices для непосредственно
распространяемых сторонних assets без построения общего dependency inventory.

- `THIRD_PARTY_NOTICES.md` фиксирует provenance, copyright, license и точные
  LibreOffice, piper-rs и eSpeak NG revisions.
- Полный notice русского словаря хранится рядом с `ru.aff`/`ru.dic`; оба MIT
  license texts Signalsmith и общий notice включены в Tauri resources. Корневой
  GPLv3 `LICENSE` сохранён как `bundle.licenseFile`.
- Offline-check сверяет SHA-256 словаря, обязательные тексты, точные piper-rs
  pins в Cargo.toml/Cargo.lock и bundle mappings; CI запускает его отдельным
  шагом.
- `scripts/check-third-party-notices.ps1`, `scripts/check-docs.ps1` и
  `npm run build` прошли. Release build через `scripts/build.ps1 -Mode release`
  создал NSIS installer `TTSBard_0.27.1_x64-setup.exe`; staged notice и три
  отдельных license-файла присутствуют в ожидаемых путях и побайтно совпадают
  с source files.

Исходная широкая задача удалена: её dependency-inventory scope сознательно не
принят, а полезный минимальный результат закреплён здесь, в DECISION-017 и
build docs.
