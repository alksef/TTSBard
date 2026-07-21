# DECISION-009 — Окна и хоткеи

**Статус:** `accepted`
**Заменяет:** [DECISION-015](./015-hardcoded-f6-mode.md)

## Контекст

Floating windows имеют разные focus/click-through/capture требования, а
фиксированные сочетания конфликтуют с приложениями пользователя.

## Решение

Окна создаются через общий Windows manager с явными режимами. Hotkeys задаются
валидируемой конфигурацией и UI recording; runtime хранит active window и
предыдущее foreground HWND.

## Последствия

Новый сценарий описывает focus restoration, hide/close и cleanup. Поведение не
привязывается к конкретной клавише вроде F6.
