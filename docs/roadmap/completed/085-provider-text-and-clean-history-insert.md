---
id: ROADMAP-085
status: completed
created: 2026-08-28
updated: 2026-08-28
related_tasks: []
---

# ROADMAP-085 — Provider text и чистая вставка из истории фраз

## Проблема

После расстановки ударений текст превращается в provider-specific форму:
Silero получает `+` перед ударной гласной, Piper — combining acute
`U+0301`. Эта же строка сейчас сохраняется в phrase history, вставляется
обратно в редактор и уходит в WebView/Twitch. В результате технические
маркеры могут ломать редактор и внешние текстовые потребители.

## Цель

Разделить два явных представления одной синтезированной фразы:

- `provider_text` — provider-specific текст, отправляемый в TTS, участвующий
  в cache key и отображаемый в истории, чтобы пользователь видел фактические
  ударения;
- `insert_text` — чистый текст без provider markers, используемый при
  Replace/Append из истории и для WebView/Twitch routing.

История не хранит отдельный `display_text`: им является `provider_text`.

## Границы и инварианты

- Повторная отправка вставленного `insert_text` проходит обычный pipeline и
  попадёт в кеш только если вновь полученный `provider_text`, provider, voice
  и эффекты совпали. Изменённая или удалённая расстановка ударений намеренно
  даёт cache miss.
- Ручные markers не переносятся скрытым состоянием через историю; после
  чистой вставки пользователь при необходимости вводит их заново.
- Старый `phrase_history.json` с полем `text` остаётся читаемым. Для известных
  legacy providers `insert_text` выводится из старого provider text безопасным
  снятием их markers; новые записи сохраняют оба явных поля.
- Direct cache replay остаётся воспроизведением по сохранённому `cache_key` и
  не запускает повторный synthesis.

## План

1. Расширить backend contract `PhraseEntry`, запись истории и migration
   legacy JSON.
2. Пронести `provider_text` и `insert_text` через speech pipeline: первый —
   только на synthesis/cache boundary, второй — в очередь/UI-routing и
   external text consumers.
3. Обновить IPC TypeScript contract и Phrase History: отображать
   `provider_text`, а в editor emit `insert_text`.
4. Добавить regression tests для legacy migration, выбора текста и отсутствия
   provider markers на внешнем маршруте; выполнить Rust, frontend и IPC
   проверки.

## Критерии готовности

- В истории Piper/Silero видны акценты/markers, но Replace/Append возвращают
  чистый текст.
- WebView и Twitch получают чистый текст.
- Cache key строится по `provider_text`; cache hit сохраняется для полностью
  совпадающего повторного pipeline result.
- Имеющиеся пользовательские JSON-файлы phrase history не теряются и
  корректно читаются после обновления.

## Outcome

- Введены отдельные `provider_text` и `insert_text` для истории и speech
  pipeline; provider-specific строка применяется только для synthesis и cache
  key.
- Legacy `phrase_history.json` с `text` читается без потери записей. Для Piper
  и Silero из него безопасно выводится чистый insert text, а новые записи
  сохраняются с двумя явными полями.
- History отображает provider text, но Replace/Append, WebView, Twitch и
  playback labels используют marker-free insert text.
