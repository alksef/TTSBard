# Deep Research: lip-sync TTSBard → VTube Studio

**Дата:** 2026-07-18  
**Статус:** углублённое исследование / product and technical validation  
**Связано:** [ROADMAP-042](../roadmap/completed/042-vtube-studio-typing-ui.md), [ROADMAP-045](../roadmap/completed/045-vtube-studio-typing-output-modes.md), [tts_pipeline.rs](../../src-tauri/src/commands/tts_pipeline.rs), [playback.rs](../../src-tauri/src/playback.rs)

---

## 1. Краткий вывод

Lip-sync для TTSBard актуален, но его ценность нужно формулировать точно.

### Что уже умеет VTube Studio

VTube Studio имеет встроенный **Advanced Lipsync**. Он принимает звук с выбранного
микрофонного входа, анализирует его через основанный на uLipSync алгоритм и
формирует:

- `VoiceA`;
- `VoiceI`;
- `VoiceU`;
- `VoiceE`;
- `VoiceO`;
- `VoiceSilence`;
- `VoiceVolume`;
- `VoiceVolumePlusMouthOpen`;
- `VoiceFrequency`;
- `VoiceFrequencyPlusMouthSmile`.

При наличии модели с vowel blendshapes это качественнее простого управления
`MouthOpen` по громкости.

### Что уже умеет TTSBard

TTSBard уже поддерживает одновременный вывод:

- в динамики/наушники;
- в виртуальный микрофон VB-Cable/VoiceMeeter.

Следовательно, пользователь уже сейчас может выбрать virtual cable как вход
VTube Studio и использовать встроенный Advanced Lipsync. Для стримера, которому
виртуальный микрофон всё равно нужен для Discord, игры или другой программы,
отдельный API lip-sync частично дублирует существующий путь.

### Где остаётся ценность прямого API

Прямой VTS API lip-sync полезен, когда:

- пользователь не хочет устанавливать и настраивать virtual cable;
- TTS должен двигать рот, но не должен появляться как системный microphone input;
- требуется изолировать несколько TTS-источников;
- lip-sync является частью более широкой интеграции с `Typing`, `Thinking`,
  реакциями и VTS hotkey;
- нужен полный контроль над кривой рта, паузами и моментом сброса.

### Итоговая продуктовая оценка

- **Lip-sync как отдельная «уникальная» функция:** средняя ценность, потому что
  VTS + virtual cable уже закрывают задачу и могут дать более точные vowel shapes.
- **Lip-sync без кабеля:** заметная ценность как упрощение настройки.
- **Lip-sync внутри «реактивного аватара»:** высокая ценность, потому что то же
  соединение даёт набор текста, thinking-state, эмоции, hotkey и Items.

Рекомендуемое позиционирование:

> «Реактивный VTube Studio-аватар одной кнопкой: показывает набор текста,
> реагирует и синхронно говорит без обязательного виртуального аудиокабеля».

Не следует обещать, что простой API lip-sync будет качественнее встроенного
Advanced Lipsync VTube Studio.

---

## 2. Подтверждение актуальности

### 2.1. Существующие продукты

Категория уже коммерчески существует:

- VTS P.O.G. продаётся как Windows-плагин, связывающий TTS с VTube Studio,
  Live2D/PNG/3D pets;
- Blerp предлагает TTS pets с lip-sync и реакциями;
- в официальном списке VTube Studio присутствуют плагины для desktop audio,
  TTS pets и sound-reactive поведения.

Наличие платных решений не доказывает массовый спрос, но подтверждает, что
сценарий реален и за упрощение TTS → avatar пользователи готовы платить.

### 2.2. Пользовательская проблема

В обсуждениях 2024–2026 годов продолжают встречаться вопросы:

- как заставить модель двигать ртом только от микрофона/аудио;
- как подать prerecorded/TTS audio вместо физического микрофона;
- почему Advanced Lipsync не реагирует или неправильно смешивается с camera
  tracking;
- как настроить virtual cable без feedback/echo;
- как разделить несколько говорящих персонажей.

