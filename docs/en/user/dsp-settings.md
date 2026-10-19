**English | [Русский](../../ru/user/dsp-settings.md)**

# Practical DSP settings

DSP lightly processes synthesized speech for tonal balance, consistent volume
and safer peaks. It cannot recover authentic details absent from the source
and does not replace DeepFilterNet.

```text
DeepFilterNet → pitch/tempo → EQ → Compressor → Limiter
```

Use **With effects** in **Preview effects** to hear current unsaved settings.

## How to adjust

1. Choose a short representative phrase with sibilants, plosives and long vowels.
2. Compare **Original** and **With effects**.
3. Enable one block at a time.
4. Compare diction, word endings and pauses as well as loudness.
5. Save after comparing several voices.

Do not raise EQ, compressor makeup and device volume together: louder can
easily be mistaken for better.

## EQ

EQ changes frequency balance, not duration or pitch.

### Low Cut

Removes rumble, excessive bass and vibration:

| Situation | Frequency |
|---|---:|
| Clean TTS without rumble | Off |
| Slightly boomy | 60–80 Hz |
| Strong muddiness or hum | 90–120 Hz |

A thin, dry voice means the cutoff is too high. The slope field describes the
desired roll-off, but the current DSP uses a fixed gentle second-order low-cut.
Do not use slope as the main adjustment; it is reserved for future finer control.

### Parametric bands

Each of three bands has frequency (center), gain in dB and Q (bandwidth).

| Region | Symptom/purpose | Starting adjustment |
|---|---|---|
| 150–300 Hz | Mud/boom | `-1…-3 dB`, Q `0.7–1.2` |
| 500–1000 Hz | Nasal/boxy sound | `-1…-2 dB`, Q `1.0–1.5` |
| 2–4 kHz | Consonant clarity | `+1…+2 dB`, Q `0.7–1.2` |
| 5–8 kHz | Harshness/sibilance | `-1…-2 dB`, Q `1.5–3` |
| 8–12 kHz | Air/brightness | Carefully try `+0.5…+1.5 dB` |

Cut a problem region before boosting. Large boosts cause harshness/clipping.
A 16 kHz source has limited content above about 8 kHz; a high shelf cannot
authentically restore it. Low Q is broad/natural, high Q narrow/surgical;
`0.7–1.5` is usually enough for speech.

### High Shelf

Changes frequencies above the cutoff. Try `+0.5…+1.5 dB` around 8–10 kHz for
brightness, or `-1…-2 dB` around 6–8 kHz for harshness. If sibilants become
whistly, lower the shelf first, then check 5–8 kHz with a parametric band.

## Compressor

Reduces the difference between quiet and loud parts. Too much raises noise
and removes natural dynamics.

| Parameter | Purpose | Starting point |
|---|---|---|
| Threshold | Level where compression begins | `-18 dB` |
| Ratio | Compression strength | `2:1` |
| Attack | Speed of catching peaks | `5–10 ms` |
| Release | Speed of letting go | `80–150 ms` |
| Knee | Softness of compression onset | `4–8 dB` |
| Makeup | Compensate lost loudness | `0…+2 dB` |

- Uneven volume: threshold `-20…-16 dB`, ratio `2:1`.
- Harsh consonants: increase attack to `10–20 ms`.
- Audible breathing/pumping: increase release.
- Flat, squeezed sound: raise threshold, reduce ratio or disable makeup.
- Merely louder: reset makeup to `0 dB` and compare at equal loudness.

TTS rarely needs a ratio above `3:1` or makeup above `+3 dB`.

## Limiter

The final safety block prevents peaks after EQ/makeup, rather than shaping the
voice. Start with **Ceiling** `-1 dB`, **Release** `50–100 ms`. If it reduces
the signal heavily all the time, reduce earlier boosts first.
The current implementation uses a peak envelope, smooth release and hard
ceiling without lookahead. Very fast single peaks can therefore clip harder
than with a studio lookahead limiter.

## Starting recipes

### Natural voice

```text
EQ: enable only a parametric band, -1 dB at 200–300 Hz
Compressor: threshold -18 dB, ratio 2:1, attack 8 ms, release 120 ms, makeup 0 dB
Limiter: ceiling -1 dB, release 70 ms
```

### Clearer voice

```text
EQ: +1 dB around 3 kHz; optionally +0.5 dB high shelf around 9 kHz
Compressor: threshold -18 dB, ratio 2:1, makeup 0…+1 dB
Limiter: ceiling -1 dB
```

### Dark or muffled voice

```text
EQ: +1…+1.5 dB around 3 kHz and +0.5…+1 dB high shelf
Compressor: gentle, ratio 2:1
Limiter: safety only, not a loudness maximizer
```

### Harsh voice

```text
EQ: -1…-2 dB at 5–8 kHz; high shelf off
Compressor: attack 10–20 ms, ratio about 2:1
Limiter: ceiling -1 dB
```

## Symptoms and quick fixes

| Symptom | Check |
|---|---|
| Thin voice | Lower or bypass Low Cut |
| Harshness | Reduce 3–8 kHz or high shelf |
| Less intelligible words | Reduce denoising, reset boosts to 0 dB, increase attack |
| Pumping volume | Reduce ratio, increase release |
| Clicks/hard peaks | Reduce makeup/EQ boosts; keep limiter enabled |
| Little audible difference | Normal for clean TTS; do not boost everything |

Small `±1…2 dB` adjustments are preferable to aggressive chains. For an already
clean voice, address a specific issue rather than enabling every block.
