---
id: ROADMAP-034
status: completed
created: 2026-07-22
updated: 2026-07-25
related_tasks: []
---

# ROADMAP-034 — Silero voice metadata и playback recent dedup

## Outcome

Metadata голоса и дедупликация реализованы.

## Scope

Two focused fixes unrelated to persistent-cache architecture:

1. Persist the Silero voice selected through the generic speaker command so new phrase-history entries contain the voice.
2. Stop repeated replay of one persistent history cache entry from creating multiple records in the playback window's in-memory `recent` list.

The current `temp` versus `audio_cache` architecture is intentionally unchanged.
