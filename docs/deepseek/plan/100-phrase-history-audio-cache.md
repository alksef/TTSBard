# Plan 100 — Быстрое воспроизведение аудиокеша из истории

Stage: `docs/stage/33-phrase-history-audio-cache.md`

## Результат

История фраз хранит сведения о конкретной аудиоверсии: provider, voice и стабильный cache key. В истории есть кнопка быстрого воспроизведения. Нажатие пытается открыть конкретный persistent cache entry; при отсутствии файла backend возвращает понятный cache miss, а UI показывает плашку без нового TTS-запроса.

## Архитектура

1. Расширить `PhraseEntry` backward-compatible полями `provider`, `voice`, `cache_key`.
2. В момент успешной подготовки аудио сохранять готовое к воспроизведению аудио в cache directory с атомарной записью.
3. Не использовать абсолютные пути в JSON истории; путь строится backend из безопасного cache key.
4. История должна различать аудиоверсии, поэтому dedup по одному только тексту больше не применять для записей с другим provider/voice/cache key; повтор той же аудиоверсии может увеличивать count.
5. Добавить backend command для replay по history entry/cache key. Он проверяет и открывает конкретный файл во время операции, без предварительного сканирования всей папки.
6. В `PhraseHistoryList.vue` добавить icon-only play action с `title` и `aria-label`; показывать provider/voice мелким meta-текстом. Ошибку cache miss показать в существующем UI-стиле ошибки.

## Разбиение

- Task 100-round1-01: backend model, persistent audio cache, recording metadata, replay command, tests.
- Task 100-round1-02: frontend DTO/composable/history controls and cache-miss UX after backend task passes review.

## Проверки

- `cargo test --manifest-path src-tauri/Cargo.toml --lib`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `npx vue-tsc --noEmit`
