# Plan 66: Security Fixes — Cookie Parsing, Search Sanitization, External Links

## Overview

Three security issues:

1. Cookie parsing without URL-decoding
2. Search query passed to backend without sanitization
3. Missing `rel="noopener noreferrer"` on external links

---

## Issue 1: Cookie Parsing Without URL-Decoding

**Problem:** `get_cookie_from_headers()` in `src-tauri/src/webview/server.rs:241-252` parses cookie values raw. If the access token contains URL-special characters (e.g. `+`, `%`, `=`), the cookie value will be URL-encoded by the browser. Comparison with the raw access token will fail, blocking authentication.

**Analysis:** When a browser sends cookies, values are URL-encoded per RFC 6265. The `Set-Cookie` header at line 262 sets the raw token value, but the browser may re-encode it when sending it back. The `get_cookie_from_headers` function does a raw string comparison, so URL-encoded characters won't match.

**Fix:** Apply `urlencoding::decode()` to the extracted cookie value before returning it. The `urlencoding` crate (v2.1) is already a project dependency.

**File:** `src-tauri/src/webview/server.rs:248`

```rust
// Before:
parts.next().map(|s| s.to_string())

// After:
parts.next().map(|s| urlencoding::decode(s).unwrap_or(s.into()).into_owned())
```

---

## Issue 2: Search Query Passed to Backend Without Sanitization

**Problem:** `searchQuery` from `FishAudioModelPicker.vue:67` is passed via `invoke('fetch_fish_audio_models', { title: searchQuery.value || null })` to the Rust backend, which uses it in `FishTts::list_models()` at `src-tauri/src/tts/fish.rs:198` via `request.query(&[("title", title)])`.

**Analysis:** This is **not a real vulnerability**. The `reqwest` crate's `.query()` method percent-encodes query parameters automatically (via `url::Url::query_pairs_mut`). The `title` value is passed as a query parameter — it cannot inject headers, change the URL path, or modify the request body. The worst case is a malformed search returning no results or an API error, which is already handled by the existing `map_err`.

**Fix:** No code change required. Add a comment in `FishAudioModelPicker.vue` noting that reqwest handles URL-encoding, to prevent future confusion during audits.

---

## Issue 3: Missing `rel="noopener noreferrer"` on External Links

**Problem:** External links without `rel="noopener noreferrer"` allow the opened page to access `window.opener`, which can be exploited for tab-nabbing or phishing attacks.

**Affected files found via grep:**
- `src/components/TwitchPanel.vue:327` — `<a href="https://twitchtokengenerator.com" target="_blank" class="help-link">` — **missing `rel`**

**Already correct (no action needed):**
- `src/components/TelegramAuthModal.vue:134-136` — has `rel="noopener noreferrer"` ✓
- `src/components/tts/TelegramConnectionStatus.vue:157` — has `rel="noopener noreferrer"` ✓

**Note:** The review mentioned `src/components/tts/TtsSileroCard.vue:130-134`, but that file has no external links — it only contains a `ProviderCard` wrapper with no `<a>` tags.

**Fix:** Add `rel="noopener noreferrer"` to the link in `TwitchPanel.vue`.

**File:** `src/components/TwitchPanel.vue:327`

```html
<!-- Before: -->
<a href="https://twitchtokengenerator.com" target="_blank" class="help-link">

<!-- After: -->
<a href="https://twitchtokengenerator.com" target="_blank" rel="noopener noreferrer" class="help-link">
```

---

## Steps

1. Fix cookie URL-decoding in `src-tauri/src/webview/server.rs`
2. Add `rel="noopener noreferrer"` to external link in `src/components/TwitchPanel.vue`
3. Build and verify (TypeScript + Rust compilation)
