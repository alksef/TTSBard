# DECISION-001 — Windows platform

**Статус:** `accepted`

## Контекст

Перехват произвольного ввода, foreground window и специальные стили окон
требуют Win32 API.

## Решение

TTSBard является Windows desktop-приложением. Платформенные возможности
изолируются в модулях окон, hotkeys и hooks; Windows 10/11 — целевая среда.

## Последствия

Можно использовать `WH_KEYBOARD_LL`, HWND и capture protection, но приложение
не обещает кроссплатформенность и не обходит Secure Desktop или более высокий
уровень привилегий.
