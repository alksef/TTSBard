# Pitch, Speed, Volume в TTS-Voice-Wizard

## Ответ на главный вопрос

**ДА, для большинства TTS сервисов это ПОСТПРОЦЕССИНГ аудио.**

Аудио сначала генерируется TTS моделью, а затем обрабатывается через SoundTouch библиотеку для изменения pitch и speed, и через NAudio для изменения volume.

Исключение: Azure TTS и Amazon Polly могут принимать эти параметры через SSML теги напрямую в API.

---

## Обзор архитектуры

TTS-Voice-Wizard использует **гибридную систему**:

| Тип | TTS сервисы | Метод |
|-----|-------------|-------|
| **Постпроцессинг** | ElevenLabs, TikTok, Google, OpenAI, System Speech | SoundTouch + NAudio |
| **SSML параметры** | Azure TTS, Amazon Polly | SSML prosody теги + опциональный постпроцессинг |
| **API параметры** | VoiceWizardPro | URL параметры (&speed=, &pitch=, &volume=) |

---

## Пайплайн обработки аудио

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│  TTS Service    │ -> │  Volume         │ -> │  Speed          │ -> │  Pitch          │
│  (MP3/WAV)      │    │  (NAudio)       │    │  (SoundTouch)   │    │  (SoundTouch)   │
└─────────────────┘    └─────────────────┘    └─────────────────┘    └─────────────────┘
                              ↓                      ↓                      ↓
                         WaveChannel32      VarispeedSampleProvider  VarispeedSampleProvider
                         (amplitude)        (UseTempo=true)          (UseTempo=false)
```

### Код (AudioDevices.cs)

```csharp
// 1. Чтение аудио из TTS сервиса
Stream audioStream = TTS_engine.GenerateSpeech();
WaveStream audioReader = new Mp3FileReader(memoryStream);

// 2. VOLUME - простое умножение амплитуды
var volumeFloat = volume * 0.1f; // 0-10 → 0.0-1.0
var wave32 = new WaveChannel32(audioReader, volumeFloat, 0f);

// 3. SPEED - изменение скорости БЕЗ изменения pitch (Tempo режим)
VarispeedSampleProvider speedControl = new VarispeedSampleProvider(
    new WaveToSampleProvider(wave32), 100,
    new SoundTouchProfile(true, false)); // UseTempo=true
speedControl.PlaybackRate = rateFloat;

// 4. PITCH - изменение высоты БЕЗ изменения скорости (Rate режим)
VarispeedSampleProvider speedControl2 = new VarispeedSampleProvider(
    speedControl, 100,
    new SoundTouchProfile(false, false)); // UseTempo=false
speedControl2.PlaybackRate = pitchFloat;

// 5. Воспроизведение
var outputDevice = new WaveOut();
outputDevice.Init(speedControl2);
outputDevice.Play();
```

---

## Диапазоны значений

| Параметр | UI Trackbar | Конвертированный float | SSML процент |
|----------|-------------|------------------------|--------------|
| Volume   | 0 - 10      | 0.0 - 1.0              | -100% - 0%   |
| Pitch    | -10 - +10   | 0.9 - 1.1              | -10% - +10%  |
| Speed    | -10 - +10   | 0.9 - 1.1              | -10% - +10%  |

### Конвертация Pitch/Speed

```csharp
public static float ConvertPitchToFloat(int pitchValue)
{
    // pitchValue: -10 до +10
    float normalizedValue = (pitchValue + 100) * 1.0f;
    float floatValue = normalizedValue / 100.0f; // Результат: 0.9 - 1.1

    floatValue = Math.Clamp(floatValue, 0.0f, 2.0f);
    if(floatValue == 0f) floatValue = 0.01f;

    return floatValue;
}
```

Примеры:
- `-10` → `0.9` (90% от нормы, медленнее/ниже)
- `0` → `1.0` (норма)
- `+10` → `1.1` (110% от нормы, быстрее/выше)

---

## Как работает каждый параметр

### Volume (Громкость)

**Метод:** NAudio `WaveChannel32`

**Принцип:** Простое умножение амплитуды аудио сигнала.

```csharp
var volumeFloat = volume * 0.1f; // 0-10 → 0.0-1.0
var wave32 = new WaveChannel32(audioReader, volumeFloat, 0f);
```

**Особенности:**
- Применяется первым в пайплайне
- Работает с raw аудио данными
- Не влияет на длительность или pitch

**SSML (Azure/Amazon):**
```csharp
// Azure
var volumePercent = (int)Math.Floor((volume * 0.1f - 1) * 100);
string volumeString = "<prosody volume=\"" + volumePercent + "%\">";

// Amazon (в децибелах)
var volumePercent = (int)Math.Floor((volume * 0.1f - 1) * 10);
string volumeString = "<prosody volume=\"" + volumePercent + "dB\">";
```

---

### Speed (Скорость воспроизведения)

**Метод:** SoundTouch `SetTempo()`

**Принцип:** Изменение скорости воспроизведения БЕЗ изменения высоты звука.

```csharp
VarispeedSampleProvider speedControl = new VarispeedSampleProvider(
    new WaveToSampleProvider(wave32), 100,
    new SoundTouchProfile(true, false)); // UseTempo=true
speedControl.PlaybackRate = rateFloat;
```

**Ключевая особенность:**
- `UseTempo=true` - использует алгоритм tempo preservation
- Сохраняет pitch, меняет только скорость
- Основан на time-stretching алгоритмах

**SSML (Azure/Amazon):**
```csharp
// Azure
var ratePercent = rate; // -10 до +10
string rateString = "<prosody rate=\"" + ratePercent + "%\">";

