# LunaTranslator → TTSBard: граница интеграции текста

**Дата:** 2026-08-31
**Связанный roadmap:** [ROADMAP-087](../roadmap/active/087-external-text-input-server.md)

## Вопрос

Нужно ли переносить часть OCR/HOOK/translation-функционала LunaTranslator в
TTSBard или эмулировать её TTS API, чтобы стример мог получать игровой текст в
TTSBard быстрее и проще?

## Проверенные источники

- LunaTranslator v10.16.5.2, локальные исходники `D:\Projects\LunaTranslator`:
  `LunaTranslator/tts/vitsSimpleAPI.py` и встроенная сетевая служба.
- TTSBard: существующие очередь, PCM pipeline и маршрутизация доставки.

## Наблюдения

`vitsSimpleAPI.py` — Python-adapter: Luna делает HTTP GET к `/voice/speakers`
и `/voice/<model>`, ожидает audio bytes, после чего воспроизводит их сама.
`selfbuild` также загружает и вызывает Python-обёртку. Поэтому эмуляция этого
контракта передаст владение playback LunaTranslator и обойдёт очередь,
виртуальный микрофон, DSP и единое управление TTSBard.

Отдельный Python HTTP-server не является главным источником задержки: при
localhost транспорт обычно мал по сравнению с inference TTS. Встроенные модули
Luna также Python, так что переписывание тонкой обёртки в Rust не даёт
содержимого ускорения без замены самого OCR/translation/TTS pipeline.

У Luna есть WebSocket text-output (`/api/ws/text/origin` и
`/api/ws/text/trans`), который потенциально удобнее custom TTS-script. Но
текущая сетевая служба bind-ится на `0.0.0.0` и публикует дополнительные API;
это не безопасный generic default без отдельного решения о bind/auth.

## Вывод

TTSBard не дублирует LunaTranslator. Luna остаётся upstream для игрового
текста, OCR, hook и перевода; TTSBard — downstream для очереди, синтеза и
аудиовыходов. Для MVP выбран loopback Input Server:

```text
LunaTranslator / любой local producer → POST text → TTSBard queue → audio-only playback
```

Он принимает текст на `127.0.0.1`, создаёт audio-only job и намеренно не
рассылает эту фразу в WebView или Twitch. В Luna нужно отключить
`Поведение → Озвучивать автоматически`: тогда её TTS-adapter не вызывается, а
готовый текст отправляется в TTSBard.

## Когда пересмотреть

- Добавить явный Luna WebSocket connector после проверки его bind/firewall и
  настройки пользователя.
- Рассматривать VITS/OpenAI-TTS-compatible audio API только для клиентов,
  которым действительно нужны audio bytes, а не для пути Luna → TTSBard.
- Возвращаться к дублированию upstream-функций только при отдельном спросе на
  автономный streamer workflow без LunaTranslator.
