# План: завершить вход Telegram для аккаунтов с 2FA

## Цель

Исправить Telegram user-login flow: после корректного кода при включённом 2FA показывать пользователю шаг ввода пароля, передавать пароль в `grammers-client::Client::check_password` через backend и завершать авторизацию.

## Файлы в scope

- `src-tauri/src/telegram/types.rs`
- `src-tauri/src/telegram/client.rs`
- `src-tauri/src/commands/telegram.rs`
- `src-tauri/src/lib.rs` — только если потребуется регистрация новой команды
- `src/composables/useTelegramAuth.ts`
- `src/components/TelegramAuthModal.vue`

## Требования к реализации

### Backend

1. Расширить `AuthState` безопасным состоянием `PasswordRequired` (serde-значение должно быть пригодно для Tauri/TypeScript).
2. Добавить во внутреннее состояние `TelegramClient` pending `PasswordToken`. Не сериализовать и не логировать его.
3. В `sign_in(code)` при `SignInError::PasswordRequired(token)` сохранить token во внутреннем mutex и вернуть `Ok(AuthState::PasswordRequired)`, а не строковую ошибку.
4. Добавить метод `check_password(password: &str) -> Result<AuthState, String>`:
   - взять pending `PasswordToken` только на время попытки;
   - вызвать `grammers` `client.check_password(token, password)` с таймаутом;
   - при успехе вернуть `Connected` и выполнить ту же успешную post-login логику, что и для входа по коду;
   - при `InvalidPassword` вернуть понятную ошибку и сохранить token для повторной попытки;
   - при прочих ошибках вернуть безопасное сообщение и не логировать пароль/token/SRP-параметры.
5. Не менять поведение входа без 2FA, авт восстановления, sign-out и сохранения сессии.
6. Команда `telegram_sign_in` должна вернуть `AuthState`, а не выбрасывать `PasswordRequired` как `Err`. Добавить `telegram_check_password`, возвращающую `AuthState`.

### Frontend

1. Расширить `TelegramAuthState` значением `password_required`, добавить `needsPassword` computed.
2. `signIn(code)` должен читать результат команды: при `password_required` перейти в это состояние и не показывать общий error-state; при `connected` получить пользователя и завершить flow.
3. Добавить `checkPassword(password)` с тем же поведением: `connected` → загрузить user info; ошибка → остаться на `password_required` и показать сообщение.
4. В `TelegramAuthModal.vue` добавить поле пароля 2FA с toggle visibility, submit handler и кнопку назад/отмены. После неправильного пароля форма должна оставаться доступной.
5. Очистить пароль после попытки и при закрытии/reset; не писать пароль в debug-логи.
6. Не ломать существующие состояния credentials/code/error/connected и текущую визуальную систему модального окна.

## Acceptance criteria

- Без 2FA вход по коду работает как раньше.
- С 2FA после кода появляется поле пароля, а не ошибка «функция не реализована».
- Правильный пароль переводит состояние в `connected`, получает пользователя и сохраняет сессию.
- Неправильный пароль не теряет pending auth context и позволяет повторить ввод.
- Пароль, `PasswordToken`, login code и SRP-данные не попадают в логи или frontend state после завершения попытки.
- `cargo check` из `src-tauri` и `npx vue-tsc --noEmit` проходят.
