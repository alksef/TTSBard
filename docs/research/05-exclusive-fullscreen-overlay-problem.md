# Research: Exclusive Fullscreen — несовместимость главного окна TTSBard с играми в эксклюзивном полноэкранном режиме

**Дата:** 2026-07-11  
**Причина:** Cult of the Lamb — alt+tab гасит игру в чёрный экран, горячая клавиша сворачивает игру, оверлей не виден.  
**Статус:** ресёрч завершён, решения сформулированы, реализация не начата.

---

## Контекст: два режима полноэкранного запуска игр

| Режим | Что происходит | Alt+Tab | Оверлей HWND_TOPMOST |
|---|---|---|---|
| **Exclusive Fullscreen** | D3D SwapChain захватывает GPU напрямую, DWM отключается | Чёрный экран, потеря D3D контекста | **Физически невозможен** — нет compositor |
| **Borderless Windowed** | Игра — обычное окно без рамки, DWM активен | Мгновенный, без потерь | Отображается нормально |

Cult of the Lamb по умолчанию запускается в **Exclusive Fullscreen**. Именно поэтому все три проблемы возникают только с ней (и аналогичными играми), а не с большинством современных игр на Unity/Unreal, которые идут в Borderless.

---

## Диагностика трёх симптомов

### 1. Alt+Tab → чёрный экран

**Механизм:** В Exclusive Fullscreen режиме DirectX/DXGI захватывает back-buffer GPU в монопольном режиме (`DXGI_SWAP_EFFECT_SEQUENTIAL` или `DXGI_SWAP_CHAIN_FLAG_ALLOW_MODE_SWITCH`). Любое событие, вызывающее переключение активного окна (включая появление `HWND_TOPMOST` окна), заставляет Windows послать игре `WM_ACTIVATEAPP(FALSE)` → DirectX теряет устройство → GPU контекст сбрасывается → чёрный экран пока контекст не восстановлен.

**Вклад TTSBard:** два окна с `alwaysOnTop: true` объявлены в `tauri.conf.json` и существуют в Z-order всегда, пока приложение запущено:
- `soundpanel` — `"alwaysOnTop": true`  
- `playback-control` — `"alwaysOnTop": true`

Их присутствие само по себе нестабилизирует exclusive fullscreen при любом переключении.

### 2. Горячая клавиша (`Ctrl+Shift+F3`) сворачивает игру вместо показа TTSBard

**Механизм:** В `hotkeys.rs → handle_main_window()` вызывается:
```rust
let _ = window.set_always_on_top(true);  // SetWindowPos(HWND_TOPMOST, ...)
let _ = window.set_focus();               // SetForegroundWindow(hwnd)
```

`SetForegroundWindow` → Windows переключает активное окно → игра получает `WM_ACTIVATEAPP(FALSE)` → exclusive fullscreen выходит из режима → игра сворачивается.

Это **фундаментальное ограничение Windows**: нельзя показать активное окно над exclusive fullscreen игрой без её сворачивания. Это не баг TTSBard — это архитектурное ограничение ОС.

### 3. Оверлей soundpanel/playback-control не отображается поверх игры

**Механизм:** В Exclusive Fullscreen DWM (Desktop Window Manager) полностью отключается. DWM — это compositor, который накладывает содержимое всех окон поверх друг друга. Без него:
- DirectX пишет напрямую в front-buffer GPU
- `HWND_TOPMOST` окна существуют в Z-порядке WinAPI, но **не рендерятся** в front-buffer игры
- Результат: окно как будто есть, курсор на него реагирует, но оно визуально невидимо

---

## Корневая проблема: главное окно принципиально нельзя показать

Это не решается добавлением `WS_EX_NOACTIVATE` к главному окну. Причина:

1. **Главное окно требует ввода** (редактор текста, CodeMirror). Для ввода нужен фокус. Окно с `WS_EX_NOACTIVATE` фокус не получает → пользователь не сможет печатать.

