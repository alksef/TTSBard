**English | [Русский](../../ru/user/language-packs.md)**

# Interface language and external language packs

TTSBard includes English and Russian. Add a language without rebuilding: copy
the matching release's `en.json`, translate values and put it in `locales`.

## Change the interface language

1. Open **Settings → General**.
2. Choose **Language**. Names appear in their own language: English, Русский
   and the names supplied by installed external packs.
3. The choice is saved and applies on the **next launch**. Existing windows
   and the tray keep the previous language until restart; the application tells you.

This does not change TTS/OCR/spelling languages, your text, voice names or saved
tab/set names. Language packs translate interface messages; they do not add
spelling dictionaries, RUAccent models or number conversion for another
language, and do not rewrite AI prompts. See [language limitations](guide.md#language-support).

## Add a translation

1. Get `en.json` matching the application version. An installed release includes
   it in the application's `locales` resource folder; source catalogs are in the
   repository's root `locales` directory.
2. Copy it to a language-code filename such as `de.json`. Use a valid tag such
   as `de`, `pt-BR`, `zh-Hant` or `sr-Latn-RS`.
3. Set `locale` to that filename's code, `name` to the language's native name
   (for example `Deutsch`), and translate **only values** inside `messages`.
   Keep keys and structure.
4. Under **Settings → General → Folders**, open **Configuration** and save the
   file in its `locales` subfolder (create it if missing). Windows search order:
   - `%APPDATA%\ttsbard\locales\`: recommended custom folder, retained on updates.
   - `locales\` beside the application.
5. Restart and select the language under Settings → General.

## Translation rules

- Keys such as `common.save`, `tray.quit` and `settings.title` are stable IDs:
  do not move or rename them.
- Placeholders such as `{count}`, `{detail}` and `{name}` must match the English
  string's set exactly; surrounding words can be reordered.
- Do not add unknown keys or remove existing ones. Partial packs are supported:
  missing messages use English, extra keys are ignored.
- Values cannot be empty. Keep valid JSON with `schemaVersion`, `locale`,
  `name` and `messages`.
- Executable HTML such as `<script>` is rejected.
- Technical details from third-party libraries/services may remain untranslated.

One pack covers all application windows and native tray menu items. Missing or
damaged packs fall back to English; the saved choice is restored when the file
becomes valid again.
