# Plan 110: rate_limiter panic + WebViewPanel `as any`

**Дата:** 2026-07-11  
**Источник:** review-001-2026-07-11 (MINOR × 2)  
**Сложность:** Низкая — два точечных изолированных фикса.

---

## Fix 1: Panic в `rate_limiter.rs::with_config`

### Проблема
`with_config(requests_per_minute: u32, burst_size: u32)` вызывает
`NonZeroU32::new(x).unwrap()`. При аргументе `0` — panic.
Файл уже помечен `#[allow(dead_code)]`, но используется в тестах.

### Решение
Заменить тип параметров с `u32` на `NonZeroU32`. Компилятор
гарантирует ненулевость на стороне вызывающего кода. Убрать `.unwrap()`.

---

## Fix 2: `as any` в `WebViewPanel.vue`

### Проблема
```typescript
// WebViewPanel.vue:265-266
access_token: (newSettings as any).access_token || null,
upnp_enabled: (newSettings as any).upnp_enabled || false,
```
Тип `newSettings` (DTO из `get_webview_settings`) не содержит
`access_token` и `upnp_enabled` → обходят через `as any`.

### Решение
Найти Rust-структуру, которую возвращает `get_webview_settings`,
убедиться что поля там есть, и расширить (или создать) TypeScript-интерфейс
так, чтобы он содержал `access_token?: string | null` и `upnp_enabled: boolean`.
Убрать оба `as any`.

---

## Не трогать
- Логику WebView (запуск/остановка сервера).
- Остальные команды и composable.
- `rate_limiter.rs::new()` (там литералы, ненулевые — ок).
