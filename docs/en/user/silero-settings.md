**English | [Русский](../../ru/user/silero-settings.md)**

# Manual Silero timing settings

Telegram/Silero synthesis timeouts and delays are edited in `settings.json`;
there are no interface controls for these fields.

## Find settings.json

Under **Settings → General → Folders**, click **Open** next to **Configuration**.
The file is `%APPDATA%\ttsbard\settings.json`, for example
`C:\Users\<Name>\AppData\Roaming\ttsbard\settings.json`.

## Parameters

All values are milliseconds under `tts.telegram`.

| Key | Default | Range | Purpose |
|---|---|---|---|
| `synthesis_response_timeout_ms` | `10000` | `1000`…`120000` | Wait for Silero's response after sending text |
| `download_retry_delay_ms` | `1000` | `100`…`10000` | Delay before another download attempt |

## Example

```json
{
  "tts": {
    "telegram": {
      "synthesis_response_timeout_ms": 15000,
      "download_retry_delay_ms": 2000
    }
  }
}
```

Add fields individually; omitted fields use defaults.

## Normalization

Out-of-range values are clamped: `synthesis_response_timeout_ms: 0` becomes
`1000`, and `download_retry_delay_ms: 99999` becomes `10000`.

## Apply changes

**Restart the application** for changes to take effect. Live reloading is not supported.
