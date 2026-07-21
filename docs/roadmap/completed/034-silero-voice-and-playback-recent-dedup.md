# ROADMAP-034 — Silero voice metadata и playback recent dedup

**Статус:** `completed` — metadata голоса и дедупликация реализованы

## Scope

Two focused fixes unrelated to persistent-cache architecture:

1. Persist the Silero voice selected through the generic speaker command so new phrase-history entries contain the voice.
2. Stop repeated replay of one persistent history cache entry from creating multiple records in the playback window's in-memory `recent` list.

The current `temp` versus `audio_cache` architecture is intentionally unchanged.
