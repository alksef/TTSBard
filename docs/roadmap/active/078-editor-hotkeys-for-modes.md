---
id: ROADMAP-078
status: in_progress
created: 2026-08-19
updated: 2026-08-19
related_tasks: []
---

# ROADMAP-078 — Горячие клавиши режимов редактора

## Контекст

Смена маршрута, переключение передачи набора, смена режима быстрого
редактора и показ истории требуют мыши. Все эти действия частые и «слепые» —
хоткеи в редакторе сокращают отрыв от клавиатуры. Инфраструктура
editor-scoped hotkeys уже существует (ROADMAP-068: HotkeyDto,
matchesEditorHotkey, set_editor_hotkey с duplicate-проверкой, HotkeysPanel).

## Цель

Четыре новых настраиваемых editor-хоткея с дефолтами:

| Действие | Дефолт | Поведение |
|---|---|---|
| cycle_route | `Ctrl+R` | следующий маршрут по ROUTE_ORDER (twitch_only пропускается, если Twitch не подключён) |
| toggle_typing | `Ctrl+T` | вкл/выкл «Передавать набор» (тот же код, что у иконки) |
| cycle_quick_mode | `Ctrl+W` | по кругу `disabled → collapse → return_focus → disabled`, персистится через `set_editor_quick` |
| toggle_history | `Ctrl+H` | показать/скрыть историю фраз (тот же showHistory, что у кнопки Clock) |

## Границы

- Хоткеи только при фокусе в editor scope (тот же контракт, что у
  next/previous_tab): `handleEditorScopeKeydown` в InputPanel.
- `preventDefault()` + `stopPropagation()` для поглощения браузерных
  дефолтов (Ctrl+R перезагрузка и т.п.).
- Не срабатывать при открытом autocomplete/SpellContextMenu — свериться с
  существующим арбитражем (shouldEnterSubmit и проверки в TtsEditor).
- Все биндинги настраиваемы в HotkeysPanel и попадают в duplicate-проверку.
- Смена quick mode отражается индикатором у кнопки отправки (уже работает
  реактивно от editorSettings).

## Этапы

1. Backend: 4 поля в EditorHotkeys-конфиге, дефолты, описания для
   duplicate-ошибок, регистрация в set/reset по образцу существующих.
2. Frontend: поля в HotkeySettingsDto, обработка в handleEditorScopeKeydown,
   повторное использование существующих функций (toggleTypingEnabled,
   handleRouteSelect по кругу, set_editor_quick, showHistory).
3. HotkeysPanel: четыре строки в разделе editor-хоткеев.
4. Тесты: keymapArbitration/matchesEditorHotkey на новые дефолты, чистая
   функция цикла quick-mode и маршрута.

## Критерии завершения

- все четыре действия работают с клавиатуры при фокусе в редакторе;
- биндинги переназначаются, конфликты детектируются;
- в minimal mode работают так же;
- npm test / build, cargo test зелёные.

## Не входит

- глобальные (не editor-scoped) хоткеи;
- изменение существующих дефолтов submit/edit_word/орфографии/вкладок.