// Amazon
string rateString = "<prosody rate=\"";
if (ratePercent > 0) rateString += "+";
rateString += ratePercent + "%\">";
```

---

### Pitch (Высота звука)

**Метод:** SoundTouch `SetRate()`

**Принцип:** Изменение частоты дискретизации БЕЗ изменения длительности.

```csharp
VarispeedSampleProvider speedControl2 = new VarispeedSampleProvider(
    speedControl, 100,
    new SoundTouchProfile(false, false)); // UseTempo=false
speedControl2.PlaybackRate = pitchFloat;
```

**Ключевая особенность:**
- `UseTempo=false` - использует `SetRate()` вместо `SetTempo()`
- Меняет pitch И скорость одновременно
- Применяется ПОСЛЕ speed для компенсации изменения скорости

**Почему два этапа?**
```
Audio → Speed (Tempo) → Pitch (Rate)
```

Pitch применяется после Speed, чтобы:
1. Сначала изменить скорость без изменения pitch (Tempo)
2. Затем изменить pitch без влияния на скорость (Rate)

**SSML (Azure/Amazon):**
```csharp
var pitchPercent = pitch; // -10 до +10
string pitchString = "<prosody pitch=\"" + pitchPercent + "%\">";
```

---

## Используемые библиотеки

### SoundTouch

**Проект:** [SoundTouch](https://www.surina.net/soundtouch/) - Open Source Audio Processing Library

**Файлы в проекте:**
- `OSCVRCWiz\Resources\Audio\SoundTouch\SoundTouch.cs`
- `OSCVRCWiz\Resources\Audio\SoundTouch\VarispeedSampleProvider.cs`
- `OSCVRCWiz\Resources\Audio\SoundTouch\SoundTouchProfile.cs`

**Ключевые методы:**
```csharp
SetPitchOctaves(float pitchOctaves)  // Изменение pitch в октавах
SetRate(float newRate)               // Изменение скорости С изменением pitch
SetTempo(float newTempo)             // Изменение скорости БЕЗ изменения pitch
SetSampleRate(int sampleRate)         // Установка частоты дискретизации
SetChannels(int channels)             // Количество каналов
```

**Особенности:**
- P/Invoke интероп к нативным `SoundTouch.dll` / `SoundTouch_x64.dll`
- Основан на проекте NAudio Varispeed-Sample
- Использует WSOLA (Waveform Similarity-based Overlap-Add) алгоритм

### NAudio

**Назначение:** Работа с аудио форматами и воспроизведением

**Ключевые классы:**
- `WaveFileReader`, `Mp3FileReader` - чтение аудио
- `WaveChannel32` - громкость (амплитуда)
- `WaveOut` - воспроизведение
- `ISampleProvider` - интерфейс для обработки аудио потока

---

## Ключевые файлы проекта

### Аудио обработка

**Файл:** `D:\Projects\TTS-Voice-Wizard\OSCVRCWiz\Resources\Audio\AudioDevices.cs`

- Основной метод: `PlayAudioStream()`
- Конвертация параметров (строки 262-281)
- Создание пайплайна обработки (строки 369-376)

### TTS движки с SSML поддержкой

**Azure TTS:** `OSCVRCWiz\Services\Speech\TextToSpeech\TTSEngines\AzureTTS.cs`
- SSML генерация (строки 348-354)

**Amazon Polly:** `OSCVRCWiz\Services\Speech\TextToSpeech\TTSEngines\AmazonPollyTTS.cs`
- SSML генерация (строки 96-109)

### Очередь сообщений

**Файл:** `OSCVRCWiz\Services\Speech\TextToSpeech\TTSMessageQueue.cs`

```csharp
public struct TTSMessage
{
    public int Pitch;   // -10 to +10
    public int Volume;  // 0 to 10
    public int Speed;   // -10 to +10
}
```

---

## Важные технические детали

### Двойная обработка для Pitch

Pitch применяется **после** Speed для независимого контроля параметров:

```
Audio → Volume → Speed (Tempo, сохраняет pitch) → Pitch (Rate, меняет pitch+скорость)
```

Без этого разделения:
- Изменение speed меняло бы pitch
- Изменение pitch меняло бы скорость

### Флаг applyAudioEditing

В `PlayAudioStream()` есть параметр `applyAudioEditing`:
- `true` - применять SoundTouch обработку (pitch/speed)
- `false` - только громкость

Для Azure и Amazon Polly часто `false`, так как они используют SSML.

### Значения по умолчанию

| Параметр | Trackbar | Float | Значение |
|----------|----------|-------|----------|
| Volume   | 10       | 1.0   | 100%     |
| Pitch    | 5        | 1.0   | Нет изменения |
| Speed    | 5        | 1.0   | Нет изменения |

---

## Резюме для реализации

Если вы хотите реализовать похожую функциональность:

1. **Для постпроцессингаpitch/speed:**
   - Используйте **Rubberband** (Rust) или **SoundTouch** (C++)
   - Применяйте: Volume → Speed (tempo) → Pitch (rate)

2. **Для TTS с SSML поддержкой:**
   - Передавайте параметры через `<prosody>` теги
   - Пример: `<prosody rate="+10%" pitch="-5%" volume="80%">`

3. **Для простого volume:**
   - Умножайте амплитуду сэмпла на коэффициент 0.0-1.0

4. **Диапазоны:**
   - Volume: 0-100% (0.0-1.0)
   - Pitch: ±1 октава или ±10-20%
   - Speed: 0.5x-2.0x или ±50%

---

## Дополнительные ресурсы

- [SoundTouch Library](https://www.surina.net/soundtouch/)
- [NAudio Documentation](https://github.com/naudio/NAudio)
- [SSML prosody Tag](https://www.w3.org/TR/speech-synthesis11/#S3.2.4)
- [Rubberband Library (Rust)](https://github.com/vpeal/rubberband)
