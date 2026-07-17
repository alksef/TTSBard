# План: отменять незавершённое Telegram-подключение при закрытии окна

## Цель

Если авторизация Telegram зависла на инициализации, запросе кода или вводе 2FA, закрытие модального окна должно остановить текущий backend-клиент. После смены proxy пользователь должен иметь возможность начать подключение заново.

## Scope

- `src-tauri/src/commands/telegram.rs`
- `src-tauri/src/lib.rs`
- `src/composables/useTelegramAuth.ts`
- `src/components/TelegramAuthModal.vue`

## Требования

1. Добавить отдельную Tauri-команду `telegram_disconnect`, которая:
   - берёт текущий `TelegramClient`, если он существует;
   - вызывает его существующий `disconnect()` для abort pool task и очистки pending auth;
   - удаляет client из `TelegramState`;
   - не удаляет сохранённый session-файл и не сбрасывает `api_id` в settings.
2. Добавить composable-метод `cancelConnection`, который вызывает `telegram_disconnect`, очищает auth UI state и не превращает отсутствие клиента в ошибку.
3. Изменить `TelegramAuthModal.close()` на async: если auth-flow не завершён (`state` не `connected` и не `idle`), сначала вызвать `cancelConnection`, затем закрыть окно. При закрытии уже подключённого аккаунта disconnect не вызывать.
4. Не менять поведение явной кнопки sign out: она по-прежнему должна удалять авторизацию/сессию согласно текущей логике.
5. Сохранить возможность закрыть окно во время зависшего loading; UI не должен ждать сетевой таймаут Telegram перед скрытием окна.
6. Не трогать пользовательские изменения в `src-tauri/Cargo.toml` и сохранить уже существующие UI-доработки модалки.

## Проверка

- `cargo check` из `src-tauri`;
- `npx vue-tsc --noEmit`;
- `git diff --check`.
