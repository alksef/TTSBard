# TASK-119 — Third-party notices для release bundle

**Статус:** `planned` — проект перелицензирован, полный реестр сторонних notices ещё не автоматизирован
**Связано:** [DECISION-017](../decisions/017-gpl-3-license.md)

## Контекст

`Cargo metadata` отражает license wrapper-crate, но не всегда условия
вложенного native-кода: `espeak-rs-sys` объявлен как MIT, хотя содержит eSpeak
NG под GPLv3. В bundle также входят Hunspell-словарь и vendored Signalsmith.
Одна корневая GPL-лицензия не заменяет notices этих компонентов.

## Цель

Сделать состав release bundle и сопровождающие license notices
воспроизводимыми и проверяемыми.

## Scope

1. Подтвердить происхождение и точную лицензию `resources/dict/ru.{aff,dic}`.
2. Сгенерировать реестр Rust/npm dependencies с copyright и license texts.
3. Добавить notices eSpeak NG, Hunspell dictionary и Signalsmith без
   перелицензирования их исходного содержимого.
4. Включить notice-файл в NSIS bundle рядом с корневым GPL `LICENSE`.
5. Проверить, что release по тегу предоставляет соответствующий исходный код и
   build scripts для опубликованной версии.
6. Добавить CI-проверку, обнаруживающую dependency/resource без известной
   лицензии.

## Критерии готовности

- Для каждого bundled стороннего компонента указаны источник, версия,
  copyright и лицензия.
- Инсталлятор содержит GPLv3 и third-party notices.
- Проверка воспроизводимо строится из lockfiles и явного списка ресурсов.
- Неизвестная или несовместимая лицензия блокирует release job.