2. **Exclusive fullscreen нельзя обойти с уровня WinAPI без хука D3D.** Единственный способ показать произвольный контент поверх D3D exclusive fullscreen — это [hook IDXGISwapChain::Present](https://github.com/Rebzzel/kiero) (именно так работают Steam Overlay, Discord Overlay, NVIDIA GeForce Experience). Это требует DLL injection в процесс игры — неприемлемо для TTS-приложения (детектируется античитом, нарушает ToS игр).

**Итог:** Показ главного окна TTSBard поверх exclusive fullscreen игры — **архитектурно невозможен** на уровне WinAPI без хука D3D. Это системное ограничение Windows, а не баг.

---

## Что можно сделать

### Направление A: Пользователь переключает игру в Borderless Windowed

Единственное 100% рабочее решение для **показа главного окна** сегодня.

- Cult of the Lamb: Настройки → Видео → Режим экрана: Borderless / Оконный без рамки
- После этого все три проблемы исчезают немедленно

**Ограничение:** требует ручных действий пользователя; не все игры поддерживают Borderless.

---

### Направление B: Минимизировать побочный ущерб (не решает, но улучшает)

Даже если главное окно показать нельзя — soundpanel и playback-control теоретически могут работать в ограниченном режиме, если перестать красть фокус.

#### B1: `WS_EX_NOACTIVATE` для soundpanel и playback-control

Флаг `WS_EX_NOACTIVATE` запрещает окну захватывать фокус при показе. В сочетании с `SWP_NOACTIVATE` в `SetWindowPos` — окно существует в TOPMOST Z-порядке, не вызывая `WM_ACTIVATEAPP(FALSE)` у игры.

**Что это даёт:**
- Alt+Tab перестаёт гасить игру при простом наличии TTSBard в фоне
- soundpanel/playback-control появляются без сворачивания игры (хотя всё равно не видны в exclusive fullscreen — только в borderless)
- Горячая клавиша soundpanel перестаёт сворачивать игру

**Что это НЕ даёт:**
- Ввод текста в WS_EX_NOACTIVATE окно через DOM невозможен (нет фокуса, нет WM_KEYDOWN)
- Главное окно по-прежнему нельзя сделать полностью функциональным без кражи фокуса
- В exclusive fullscreen оверлей всё равно не виден

**Реализация:**
```rust
// window.rs
#[cfg(windows)]
pub fn set_window_noactivate(hwnd: isize) -> anyhow::Result<()> {
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::*;
        let hwnd = HWND(hwnd as *mut _);
        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
        SetWindowLongW(hwnd, GWL_EXSTYLE, ex_style | WS_EX_NOACTIVATE.0 as i32);
        SetWindowPos(
            hwnd, HWND_TOPMOST, 0, 0, 0, 0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        )?;
    }
    Ok(())
}
```

Применять после `show_soundpanel_window()` и `show_playback_window()`.

#### B2: Убрать `set_always_on_top` + `set_focus` из хоткея главного окна

В `hotkeys.rs → handle_main_window()` текущий код:
```rust
let _ = window.set_always_on_top(true);  // крадёт место в Z-топе
let _ = window.set_focus();               // SetForegroundWindow → сворачивает игру
```

Альтернатива: использовать `ShowWindow(hwnd, SW_SHOWNOACTIVATE)` — окно становится видимым, но без кражи фокуса. Пользователь сам кликает на него, когда game переходит в windowed. Не идеально, но не ломает игровой сеанс.

```rust
#[cfg(windows)]
pub fn show_window_noactivate(hwnd: isize) -> anyhow::Result<()> {
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::*;
        ShowWindow(HWND(hwnd as *mut _), SW_SHOWNOACTIVATE);
    }
    Ok(())
}
```

---

### Направление C: Keyboard input через WH_KEYBOARD_LL (существующий механизм)

У TTSBard уже есть `WH_KEYBOARD_LL` hook для soundpanel и intercept режима. Этот hook получает все нажатия клавиш **независимо от того, какое окно в фокусе**. Это открывает путь к частичному управлению без фокуса:

**Идея:** при нажатии горячей клавиши — **не показывать главное окно** вообще, а активировать «режим ввода через хук». Пользователь печатает, текст перехватывается hook'ом и накапливается в буфере, Enter отправляет в TTS. Визуальная обратная связь — через уже существующее `playback-control` окно (которое можно показать с `WS_EX_NOACTIVATE`).

**Что нужно:**
- Добавить «режим ввода через хук» (аналог существующего `Intercept`, но с полноценным редактором-буфером)
- Показывать текущий буфер ввода в playback-control или отдельном mini-overlay окне с `WS_EX_NOACTIVATE`
- Enter/Escape работают через hook, не через фокус окна

**Ограничения:**
- Нет полноценного CodeMirror, автодополнения, вкладок
- Нужно реализовать отдельный мини-редактор в Rust-хуке или в overlay-окне

---

### Направление D: Детекция режима игры и авто-предупреждение

Определять, запущена ли полноэкранная игра, и показывать пользователю уведомление:

```rust
// Псевдокод: проверить, есть ли окно с WS_EX_TOPMOST + WS_EX_LAYERED + fullscreen
fn detect_exclusive_fullscreen() -> bool {
    // EnumWindows → проверить размер == монитор + GetWindowLong(GWL_STYLE)
    // нет WS_CAPTION + WS_BORDER → кандидат на fullscreen
}
```

Если обнаружена exclusive fullscreen игра — при нажатии хоткея показать toast/balloon уведомление из трея (через `tauri-plugin-notification`) вместо попытки показать главное окно.

---

## Сравнение направлений

| Направление | Решает показ гл. окна | Решает overlay | Сложность | Риск |
|---|---|---|---|---|
| A: Borderless Windowed (пользователь) | ✅ | ✅ | Нулевая | Нет |
| B1: WS_EX_NOACTIVATE для оверлеев | ❌ | Частично (только в borderless) | Низкая | Нет |
| B2: SW_SHOWNOACTIVATE для гл. окна | ❌ (окно без фокуса → нет ввода) | — | Низкая | Нет |
| C: Ввод через WH_KEYBOARD_LL хук | ✅ (функционально, без GUI) | ✅ (mini-overlay) | Высокая | Конфликт с античитом |
| D: Детекция + уведомление | ❌ (только информирует) | ❌ | Средняя | Нет |

---

## Рекомендуемый путь

1. **Документировать** ограничение в UI: при открытии настроек или первом запуске — показать подсказку «для работы в игре используйте Borderless Windowed».
2. **Реализовать B1** (`WS_EX_NOACTIVATE` для soundpanel и playback-control) — не решает главное, но убирает раздражающее сворачивание при alt+tab.
3. **Долгосрочно рассмотреть C** — «режим ввода в игре» через существующий WH_KEYBOARD_LL с mini-overlay без фокуса. Это самый сложный, но единственный путь к по-настоящему игровому workflow без требования Borderless.

---

## Приложение: D3D Hook — как это делают Steam и Discord

### Механизм работы

Steam Overlay и Discord Overlay решают ту же задачу (рисовать поверх exclusive fullscreen) через **DLL injection + vtable hook**. Это единственный технически рабочий метод. Работает так:

```
Запуск игры
  → Steam/Discord через CreateRemoteThread или AppInit_DLLs
    внедряют GameOverlayRenderer64.dll / DiscordOverlay.dll в процесс игры
  → DLL при загрузке берёт адрес IDXGISwapChain::Present() из vtable
  → Заменяет указатель на свою функцию (vtable hook / detour)
  → Теперь каждый вызов Present() проходит через их код
  → Они рисуют свой UI (ImGui / собственный рендерер) поверх кадра
    прямо в тот же back-buffer, до того как D3D отправит его на экран
  → Вызывают оригинальный Present()
```

**Ключевые точки:**
- Внедрение: `CreateRemoteThread` + `LoadLibrary` (классика) или через `AppInit_DLLs` реестр
- Хук: vtable patch на `IDXGISwapChain::Present` (или `IDXGISwapChain1::Present1` для DXGI 1.2+)
- Рендеринг: напрямую в D3D device/context игры, который уже есть в памяти процесса
- Ввод: перехват `WM_KEYDOWN` / Raw Input в том же процессе

Библиотеки, которые реализуют этот паттерн (open-source):
- **[kiero](https://github.com/Rebzzel/kiero)** (C++) — Universal graphical hook: D3D9–D3D12, OpenGL, Vulkan
- **[hudhook](https://github.com/veeenu/hudhook)** (Rust) — "A videogame overlay framework written in Rust, supporting DirectX and OpenGL". Это Rust-крейт, написанный именно для game overlay, использует ImGui для рендеринга.

---

### Как Steam и Discord проходят через античиты

**Ответ: они не "проходят" — их специально пропускают.**

Это принципиально важный момент: Steam и Discord не обходят античиты — их **явно вайтлистят** на нескольких уровнях:

#### Уровень 1: Доверие по подписи издателя (Code Signing)
- `GameOverlayRenderer64.dll` подписан сертификатом **Valve Corporation**
- `DiscordOverlay.dll` подписан сертификатом **Discord, Inc.**
- EAC и BattlEye ведут внутренние списки доверенных издателей. DLL с валидной подписью известного вендора автоматически попадают в "low-risk" категорию.
- Но **подпись одна не гарантирует пропуск** — нужно ещё следующее.

#### Уровень 2: Явный allowlist в конфигурации игры
- **Easy Anti-Cheat (EAC):** Разработчик игры при интеграции EAC получает конфиг-файл (`EasyAntiCheat/Settings.json` или аналог). В нём можно явно разрешить конкретные модули по имени или хешу. Valve и Discord договорились с EAC/Epic Games, что их оверлеи включены в **глобальный whitelist по умолчанию** для всех игр на EAC.
- **BattlEye:** Аналогичная система. BattlEye прямо [указывает в FAQ](https://www.battleye.com/support/faq/): "We do not block overlays from trusted sources such as Steam". Разработчик игры может дополнительно настроить allowlist.
- **Riot Vanguard:** Работает иначе — kernel-driver уровень. Он видит всё что происходит в системе. Steam overlay разрешён через явное партнёрство Riot/Valve.

#### Уровень 3: Точка внедрения — до запуска античита
- Steam запускает игру через свой launcher и **инжектирует оверлей ещё до того**, как EAC/BattlEye успевает загрузиться и занести DLL в "красный список".
- К моменту, когда античит начинает сканирование, `GameOverlayRenderer64.dll` уже в памяти как "часть исходной загрузки" а не как внешнее вмешательство.

#### Итог по античитам:
| Механизм доверия | Steam | Discord | Произвольная DLL (TTSBard) |
|---|---|---|---|
| Code signing (известный вендор) | ✅ Valve | ✅ Discord Inc. | ❌ Нет сертификата или неизвестный |
| Глобальный EAC/BE whitelist | ✅ Договорной | ✅ Договорной | ❌ Нет договорённости |
| Инжект до старта AC | ✅ Steam launcher | ✅ Discord launcher | ❌ Внешний процесс |
| Kernel-level (Vanguard) | ✅ Партнёрство | ⚠️ Частично | ❌ |

**Произвольная DLL без этих условий будет обнаружена и заблокирована** большинством античитов. В лучшем случае — kick из игры, в худшем — бан аккаунта.

---

### Оценка реализации D3D hook для TTSBard

#### Что технически нужно:

1. **Отдельная DLL (`.dll`)** на Rust или C++ — именно DLL, не exe-процесс. TTSBard сейчас — `.exe`, это несовместимо.
2. **Механизм инжекции** DLL в процесс игры (`CreateRemoteThread` + `LoadLibrary` из TTSBard.exe).
3. **vtable hook** `IDXGISwapChain::Present` внутри DLL — с помощью `hudhook` (Rust) или `kiero` (C++).
4. **IPC между DLL и TTSBard.exe** — для передачи текста, статуса TTS. Варианты: named pipe, shared memory, socket.
5. **UI-рендеринг внутри DLL** — ImGui через `hudhook`, или только текстовый дисплей.

#### Оценка по крейту `hudhook`:

```toml
# Cargo.toml (в отдельном crate типа cdylib)
[lib]
crate-type = ["cdylib"]

[dependencies]
hudhook = "0.7"  # поддерживает D3D9/D3D11/D3D12/OpenGL
```

`hudhook` предоставляет:
- Автоматический hook Present для нужного D3D backend
- ImGui рендеринг поверх игры
- Обработку ввода (мышь, клавиатура) внутри overlay

**Cult of the Lamb** использует Unity → скорее всего D3D11 или D3D12 backend. `hudhook` это поддерживает.

#### Схема архитектуры:

```
TTSBard.exe (основной процесс)
  ├─ Глобальные хоткеи (WH_KEYBOARD_LL)
  ├─ TTS провайдеры
  ├─ Named Pipe Server ←──────────────────┐
  └─ InjectDLL(game_pid, ttsbard_hook.dll)│
                                          │
ttsbard_hook.dll (инжектирована в игру)   │
  ├─ IDXGISwapChain::Present hook         │
  ├─ ImGui overlay UI (мини-редактор)     │
  ├─ WH_KEYBOARD_LL внутри игры           │
  └─ Named Pipe Client ────────────────────┘
       ↓ (IPC)
    Текст → TTSBard.exe → TTS
```

#### Риски и ограничения:

| Аспект | Оценка |
|---|---|
| **Античит:** EAC/BattlEye | ❌ **Высокий риск бана** без whitelist договорённости |
| **Античит:** игры без AC (Cult of the Lamb) | ✅ Должно работать |
| **Сложность разработки** | 🔴 Очень высокая: отдельный DLL crate, IPC, ImGui UI |
| **Стабильность** | ⚠️ DLL crash = crash игры; сложно отлаживать |
| **Поддержка D3D версий** | ✅ hudhook покрывает D3D11/D3D12/OpenGL |
| **Code signing** | ❌ Нет EV сертификата → Windows Defender SmartScreen предупреждение |
| **Обновления игры** | ⚠️ Смена D3D backend → hook перестаёт работать |

#### Вывод по D3D hook для TTSBard:

**Технически реализуемо** (есть Rust-инструментарий — `hudhook`), но:

1. **Для игр без античита** (включая Cult of the Lamb) — будет работать, после реализации.
2. **Для игр с EAC/BattlEye/Vanguard** — заблокируется без специальных договорённостей, которых у indie-проекта нет.
3. **Сложность** несоразмерна масштабу проекта: полноценная DLL, IPC, ImGui — это фактически второй полноценный подпроект внутри TTSBard.
4. **Альтернатива для игр без AC:** Cult of the Lamb попадает в безопасную категорию (нет античита), но **переключение в Borderless решает проблему с нулевой сложностью**.

**Рекомендация:** D3D hook — отметить как "Future / Experimental", реализовывать только если появится чёткий запрос от пользователей, которые не могут использовать Borderless и играют в игры без античита.

---

## Связанные файлы

- `src-tauri/tauri.conf.json` — `alwaysOnTop: true` у soundpanel и playback-control
- `src-tauri/src/hotkeys.rs` — `handle_main_window()`: `set_always_on_top` + `set_focus`
- `src-tauri/src/soundpanel_window.rs` — `show_soundpanel_window()`: `set_focus`
- `src-tauri/src/playback_window.rs` — `show_playback_window()`
- `src-tauri/src/window.rs` — `SetWindowDisplayAffinity` (место для новых WinAPI утилит)
- `docs/research/03-overlay-and-anticheat-compatibility.md` — смежный ресёрч

## Внешние ссылки

- [kiero — C++ universal D3D hook library](https://github.com/Rebzzel/kiero)
- [hudhook — Rust videogame overlay framework](https://github.com/veeenu/hudhook)
- [Steamworks: Steam Overlay documentation](https://partner.steamgames.com/doc/features/overlay)
- [BattlEye FAQ — Overlays policy](https://www.battleye.com/support/faq/)
