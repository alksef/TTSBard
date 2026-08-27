---
id: ROADMAP-084
status: completed
created: 2026-08-28
updated: 2026-08-28
related_tasks: []
---

# ROADMAP-084 — Ошибки очереди речи: Silero и глобальное уведомление

## Outcome

Silero распознаёт две старые грамматические формы сообщения о превышении
лимита: `превысит` и `превыша` перед упорядоченными маркерами `лимит` и
`озвучк`. Оба варианта возвращают пользователю «Превышен лимит озвучки
Silero.», не ослабляя строгую корреляцию Telegram-ответа.

Главное окно теперь слушает существующее `speech-queue-changed` и показывает
глобальный error toast при новом `failed` transition основной очереди речи.
Механизм единый для всех TTS providers, использующих `submit_speech`:
Silero, Piper, OpenAI, Fish, Local и будущих providers этого контракта.

Повторные события той же ошибки не дублируют toast. Дедупликация учитывает
`job_id`, `attempt` и текст ошибки; failure следующей Retry-попытки показывает
новое уведомление. Playback Control остаётся persistent UI с причиной ошибки и
действиями Retry/Skip.

Текущая фраза в Playback Control получила ограниченную scroll-область: длинный
текст больше не вытесняет header, controls и activity list.

## Проблема

Текстовый отказ Silero о превышении лимита мог использовать форму «превышает
лимит озвучки», не попадавшую в прежний classifier. После принятия фразы
`submit_speech` ошибка любого provider возникала в фоновом worker и была видна
только в отдельном Playback Control, хотя backend уже публиковал typed
`speech-queue-changed`.

## Реализация

- `src-tauri/src/telegram/bot.rs` расширил классификацию limit rejection и
  получил регрессионные Rust-тесты.
- `src/composables/speechQueueFailureNotifications.ts` вычисляет новые failure
  transitions из валидного queue payload и сохраняет состояние дедупликации.
- `src/App.vue` подключил helper к lifecycle-managed Tauri listener и
  существующему `useErrorHandler`/`ErrorToasts`.
- `src-playback/PlaybackControlApp.vue` ограничил высоту текущей фразы и
  добавил вертикальную прокрутку без горизонтального overflow.

Прямые integration commands и export flow, обходящие `submit_speech`, остаются
вне этого outcome и используют свои локальные способы возврата ошибок.

## Проверка

- Rust classifier tests: 159 passed.
- Frontend unit tests: 639 passed; покрыты первый failure, отсутствие дубля,
  retry с новой попыткой, clearing failed key, пустая ошибка и malformed payload.
- `npm run check:ipc` — passed.
- `npm run build` — passed.
- `./scripts/check-docs.ps1` — passed до завершения статуса; повторяется после
  переноса roadmap item.
