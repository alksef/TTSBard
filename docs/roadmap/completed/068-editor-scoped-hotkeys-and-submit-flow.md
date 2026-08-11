---
id: ROADMAP-068
status: completed
created: 2026-08-12
updated: 2026-08-12
related_tasks: []
---

# ROADMAP-068 — Локальные горячие клавиши и поток отправки редактора

## Контекст

Приложение различает глобальные shortcuts и одну локальную команду главного
окна, но persisted `HotkeySettings` остаётся плоским, а локальный обработчик
`return_previous_window` установлен на весь document. Внутри CodeMirror отдельно
существуют жёстко заданные Enter/Escape bindings.

Редактору нужна собственная область сочетаний, которая включает текст активной
вкладки и полосу вкладок, но никогда не регистрируется через Windows global
shortcut API. Первый keyboard-first сценарий — открыть орфографическое меню
`Ctrl+E`. Второй — набрать и поставить в очередь несколько фраз через
`Ctrl+Enter`, оставшись в редакторе, а последнюю отправить обычным `Enter` и
выполнить настроенное действие быстрого редактора.

## Цель

Ввести настраиваемые editor-scoped действия с отдельным блоком в панели горячих
клавиш и реализовать два ясных submit intent:

- **озвучить и продолжить** — остаёмся в редакторе;
- **озвучить и завершить быстрый ввод** — применяем quick-editor policy.

## Область действия

`Редактор` — локальная область главного окна, включающая CodeMirror и
`EditorTabs`. Доступность команды дополнительно зависит от контекста:

- действия над словом требуют фокуса текста CodeMirror;
- действия вкладок работают при фокусе текста или интерактивного элемента
  полосы вкладок;
- submit использует текст активной вкладки и не работает в настройках, аудио и
  других панелях приложения.

## Начальный набор действий

| Action id | Название | Default | Доступность |
|---|---|---|---|
| `edit_word` | Редактировать слово | `Ctrl+E` | курсор/одно слово в CodeMirror |
| `submit_continue` | Озвучить и продолжить | `Ctrl+Enter` | активная непустая вкладка |
| `next_spelling_error` | Следующая ошибка | `F7` | CodeMirror, есть диагностики |
| `previous_spelling_error` | Предыдущая ошибка | `Shift+F7` | CodeMirror, есть диагностики |
| `next_tab` | Следующая вкладка | `Ctrl+Tab` | область редактора |
| `previous_tab` | Предыдущая вкладка | `Ctrl+Shift+Tab` | область редактора |

Каждое действие настраивается и может быть отключено пустым binding. Стандартные
операции редактирования (`copy/paste/undo/select all`, стрелки, Home/End) не
попадают в настройки и не переопределяются.

## Продуктовый контракт

1. В `HotkeysPanel` есть отдельный блок **Редактор** с пояснением «Работают
   только внутри редактора и его вкладок».
2. Editor bindings сохраняются отдельно от global и main-window-local bindings
   и не вызывают регистрацию через `tauri-plugin-global-shortcut`.
3. Сопоставление использует физический `KeyboardEvent.code`: `Ctrl+E` означает
   `Ctrl+KeyE` и одинаково работает в раскладках `E` и `У`.
4. `preventDefault`/`stopPropagation` выполняются только после совпадения
   активного binding и подтверждения, что действие доступно.
5. Конфликты внутри editor scope запрещены. Конфликт editor binding с global
   binding также запрещён, поскольку global shortcut способен сработать при
   фокусе приложения. Конфликт с main-window-local либо запрещается, либо имеет
   один явно протестированный приоритет editor scope; для начальной реализации
   предпочтителен запрет.
6. `Ctrl+E` вызывает команду ROADMAP-067 у курсора/однословного выделения. При
   отсутствии ошибки текст не меняется и второе меню не создаётся.
7. `F7` и `Shift+F7` циклически перемещают курсор между актуальными spellcheck
   issues и делают целевой диапазон видимым.
8. `Ctrl+Tab` и `Ctrl+Shift+Tab` циклически переключают вкладки и возвращают
   фокус в текст новой активной вкладки.
9. `Ctrl+Enter` отправляет snapshot активной вкладки, записывает историю,
   очищает только неизменившийся sender tab, сохраняет окно открытым и возвращает
   фокус редактору. Quick-editor mode для этого intent не применяется.
10. Обычный `Enter` сохраняет текущий контракт: после успешной отправки очищает
    неизменившийся sender tab и выполняет `collapse`, `return_focus` либо остаётся
    в окне при `disabled`.
11. Пока предыдущая отправка не принята существующим submit pipeline, повторное
    нажатие не создаёт дубликат. Пользовательский ввод во время await не
    очищается ответом старой отправки.
