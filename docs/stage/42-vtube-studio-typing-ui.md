# Stage 42 — VTube Studio: typing-state и UI интеграции

## Что поставляем

Этот stage доводит существующий VTube Studio MVP до понятного и устойчивого
вертикального среза:

1. TTSBard подключается и авторизуется как внешний VTube Studio API plugin.
2. Пользовательская правка главного редактора переводит параметр
   `TTSBardTyping` в `1`.
3. После настраиваемого периода без правок параметр возвращается в `0`.
4. Панель VTube Studio выглядит и ведёт себя как панели WebView и Twitch:
   карточки настроек, проверка подключения, валидация порта, статус последней
   проверки и понятное описание передаваемого параметра.

## Что уже существует

- Rust WebSocket service, token authentication и сохранение токена.
- Создание custom parameter `TTSBardTyping`.
- Injection `1/0` и keep-alive `1` каждые 500 мс.
- Источник пользовательских правок CodeMirror, который не реагирует на
  `ExternalUpdate`.
- Сброс typing перед отправкой и при размонтировании `InputPanel`.

## Обнаруженный разрыв

До Stage 42 frontend вызывал `set_vtube_studio_typing(true)` на каждую правку.
Поэтому каждое нажатие повторно отправляло `1` в WebSocket и перезапускало
backend keep-alive. Это расходилось с research-контрактом.

Правильная burst-семантика:

```text
idle --первое user-edit--> typing=true
typing --следующее user-edit--> только перезапуск idle timer
typing --таймаут/Enter/Escape/hide/unmount--> typing=false
```

Переходы `true/false` должны быть сериализованы, чтобы поздний async `true` не
мог выполниться после `false`.

## Настройка редактора

Добавить в `EditorSettings`:

```text
typing_idle_timeout_ms: u32
default: 800
min: 200
max: 5000
UI step: 100
```

В интерфейсе «Настройки → Редактор»:

- подпись: «Завершать состояние набора через»;
- числовое поле в миллисекундах;
- пояснение: начало передаётся сразу, задержка считается после последней
  пользовательской правки.

Эта настройка управляет только frontend idle timeout. Protocol keep-alive VTube
Studio остаётся фиксированным внутренним интервалом 500 мс.

## Граница WebView

WebView не является источником typing-state. Его текущий pipeline получает
только готовый `AppEvent::TextSentToTts` после обработки/синтеза и отправляет
SSE payload `{"text": ...}`.

В этом stage WebView SSE не расширяется и черновик редактора наружу не
передаётся. VTube Studio получает только логический bool, но не текст.

## UI VTube Studio

- Сохранить общую структуру WebView/Twitch: `settings-section`, header со
  status badge и одной action-кнопкой, строки полей, footer/save semantics.
- Убрать дублирующие вызовы одной и той же проверки.
- Статус называть последней проверкой (`Не проверено`, `Проверка…`,
  `Проверено`, `Ошибка`), потому что live status events пока отсутствуют.
- Валидировать порт `1024..65535` до Tauri invoke.
- Добавить карточку «Статус набора» с именем `TTSBardTyping`, значениями
  `1 = печатает`, `0 = ожидание` и ссылкой на интервал в настройках редактора.
- Использовать только существующие semantic CSS variables и корректно
  раскладывать поля/кнопки на узкой ширине.

## Backend resilience

- При ошибке injection/transport очищать нерабочий cached WebSocket и auth
  state, чтобы следующий `typing=true` мог переподключиться.
- `typing=false` при отсутствии активного WebSocket не должен создавать новое
  соединение только ради idle/reset.
- Disconnect по-прежнему пытается отправить `0`, если живое соединение есть.

## Non-goals

- typing-state в WebView/OBS;
- lip-sync, speaking/thinking и VTS hotkeys;
- UDP discovery;
- настоящий push-based live connection badge;
- Item/VFX/модельные профили.

## Приёмка

1. Один burst правок отправляет ровно один `true` и один `false`.
2. Изменение интервала применяется к следующему user edit без перезапуска.
3. Enter, Escape/hide и unmount гарантированно завершают typing.
4. Программные изменения редактора не запускают typing.
5. Панель не показывает `Подключено` как live-факт; успешный результат —
   `Проверено`.
6. Невалидный порт не вызывает save/test backend.
7. Light/dark theme и узкая ширина не требуют отдельных цветов или
   горизонтального скролла.
8. Focused Vitest, `vue-tsc`, Rust tests/check и production frontend build
   проходят.
