# Research: оверлей-браузер Steam после ввода и попадание оверлея в записи

> Состояние на 2026-09-18. Проверено по первичным источникам (документация
> Steamworks, OBS forums/issues, официальные страницы Steam/NVIDIA) и
> обсуждениям сообщества. Не является гарантией для конкретной версии Steam,
> игры или recorder'а.
> Итоги и связи — в [индексе исследований](./README.md).

**Дата:** 2026-09-18
**Статус:** завершённое исследование ограничений платформы; связано с
[ROADMAP-108](../roadmap/completed/108-local-web-input-form.md)

## Вопрос 1. Можно ли автоматически убрать окно браузера из оверлея после ввода?

**Короткий ответ: нет, из внешнего приложения это автоматизировать нельзя.
Рабочий цикл — Shift+Tab (скрыть оверлей), вкладка при этом сохраняется.**

Факты:

1. Steamworks позволяет игре только **открыть** оверлей-браузер
   (`ISteamFriends::ActivateGameOverlayToWebPage`) и реагировать на
   открытие/закрытие оверлея колбеком `GameOverlayActivated_t`. Публичного API
   «закрыть вкладку» или «скрыть оверлей» в документации нет — даже сама игра
   не может это сделать документированным способом
   ([Steam Overlay docs](https://partner.steamgames.com/doc/features/overlay),
   [ISteamFriends](https://partner.steamgames.com/doc/api/isteamfriends)).
2. Стороннее приложение (не процесс игры) не может вызывать Steamworks для
   чужой игры; иные пути (DLL-инъекция, hook ввода) — античит-риск, см.
   [исследование античита](./03-overlay-and-anticheat-compatibility.md).
3. Веб-страница тоже бессильна: `window.close()` закрывает только
   script-opened окна; навигация на `steam://`-протокол не является
   документированным способом скрыть оверлей и приведёт к потере вкладки.
4. Штатный цикл пользователя: ввёл реплику → `Enter` → `Shift+Tab` (оверлей —
   тумблер, игра получает ввод) → следующий `Shift+Tab` переоткрывает ту же
   вкладку. Вкладка остаётся открытой до конца сессии игры
   ([Steam Support: In-Game Overlay](https://help.steampowered.com/en/faqs/view/3978-072C-18DF-FBF9)).
5. Горячей клавиши «закрыть вкладку» в оверлей-браузере Steam нет
   ([обсуждение](https://www.reddit.com/r/Steam/comments/15o4k6u/steam_overlay_web_browser_keyboard_shortcuts/)).

## Вопрос 2. Попадает ли Steam Overlay в записи?

**Зависит от recorder'а. Собственная запись Steam и NVIDIA оверлей не пишут;
OBS Game Capture фильтрует его ненадёжно (DX11 — обычно да, DX12/Vulkan —
известный баг); Game Bar и display capture пишут всё.**

| Recorder | Оверлей в записи | Основание |
|---|---|---|
| Steam Game Recording (фоновая запись) | **Нет** | Официально «Record your games, not your desktop»; запись работает через тот же оверлейный механизм, но его UI в кадр не попадает ([страница функции](https://store.steampowered.com/gamerecording), [Steam Support](https://help.steampowered.com/en/faqs/view/23B7-49AD-4A28-9590)) |
| NVIDIA ShadowPlay / NVIDIA App, Instant Replay (режим по умолчанию) | **Нет** | Захватывается вывод игры, а не рабочий стол; оверлей попадает только при включённом «Desktop capture» ([NVIDIA Support](https://nvidia.custhelp.com/app/answers/detail/a_id/5602/)) |
| OBS **Game Capture**, DX11 | Можно исключить: ПКМ по источнику → снять галку «Capture third-party overlays (such as steam)» | [OBS forum](https://obsproject.com/forum/threads/any-way-to-hide-steam-overlay-when-using-game-capture-on-directx-12-games.94593/) |
| OBS **Game Capture**, DX12/Vulkan | **Ненадёжно**: оверлей часто попадает даже с выключенной галкой | Открытые баги OBS: [#3946](https://github.com/obsproject/obs-studio/issues/3946), [#5630](https://github.com/obsproject/obs-studio/issues/5630) |
| Xbox Game Bar (Windows.Graphics.Capture), display capture, «Desktop capture» в NVIDIA | **Да** | Пишется итоговая композиция окна/экрана; оверлей рисуется в swapchain игры (см. [exclusive fullscreen](./05-exclusive-fullscreen-overlay-problem.md)) |

Замечания:

- Утверждение «window capture не включает оверлеи» из обсуждений сообщества
  считать недостоверным: оверлей рисуется в swapchain игры, а не отдельным
  слоем.
- Самый простой путь для записи без оверлея — встроенная фоновая запись Steam:
  не пишет его по дизайну и не требует настройки OBS.

## Продуктовые альтернативы (зафиксированы, не реализованы)

1. Подсказка «Shift+Tab — вернуться в игру» в статусе формы после успеха.
2. Если запись критична и recorder нельзя поменять: собственное плавающее окно
   ввода с `WDA_EXCLUDEFROMCAPTURE` (механизм уже есть в
   `src-tauri/src/window.rs`); ограничение — exclusive fullscreen, риск —
   [исследование античита](./03-overlay-and-anticheat-compatibility.md).
   Требует отдельного roadmap-решения.
