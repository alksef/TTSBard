**English | [Русский](../../ru/user/audio-effects.md)**

# Audio effects and post-processing

TTSBard processes PCM audio before playback using Signalsmith Stretch,
DeepFilterNet and built-in DSP. Preview the result with a WAV or MP3 file.

## 1. Supported effects

### 1.1 Pitch

Range `-100..100`, corresponding to up to `±12` semitones. Signalsmith Stretch
changes pitch independently of tempo.

### 1.2 Tempo

Range `-100..100`, corresponding to `0.75×..1.50×`, without changing pitch.
Scale labels: `0.75×`, `0.80×`, `0.90×`, `1.00×`, `1.10×`, `1.25×`, `1.30×`, `1.50×`.

### 1.3 Volume

Range `0..200%`; `100%` is the original level. Volume is applied separately at playback.

### 1.4 Timbre preservation

Signalsmith Stretch has a separate timbre-preservation switch to reduce the
“chipmunk” effect and excessive voice distortion when changing pitch/tempo.

## 2. Phrase boundary cleanup

The separate boundary setting at the start of the effects section fixes abrupt
phrase starts/ends. Enabled by default, it is not a DSP preset. After other
per-phrase effects it detects sudden boundary jumps, adds short fades when
needed and conservatively corrects substantial DC offset. It preserves sample
rate, channels and duration and leaves ordinary phrases unchanged if no
significant jump is found.

Disable it for A/B comparison. Adjacent phrases do not currently overlap or
crossfade; queue lookahead is deferred.

## 3. Neural speech enhancement (DeepFilterNet)

Native Rust DeepFilterNet 3 inference runs in process to reduce noise and
soften synthesis artifacts. Noise reduction depth is `5..30 dB`:

- `5..12 dB`: gentle; recommended for natural sound.
- `15..20 dB`: stronger suppression.
- `25..30 dB`: maximum; may introduce unnatural artifacts.

DeepFilterNet runs before pitch/tempo. Normal TTS falls back to original audio
if denoising fails; preview instead reports an error.

## 4. DSP processing and presets

DSP includes EQ, compressor and limiter.

### Natural

Minimal processing for clean TTS: only the safety limiter is enabled. This
preset does not control boundary cleanup or DeepFilterNet.

### Clear

Gentle low-cut, a small presence boost, light compression and safety limiter
for intelligibility. It does not restore or generate missing frequencies.

### Custom

Changing any DSP parameter selects Custom automatically; changing DeepFilterNet
or boundary cleanup does not. Presets replace only DSP settings, which can be
saved separately from main voice effects.

## 5. Preview before saving

In **Audio → Effects**, choose a WAV/MP3 file:

- **Original** plays without effects.
- **With effects** uses current voice effects, DSP and DeepFilterNet, including
  unsaved changes.

Preview uses PCM playback; stop it, replace the file or clear the selection.
**Cancel** restores saved settings; **Save** applies settings to subsequent
TTS phrases. File preview does not run boundary cleanup or test adjacent phrase joins.

## 6. Current processing order

```text
TTS audio
  → decode to interleaved f32 PCM
  → DeepFilterNet (if enabled)
  → Signalsmith Stretch (tempo + pitch + formant correction)
  → DSP: EQ + compressor + limiter (according to preset)
  → boundary cleanup (if enabled)
  → ready PCM to playback
```

File preview takes a separate path without boundary cleanup.
[Output format](audio-devices.md#output-format) under **Audio → Devices** also
applies to preview. i32 changes device delivery; effects remain f32.
The stored/API field `speed` is retained for compatibility; its user-facing
meaning is tempo. Volume is applied separately during playback.

Editor export saves the original provider audio without DeepFilterNet, tempo,
pitch, DSP, boundary cleanup or volume changes, so you can compare processing
variants using the same source.