Это слабее статистического исследования рынка, но показывает устойчивую
настройочную проблему.

### 2.3. Ограничение рыночного аргумента

Целевая аудитория TTSBard уже чаще среднего использует virtual mic, потому что
синтезированный голос нужно отправлять в Discord, игру или трансляцию. Для неё
дополнительная настройка VTS на тот же cable может быть небольшой.

Поэтому решение о приоритете нельзя принимать только по интернет-обсуждениям.
Нужен короткий тест с реальными пользователями TTSBard:

1. Есть ли у них VTube Studio?
2. Есть ли у модели только `MouthOpen` или vowel blendshapes?
3. Используют ли они virtual mic?
4. Настроен ли уже audio lipsync?
5. Что ценнее: отсутствие кабеля, typing indicator или реакции?

---

## 3. Эталонный существующий путь: VTS Advanced Lipsync

### 3.1. Архитектура

```text
TTSBard final PCM
  -> virtual output device
  -> virtual cable input
  -> VTube Studio Advanced Lipsync
  -> uLipSync/MFCC vowel analysis
  -> VoiceA/I/U/E/O + VoiceSilence + VoiceVolume
  -> Live2D parameter mappings
```

### 3.2. Как работает Advanced Lipsync

VTube Studio указывает, что Advanced Lipsync основан на
[uLipSync](https://github.com/hecomi/uLipSync). uLipSync:

1. разбивает аудио на окна;
2. вычисляет MFCC — компактное описание спектральной формы речевого тракта;
3. сравнивает MFCC текущего окна с откалиброванными профилями фонем/гласных;
4. выдаёт веса похожести для `A/I/U/E/O` и silence;
5. сочетает их с громкостью и smoothing.

Профиль желательно калибровать под конкретный голос. В случае TTS логично
калибровать VTS на синтезированные выбранным voice звуки `A/I/U/E/O`, а не на
физический голос пользователя.

### 3.3. Требования к модели

Для полного Advanced Lipsync модель должна иметь отдельные Live2D blendshape
параметры под формы рта. Официальный пример использует:

- `ParamA`;
- `ParamI`;
- `ParamU`;
- `ParamE`;
- `ParamO`;
- `ParamSilence`.

Если модель имеет только обычные `ParamMouthOpen` и `ParamMouthForm`, пользователь
получает главным образом открытие рта по громкости/частотной характеристике, а не
полноценные vowel shapes.

### 3.4. Сильные стороны

- уже встроено в VTS;
- выдаёт пять vowel weights;
- поддерживает calibration;
- умеет смешиваться с camera/phone tracking через модельный setup;
- не требует от TTSBard собственной реализации MFCC;
- анализирует именно тот сигнал, который реально пришёл на virtual device.

### 3.5. Слабые стороны

- нужен virtual audio device;
- настройка выполняется в нескольких приложениях;
- возможны неправильный routing, echo и выбор не того endpoint;
- задержка зависит от virtual device, VTS input buffering и VTS rendering;
- две независимые output-сессии TTSBard могут иметь разную device latency;
- Advanced Lipsync требует подходящего рига и настройки;
- при активной речи vowel blendshapes могут почти полностью вытеснять обычный
  camera mouth tracking.

---

## 4. Варианты прямого lip-sync через API

### 4.1. Вариант A — бинарный рот

```text
Playback start  -> MouthOpen = 1
Playback finish -> MouthOpen = 0
```

Преимущество — минимальная сложность.

Качество слишком низкое: рот остаётся открытым во время пауз и не передаёт ритм
речи. Это допустимо только как diagnostic test или аварийный fallback.

**Вердикт:** не использовать как пользовательский режим.

---

### 4.2. Вариант B — amplitude/envelope

```text
final AudioPcm
  -> mono downmix
  -> windowed RMS
  -> dB/curve normalization
  -> noise gate
  -> asymmetric smoothing
  -> timeline MouthOpen(t)
  -> InjectParameterDataRequest
```

Это наиболее практичный прямой режим для MVP.

#### Плюсы

- не нужен virtual cable;
- работает с любым TTS-провайдером;
- почти не требует CPU;
- точно повторяет паузы и динамику уже обработанного звука;
- использует стандартный `MouthOpen`, доступный почти у любой модели;
- легко тестируется и настраивается.

#### Минусы

- не различает формы гласных и согласных;
- не распознаёт закрытые `P/B/M`, если звук остаётся громким;
- при сильно нормализованном TTS рот может двигаться однообразно;
- конкурирует с camera mouth tracking и другими API-плагинами;
- требуется синхронизировать timeline с фактическим audio device playback.

#### Оценка

Это хороший «zero-setup lip-sync», но не замена VTS Advanced Lipsync по качеству.

---

### 4.3. Вариант C — MFCC vowel weights

```text
final AudioPcm
  -> overlapping windows
  -> MFCC
  -> compare with voice profile
  -> normalized A/I/U/E/O/Silence weights
  -> precomputed timeline
  -> inject VoiceA/I/U/E/O/VoiceSilence
```

Это прямой аналог класса алгоритмов, которые использует VTS Advanced Lipsync.

#### Подтверждённые реализации

- **uLipSync** — MIT, Unity/C#, runtime и pre-bake, voice calibration;
- **pylipsync 0.2.1** — MIT, Python beta, порт идеи uLipSync, использует
  `librosa`, `numpy`, `scipy`, готовые MFCC templates;
- **mfcc 0.1.0** — старый, но лёгкий pure-Rust-compatible crate с `rustfft`.

`pylipsync` по умолчанию анализирует центрированные окна около `64 ms` и может
выдавать кадры с частотой `60 fps`. Эти значения являются настройками конкретной
реализации, а не обязательным стандартом.

#### Почему TTSBard имеет преимущество

TTSBard получает целый `AudioPcm` до playback. Анализ можно выполнить заранее:

- доступен look-ahead;
- можно центрировать окна без runtime latency;
- timeline детерминирован;
- обработка не зависит от WebView или audio callback;
- можно сдвигать визуальный timeline относительно аудио;
- можно кэшировать результат вместе с phrase audio.

#### Главная проблема — calibration

MFCC-профиль зависит от тембра и обработки голоса. Профиль может измениться при:

- выборе другого voice;
- смене TTS-провайдера;
- pitch/formant effects;
- voice conversion;
- сильном EQ или enhancement.

Анализ должен выполняться по **финальному PCM после эффектов**, а профиль — по
голосу с теми же значимыми преобразованиями.

Возможная будущая функция:

1. TTSBard генерирует тестовые продолжительные `A/I/U/E/O`;
2. применяет текущие effects;
3. строит MFCC profile;
4. сохраняет profile по provider + voice + relevant effects fingerprint;
5. позволяет пользователю проверить распознавание.

#### Ограничения модели

Прямые vowel weights полезны только модели, имеющей vowel blendshapes. Для
обычной модели они не дают автоматического улучшения.

#### Вердикт

Перспективный P2/experimental режим. До MVP нужен отдельный prototype и
визуальное сравнение со встроенным VTS Advanced Lipsync.

---

### 4.4. Вариант D — Rhubarb/forced alignment

[Rhubarb Lip Sync](https://github.com/DanielSWolf/rhubarb-lip-sync) анализирует
готовую запись и возвращает timestamped mouth cues.

Факты:

- текущая публичная версия 1.14.0 опубликована 2025-04-03;
- Windows/macOS/Linux;
- принимает WAV и Ogg Vorbis;
- возвращает TSV/XML/JSON;
- использует шесть основных и до трёх дополнительных mouth shapes;
- PocketSphinx recognizer предназначен для английского;
- phonetic recognizer language-independent, но менее точен;
- dialog text может улучшать результат;
- MIT license и permissive third-party dependencies.

Владелец Rhubarb отдельно указывает, что 1.x не проектировался для real-time.
В обсуждении проекта встречается наблюдение, что анализ может занимать время,
сопоставимое с длительностью аудио. Это не гарантированный benchmark, но достаточная
причина не ставить Rhubarb в критический live path без измерений.

Дополнительная проблема: Rhubarb выдаёт формы `A–F/G/H/X` в системе для
классической 2D-анимации, а стандартный advanced rig VTS ожидает веса
`A/I/U/E/O`. Преобразование возможно только приблизительно и теряет часть
consonant shapes.

**Вердикт:** подходит для экспорта, prerecorded clips или эксперимента; не
рекомендуется для первой live-версии TTSBard.

---

### 4.5. Вариант E — provider-native timestamps

Текущая TTS-абстракция TTSBard возвращает только:

```rust
async fn synthesize(&self, text: &str) -> Result<Vec<u8>, String>;
```

То есть единых timing metadata сейчас нет.

#### Fish Audio

Fish Audio имеет отдельный endpoint
`/v1/tts/stream/with-timestamp`, который возвращает audio chunks и timestamp
alignment. Показанный официальный ответ содержит сегменты текста/слов с `start`
и `end`, а не phoneme/viseme weights.

Это полезно для:

- субтитров;
- подсветки текущего слова;
- реакций на части фразы;
- приблизительного text-driven mouth motion.

Для точного vowel lip-sync одних word timestamps недостаточно.

Текущий `FishTts` в TTSBard использует обычный `/v1/tts` и возвращает audio bytes.

#### Остальные провайдеры

Нельзя рассчитывать на общий provider API с phoneme timestamps. Даже если
конкретный движок внутри использует phonемы и duration tensors, публичный
контракт может их не возвращать.

**Вердикт:** timing metadata следует проектировать как необязательное расширение
TTS result, но lip-sync MVP не должен от него зависеть.

---

## 5. Сравнение режимов

| Режим | Кабель | Качество формы рта | Совместимость моделей | Live latency | Сложность TTSBard |
|---|---:|---|---|---|---|
| VTS Advanced Lipsync | Да | Высокое при vowel rig | Требует vowel setup для полного эффекта | Зависит от audio routing | Низкая |
| API binary | Нет | Очень низкое | Почти любая | Низкая | Очень низкая |
| API RMS/envelope | Нет | Среднее: ритм без артикуляции | Почти любая | Низкая, но нужен sync offset | Низкая |
| API MFCC vowels | Нет | Потенциально высокое | Нужен vowel rig | Низкая при precompute | Средняя/высокая |
| Rhubarb timeline | Нет | Высокое для подходящих 2D shapes | Плохо совпадает со стандартным VTS rig | Analysis delay | Средняя |
| Provider timings | Нет | Зависит от granularity | Зависит от mapping | Низкая | Провайдер-зависимая |

---

## 6. Правильный amplitude/envelope pipeline

Раннее исследование предлагало считать RMS внутри `rodio::Source` decorator.
Для текущей архитектуры TTSBard это уже не лучший вариант.

### 6.1. Почему лучше precompute

До enqueue TTSBard уже имеет финальный `AudioPcm` после:

1. decode;
2. DeepFilterNet;
3. tempo/pitch/formant processing;
4. DSP;
5. boundary cleanup.

Следовательно, можно один раз вычислить timeline до отправки фразы в
`PlaybackManager`.

Преимущества:

- нет атомиков и DSP внутри audio callback;
- одинаковый результат при repeat/replay;
- timeline можно тестировать обычными unit tests;
- возможно look-ahead сглаживание;
- легче реализовать pause/resume/seek-like state;
- тот же контейнер позже может хранить vowel weights.

### 6.2. Downmix

`AudioPcm.samples` interleaved. Для каждого frame нужно получить mono sample.

Варианты:

- среднее каналов;
- RMS каналов с сохранением знака для последующего окна;
- maximum absolute channel.

Для обычного mono/stereo TTS достаточно среднего. Нельзя считать RMS напрямую по
interleaved данным с разной энергией каналов, не определив ожидаемую семантику.

### 6.3. Окна

Практическая стартовая гипотеза:

- RMS window: `20–30 ms`;
- output hop: `~33 ms` (`30 Hz`);
- затем сравнить `20`, `30` и `60 Hz`.

Это не лимит VTS API. Официальная документация не задаёт рекомендуемую максимальную
частоту для `InjectParameterDataRequest`. Частоту нужно выбрать по prototype:

- визуальная плавность;
- FPS модели;
- CPU/JSON overhead;
- количество ошибок/очередь ответов;
- отсутствие влияния на VTS.

### 6.4. Нормализация

Линейный `RMS * multiplier` плохо переносится между голосами и эффектами.
Предпочтительна шкала dBFS и soft-knee:

```text
rms
  -> 20 * log10(max(rms, epsilon))
  -> gate/floor
  -> normalize [floor_db, open_db] to [0, 1]
  -> curve/gamma
```

Значения порогов нельзя фиксировать без тестового корпуса. Возможны:

- ручные `floor/open`;
- адаптация по percentiles текущей фразы;
- preset по provider/voice;
- hybrid: безопасные границы + percentile adaptation.

Для коротких фраз percentile adaptation может быть нестабильной, поэтому нужен
минимальный диапазон и fallback.

### 6.5. Attack/release

Открытие и закрытие рта должны сглаживаться раздельно:

- более быстрый attack;
- более медленный release;
- немедленный или ускоренный reset при stop.

Стартовая экспериментальная зона, а не утверждённый default:

- attack `15–40 ms`;
- release `50–120 ms`.

Слишком быстрый release создаёт «дребезг», слишком медленный оставляет рот
открытым на коротких паузах.

### 6.6. Plosives и закрытый рот

Amplitude не понимает, что `P/B/M` произносятся с закрытыми губами. Улучшения
без полноценного phoneme recognizer:

- короткие dips из text-informed heuristics — рискованно без alignment;
- spectral features/MFCC;
- provider timing + G2P;
- не исправлять в MVP и честно назвать режим volume-based.

Последний вариант наиболее безопасен.

---

## 7. Синхронизация с playback

Это главный риск прямого API, более важный, чем скорость расчёта RMS.

### 7.1. Текущее поведение

`PlaybackManager`:

1. создаёт отдельный `OutputStream + Sink` для speaker;
2. создаёт отдельный `OutputStream + Sink` для virtual mic;
3. добавляет одинаковый `AudioPcm` в оба sink;
4. после успешного создания хотя бы одного sink отправляет `PlaybackStarted`.

Фактический первый слышимый sample появляется позже `PlaybackStarted` из-за:

- очереди rodio;
- CPAL/device buffer;
- драйвера;
- конкретного устройства;
- virtual cable;
- OBS/audio capture path.

Speaker и virtual mic могут иметь разные задержки.

### 7.2. Clock

Timeline должен работать от монотонного clock, а не от количества циклов
`sleep(30 ms)`. Накопление sleep drift приведёт к рассинхронизации длинных фраз.

```text
elapsed = monotonic_now - playback_epoch
frame = timeline.sample(elapsed + configured_offset)
```

Pause:

- сохранить elapsed;
- отправить рот в `0`;
- при resume создать новый epoch с учётом сохранённой позиции.

Stop/finish:

- отправить финальный `0`;
- отменить timeline task;
- прекратить владеть параметром.

### 7.3. Offset

Нужна настройка `lip_sync_offset_ms`, потому что приложение не знает реальную
device-to-visible latency.

Предлагаемый диапазон UI:

```text
-250 ms ... +250 ms
```

Знак следует объяснить в интерфейсе через «рот раньше / рот позже», а не только
через математический плюс/минус.

### 7.4. Как измерять

Практический тест:

1. PCM с короткими импульсами/слогами;
2. модель с контрастным `MouthOpen`;
3. запись окна VTS и аудио в OBS;
4. frame-by-frame сравнение момента audio onset и движения рта;
5. повтор для speaker-only и virtual-mic/OBS path.

Без такого теста нельзя обещать «нулевую задержку».

---

## 8. Ограничения VTS Plugin API

Подтверждённые официальной документацией свойства:

- используется `InjectParameterDataRequest`;
- можно передавать standard и custom parameters;
- при `mode: set` API временно перехватывает параметр;
- параметр нужно повторно отправлять хотя бы раз в секунду;
- после потери управления VTS возвращается к предыдущему source/default;
- `weight` от `0` до `1` смешивает API value с face tracking;
- только один API plugin может писать параметр в `set` mode;
- `add` допускает несколько плагинов, но не использует `weight`;
- smoothing можно настроить в VTS parameter mapping;
- custom parameters ограничены: 100 на plugin, 300 глобально.

### Практические следствия

1. В конце речи отправить `MouthOpen = 0`.
2. Затем прекратить обновления, чтобы VTS вернул camera tracking.
3. Не использовать `faceFound: true` без отдельной причины: это может менять
   tracking-lost поведение пользователя.
4. Показывать понятную ошибку, если параметр занят другим плагином.
5. Не ждать response перед каждой следующей lip-sync frame.
6. Reader loop должен постоянно принимать ответы/ошибки, writer loop — отправлять
   актуальный frame.
7. Устаревшие frames нужно заменять последним значением, а не накапливать в
   неограниченной очереди.
8. Несколько значений `A/I/U/E/O/Silence/Volume` отправлять одним request.

Официальный API не подтверждает найденные в сторонних ответах утверждения о
фиксированном лимите `30–60 Hz` или гарантированной WebSocket latency
`15–30 ms`. Эти показатели должны измеряться.

---

## 9. Конфликт с camera tracking и Advanced Lipsync

Нельзя одновременно бездумно включать:

- VTS Advanced Lipsync от virtual mic;
- API injection тех же `Voice*` или `MouthOpen`;
- другой lip-sync plugin.

Возможны:

- ownership error между API plugins;
- перезапись tracking source;
- двойное smoothing;
- непредсказуемое смешивание;
- рот, который не возвращается к camera tracking.

UI должен предлагать взаимоисключающие режимы:

1. `Off`;
2. `VTS Advanced Lipsync via virtual mic`;
3. `Direct API — MouthOpen`;
4. позднее `Direct API — A/I/U/E/O (Experimental)`.

Для первого режима TTSBard не инжектирует mouth parameters, а показывает
инструкцию настройки VTS и тестовый звук.

---

## 10. Рекомендуемый продуктовый дизайн

### 10.1. Два режима первой версии

#### Режим «VTS Advanced — лучшее качество»

Показывать, когда в TTSBard выбран virtual mic:

- выбрать этот же cable input в VTS;
- включить Advanced Lipsync;
- провести calibration под TTS voice;
- использовать VTS auto-setup для vowel-rigged model;
- запустить тест `A/I/U/E/O`.

TTSBard здесь даёт setup wizard и диагностику, а анализ выполняет VTS.

#### Режим «Direct API — без кабеля»

- WebSocket authentication;
- precomputed amplitude envelope;
- injection `MouthOpen`;
- gain/gate/curve/attack/release;
- offset calibration;
- reset на pause/stop/finish/disconnect;
- предупреждение, что режим передаёт громкость, а не vowel shapes.

### 10.2. Общая ценность соединения

Независимо от выбранного mouth mode VTS API остаётся полезен для:

- `Typing`;
- `Thinking`;
- `Speaking`;
- reaction hotkeys;
- error/waiting state;
- Items.

Поэтому VTS integration не следует архитектурно сводить только к lip-sync.

### 10.3. Experimental после MVP

Pure-Rust MFCC prototype:

- precompute `A/I/U/E/O/Silence`;
- generic male/female/TTS profiles;
- per-voice calibration;
- profile fingerprint;
- сравнение со встроенным VTS Advanced Lipsync.

Не добавлять Python runtime в production TTSBard ради `pylipsync`: его код полезен
как reference, но `librosa + numpy + scipy` противоречат цели компактного
standalone Windows-приложения.

---

## 11. Предлагаемая структура данных

Общий результат анализа фразы:

```rust
struct LipSyncFrame {
    time_ms: u32,
    mouth_open: f32,
    vowels: Option<VowelWeights>,
}

struct VowelWeights {
    a: f32,
    i: f32,
    u: f32,
    e: f32,
    o: f32,
    silence: f32,
}

struct LipSyncTimeline {
    frame_interval_ms: u32,
    frames: Vec<LipSyncFrame>,
}
```

Это иллюстрация, не implementation contract.

Timeline логично хранить рядом с `QueuedPhrase` и cached phrase, чтобы repeat не
пересчитывал анализ.

Для MFCC weights нужны дополнительные инварианты:

- все значения finite и в `0..1`;
- сумма vowel weights ограничена;
- silence взаимоисключается или корректно смешивается с vowels;
- резкие переходы сглажены;
- frame timestamps монотонны.

---

## 12. План prototype/валидации

### 12.1. Корпус

Минимум 20–30 фраз:

- русский и английский;
- короткие/длинные;
- быстрые и медленные;
- много пауз;
- `P/B/M`, шипящие, гласные;
- тихое окончание;
- разные эмоции/интонации.

Провайдеры:

- OpenAI;
- Fish Audio;
- Silero;
- Piper/local model, если доступен.

### 12.2. Режимы сравнения

1. VTS Advanced Lipsync через virtual mic;
2. API raw RMS;
3. API dB envelope + attack/release;
4. API MFCC vowels prototype;
5. camera/обычный baseline.

### 12.3. Модели

- простая модель только с `MouthOpen/MouthForm`;
- модель с `A/I/U/E/O/Silence`;
- по возможности одна типовая пользовательская модель.

### 12.4. Метрики

Технические:

- audio-to-mouth onset offset;
- end offset;
- количество ложных открытий на паузах;
- количество пропущенных коротких слогов;
- VTS FPS impact;
- CPU usage TTSBard/VTS;
- requests/sec;
- response/error backlog;
- reconnect/reset correctness.

UX:

- время первичной настройки;
- количество шагов;
- нужен ли внешний driver;
- субъективная естественность;
- предпочтение в слепом парном сравнении;
- понятность режима и ошибок.

Метрики для реалистичных talking-face video вроде LSE-C/LSE-D плохо соответствуют
стилизованной Live2D-модели и не нужны для первого решения. Важнее frame analysis
и пользовательское сравнение.

### 12.5. Decision gates

Direct API amplitude включать в продукт, если:

- настройка заметно проще virtual cable;
- паузы выглядят корректно;
- offset калибруется стабильно;
- нет влияния на playback и VTS FPS;
- stop/disconnect всегда закрывает рот.

MFCC mode продолжать, если:

- заметно превосходит amplitude в слепом сравнении;
- приближается к VTS Advanced Lipsync;
- calibration можно объяснить обычному пользователю;
- анализ не добавляет заметной задержки;
- pure-Rust distribution остаётся компактной.

Если amplitude не проходит сравнение, VTS plugin всё равно может остаться ради
Typing/Thinking/reactions, а lip-sync UI будет рекомендовать встроенный Advanced
Lipsync через virtual mic.

---

## 13. Уточнения к предыдущим исследованиям

### К раннему исследованию VTube Studio

Следует считать предварительными, а не подтверждёнными:

- обещание «без задержек»;
- фиксированную частоту `30–50 FPS` как готовое решение;
- предположение, что amplitude API лучше virtual cable во всех случаях;
- необходимость считать RMS именно внутри `rodio::Source`;
- приоритет procedural jaw flap как основного fallback.

Актуальное уточнение:

- virtual mic уже реализован в TTSBard;
- VTS Advanced Lipsync через него может дать более качественные vowel shapes;
- current final `AudioPcm` позволяет precompute timeline;
- API update rate и sync offset требуют измерения.

### К исследованию реактивного аватара

Главный вывод остаётся: интеграция ценнее как event-to-action layer. Lip-sync
следует представить одним из режимов, а не единственной причиной подключения.

---

## 14. Проверка Perplexity

Perplexity оказался полезен для поиска направлений:

- вывел на официальный VTS Lipsync guide;
- указал на uLipSync;
- помог найти Rhubarb и существующие TTS/VTS products;
- подсветил необходимость сравнить virtual cable и direct API.

Но несколько его конкретных утверждений не прошли проверку:

- был приведён неправильный `setParameterValue` вместо официального
  `InjectParameterDataRequest`;
- была заявлена неподтверждённая фиксированная WebSocket latency;
- был заявлен неподтверждённый лимит update rate;
- лицензия Rhubarb ошибочно называлась MPL-2.0, официальный файл указывает MIT;
- Vosk/Vosk-TTS ошибочно описывался как готовый PCM → phoneme timestamp extractor.

Правило для дальнейших исследований:

> Использовать Perplexity для discovery и списка гипотез; API, лицензии,
> производительность и интеграционные решения подтверждать официальной
> документацией, исходниками и собственным prototype.

---

## 15. Финальная рекомендация

Разрабатывать интеграцию стоит, но поэтапно:

### P1

- VTS connection/auth/reconnect;
- Typing/Thinking/Speaking/reaction hotkeys;
- Direct API amplitude mode без кабеля;
- setup wizard для существующего VTS Advanced Lipsync через virtual mic;
- offset calibration и строгий lifecycle reset.

### P2

- pure-Rust MFCC proof of concept;
- vowel weights;
- per-TTS-voice calibration;
- controlled A/B comparison.

### Не брать в live MVP

- Rhubarb subprocess;
- AI/deep-learning audio-to-face model;
- provider-specific phoneme contract;
- обещание «идеального» или «нулевой задержки» lip-sync.

Наиболее честная и сильная версия функции:

```text
Простой режим:
TTSBard PCM -> precomputed envelope -> VTS MouthOpen

Качественный существующий режим:
TTSBard virtual mic -> VTS Advanced Lipsync -> A/I/U/E/O

Будущий качественный режим без кабеля:
TTSBard final PCM -> calibrated MFCC -> VTS A/I/U/E/O
```

---

## 16. Источники

### Первичные

- [VTube Studio — Lipsync](https://github.com/DenchiSoft/VTubeStudio/wiki/Lipsync)
- [VTube Studio Plugin API](https://github.com/DenchiSoft/VTubeStudio)
- [uLipSync](https://github.com/hecomi/uLipSync)
- [uLipSync MIT License](https://raw.githubusercontent.com/hecomi/uLipSync/master/LICENSE.md)
- [Rhubarb Lip Sync](https://github.com/DanielSWolf/rhubarb-lip-sync)
- [Rhubarb License](https://github.com/DanielSWolf/rhubarb-lip-sync/blob/master/LICENSE.md)
- [Rhubarb 2 planning / real-time limitation](https://github.com/DanielSWolf/rhubarb-lip-sync/issues/95)
- [Fish Audio TTS stream with timestamps](https://docs.fish.audio/api-reference/endpoint/openapi-v1/text-to-speech-stream-with-timestamps)
- [pylipsync on PyPI](https://pypi.org/project/pylipsync/)
- [mfcc crate](https://docs.rs/crate/mfcc/latest)

### Product/category signals

- [VTS P.O.G.](https://eruben.itch.io/vts-pog)
- [Blerp VTube Studio TTS Pet](https://blerp.com/vtuber-pet)
- [Official VTube Studio plugin list](https://github.com/DenchiSoft/VTubeStudio/wiki/Plugins)

### Локальный код TTSBard

- `src-tauri/src/tts/engine.rs`
- `src-tauri/src/commands/tts_pipeline.rs`
- `src-tauri/src/audio/effects.rs`
- `src-tauri/src/playback.rs`
- `src-tauri/src/tts/fish.rs`
- [Архитектурные решения](../decisions/README.md)
