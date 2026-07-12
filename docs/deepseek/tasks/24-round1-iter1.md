Реализуй задачу перехода аудиоэффектов на Signalsmith Stretch в репозитории D:\RustProjects\app-tts-v2.

Сначала прочитай и соблюдай:
- AGENTS.md
- docs/deepseek/WORKFLOW.md
- docs/stage/24-signalsmith-stretch-audio-effects.md
- docs/deepseek/plan/24-signalsmith-stretch-audio-effects.md

Ты пишешь код самостоятельно. Не изменяй несвязанные незакоммиченные файлы и не создавай коммит.

Требования реализации:
1. Проанализируй текущие `src-tauri/src/audio/effects.rs`, `src-tauri/src/commands/tts_pipeline.rs`, настройки/DTO, UI аудиоэффектов, `src-tauri/Cargo.toml` и `src-tauri/build.rs`.
2. Добавь зафиксированный Signalsmith Stretch header-only source/vendor и license notice. Обычная сборка должна быть воспроизводимой офлайн; не полагайся на runtime/network download.
3. Реализуй минимальный C++ C ABI bridge и сборку через `cc` в `build.rs`, совместимую с Windows/MSVC. Bridge обязан корректно обрабатывать interleaved float PCM, channels/sample rate, tempo, pitch, formant correction, подачу блоков, flush/drain и уничтожение.
4. Реализуй безопасный Rust RAII wrapper с `Result`, проверками входных данных и корректным освобождением native processor при ошибках/повторной обработке.
5. Замени последовательные `rubato`/`pitch_shift`/последующий `trim_silence` на единый Signalsmith путь после DeepFilterNet. Не обрезай хвост амплитудным heuristic после Signalsmith. Сохрани WAV encoder и существующий public AudioEffects DTO/API.
6. Поле storage/API `speed` оставь для backward compatibility, но его семантика в обработке и UI должна быть tempo. Выбери безопасный диапазон примерно 0.75..1.50x, явно ограничь экстремальные значения и обнови текст/подписи UI с «Скорость» на «Темп». Pitch по-прежнему -100..100 -> -12..+12 semitones. Formant correction включи по умолчанию и не смешивай с DeepFilterNet.
7. Добавь тесты, покрывающие mono/stereo, sample rates 16/24/44.1/48 kHz, tempo-only, pitch-only, combined, tail/flush, invalid input и повторные вызовы. Проверяй конечность PCM, длительность в допустимой погрешности и tempo-only pitch preservation для sine.
8. Не запускай полноценную сборку без необходимости, но выполни форматирование при возможности. В конце перечисли изменённые файлы и известные ограничения.

Не отмечай требования выполненными без фактического кода. Не переписывай весь проект и не меняй unrelated UI.