12. Autocomplete, орфографическое меню и быстрый редактор имеют детерминированную
    матрицу приоритетов для `Enter`, `Ctrl+Enter`, `Escape`, `Tab` и назначенных
    editor bindings.

## Этапы

### P0 — Scoped settings и UI

1. Добавить обратно совместимую вложенную editor-группу в hotkey settings/DTO.
2. Добавить editor-specific save/reset path без перерегистрации глобальных
   shortcuts.
3. Вынести строки панели горячих клавиш в action definitions со scope, label,
   default и description либо эквивалентную типизированную модель без нового
   копирования разметки на каждое действие.
4. Добавить блок «Редактор», запись, сброс, отключение и сообщения о конфликте.

### P1 — Dispatcher и орфографические действия

1. Создать один matcher физических key codes и modifiers для editor scope.
2. Подключить dispatcher к области редактора, не устанавливая новый глобальный
   document listener.
3. Связать `edit_word`, `next_spelling_error` и `previous_spelling_error` с
   актуальным EditorView и diagnostics API ROADMAP-067.
4. Зафиксировать приоритеты относительно autocomplete и открытого spell menu.

### P2 — Submit intent `Ctrl+Enter`

1. Разделить общий submit pipeline и post-submit policy без дублирования
   `submitSpeech`, history и safe sender-tab clear.
2. Добавить intent `continue` для `Ctrl+Enter` и сохранить quick-editor intent
   для обычного `Enter`.
3. После успешного `continue` сфокусировать CodeMirror активной вкладки.
4. Покрыть очередь нескольких фраз, in-flight guard, смену/закрытие вкладки и
   редактирование текста во время await.

### P3 — Действия вкладок

1. Добавить циклические next/previous commands в owner вкладок.
2. Выполнить их из editor dispatcher для `Ctrl+Tab`/`Ctrl+Shift+Tab`.
3. Не перехватывать обычный `Tab`, используемый CodeMirror/autocomplete.
4. Проверить одну вкладку, несколько вкладок, закрытие текущей и восстановление
   фокуса.

### P4 — Документация и приёмка

1. Обновить руководство пользователя и карту сочетаний по фактически
   реализованным defaults и scopes.
2. Обновить архитектурное описание различия global, main-window-local и editor
   dispatcher.
3. После независимой приёмки перенести roadmap items в `completed/`, записать
   проверяемый Outcome и обновить индекс.

## Критерии завершения

- editor shortcuts не срабатывают за пределами редактора;
- физические bindings работают в английской и русской раскладках;
- изменение editor binding не перерегистрирует глобальные shortcuts;
- конфликтная комбинация не сохраняется и объясняется пользователю;
- `Ctrl+E`, `F7` и `Shift+F7` используют актуальные spellcheck ranges;
- серия `Ctrl+Enter`, `Ctrl+Enter`, `Enter` ставит три разные фразы в очередь, а
  quick-editor action выполняется только после последнего обычного `Enter`;
- async submit не очищает новый текст или другую вкладку;
- вкладки переключаются циклически и сохраняют editor focus;
- unit/component tests покрывают scope, matching, priority и submit policies;
- проходят frontend checks, settings parity, релевантные Rust-проверки через
  `scripts/cargo.ps1` и `./scripts/check-docs.ps1`.

## Не входит

- переназначение стандартных copy/paste/undo/navigation bindings;
- системные или глобальные F-клавиши вне главного окна;
- AI-коррекция по новому сочетанию;
- новая семантика speech queue, отмена или reorder;
- пользовательский словарь орфографии;
- автоматическое открытие меню при остановке курсора.

## Связанные материалы

- [ROADMAP-037 — глобальные и локальные hotkeys](../completed/037-application-hotkeys-and-previous-window-focus.md)
- [ROADMAP-047 — очередь задач озвучивания](../completed/047-speech-job-queue.md)
- [ROADMAP-048 — Enter/Escape и autocomplete](../completed/048-editor-autocomplete-enter-escape.md)
- [ROADMAP-067 — контекстное исправление орфографии](../completed/067-editor-spellcheck-context-menu.md)

## Outcome

- Persisted `hotkeys.editor` содержит шесть независимых editor-scoped bindings
  с defaults, миграцией старых settings, conflict validation и отдельными
  save/reset командами без регистрации Windows global shortcuts.
- Панель горячих клавиш показывает блок «Редактор»; записи используют
  физический key code для буквенных клавиш и работают в русской и английской
  раскладках.
- CodeMirror dispatcher реализует `Ctrl+E`, `F7`, `Shift+F7` и submit intent
  `Ctrl+Enter`; навигация вкладок по `Ctrl+Tab`/`Ctrl+Shift+Tab` остаётся в
  editor container и возвращает фокус тексту.
- Проверено `npx vue-tsc --noEmit`, `npx vitest run`, `npm run build`, settings
  parity и focused Rust hotkey tests через `scripts/cargo.ps1`.
