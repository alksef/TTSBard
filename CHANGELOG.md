# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0-alpha] - 2026-07-10

### Added

- **Офлайн-проверка орфографии (Hunspell):** Встроенная локальная проверка орфографии в реальном времени.
- **История фраз (Phrase History):** Встроенный журнал отправленных сообщений с поиском по подстроке и двумя режимами вставки (добавление в конец / замена с подтверждением).
- **Множество вкладок (Tabs):** Поддержка независимых вкладок в текстовом редакторе.
- **Профили звуков (Sound Sets):** Возможность создавать неограниченные наборы звуков для саундпада.
- **Защита от захвата экрана (OBS/Discord):** Скрытие окон приложения на стриме во избежание попадания в кадр.
- **Внешнее подключение OBS (SSE):** Возможность удаленным стримерам выводить ваши реплики у себя через SSE-поток.
- **Fish Audio TTS:** Поддержка провайдера Fish Audio с выбором и клонированием голоса.
- **Numpad Control:** Управление воспроизведением через цифровой блок клавиатуры.
- **Панель воспроизведения (Playback Control):** Компактный оверлей для паузы, остановки речи и быстрого повтора реплик из кэша.

### Changed

- Доработан клиент Telegram (Silero Bot): добавлена поддержка FakeTLS для MTProxy.
- Оптимизирована сборка: теперь генерируется `.msi`-установщик и архив `resources.zip` со словарями.

## [0.9.0] - 2026-04-27

### Fixed

- Resolve MTProxy connection and build warnings


## [0.8.0] - 2026-04-16

### Added

- Implement audio effects with fade-out trim

### Documentation

- Update CHANGELOG for v0.8.0

### Fixed

- Align AudioPanel padding with other panels


**[Full Changelog](https://github.com/yourusername/ttsbard/compare/[object]...v0.8.0)**
## [0.7.1] - 2026-04-15

### Documentation

- Update CHANGELOG for v0.7.1

### Fixed

- Sync Tauri window theme with app theme


**[Full Changelog](https://github.com/yourusername/ttsbard/compare/[object]...v0.7.1)**
## [0.7.0] - 2026-04-15

### Added

- Add Telegram/Silero TTS speaker selection

### Documentation

- Update CHANGELOG for v0.7.0

### Refactored

- Update FishAudio TTS UI text from 'voice models' to 'voices'


**[Full Changelog](https://github.com/yourusername/ttsbard/compare/[object]...v0.7.0)**
## [0.6.0] - 2026-04-14

### Added

- Add Fish Audio TTS provider

### Documentation

- Remove floating panel references from UI guide
- Update all documentation, remove obsolete files
- Update CHANGELOG for v0.6.0

### Fixed

- Resolve 7 critical issues from code review
- Resolve 3 security issues from code review
- Eliminate scrollbar flicker in minimal mode
- Disable async_openai internal retries on rate limit
- Correct OpenAI TTS key save behavior and voice toast color
- Reload live replacements in InputPanel after saving in settings
- Skip webview broadcast for !! prefix commands
- Apply SOCKS proxy on startup when use_proxy is enabled
- Clear Fish Audio reference_id when selected model is deleted
- Show TTS errors as global toasts from InputPanel
- Save local TTS URL only on button click
- Restart autoHide timer on message change in StatusMessage
- Remove wasm32 platform deps from lock file to fix npm ci on Windows
- Remove unused getIcon function to fix CI build

### Refactored

- Remove dead code across backend modules
- Resolve 7 minor issues from code review
- Code optimizations from code review

### Style

- Align Silero reconnect button with save button style
- Use theme-aware background for example block in InfoPanel


**[Full Changelog](https://github.com/yourusername/ttsbard/compare/[object]...v0.6.0)**
## [0.5.0] - 2026-04-11

### Added

- Add minimal UI mode with toggle button
- Add customizable hotkeys for main window and sound panel
- Remove floating window (text interception)

### Documentation

- Update CHANGELOG for v0.5.0


**[Full Changelog](https://github.com/yourusername/ttsbard/compare/[object]...v0.5.0)**
## [0.4.0] - 2026-03-24

### Added

- Add WebView security with UPnP and token authentication

### Documentation

- Update WebView section in README

### Refactored

- Extract TtsPanel and SettingsPanel into smaller components


**[Full Changelog](https://github.com/yourusername/ttsbard/compare/[object]...v0.4.0)**
## [0.3.0] - 2026-03-22

### Added

- Add tabs to settings panel
- Add AI settings panel (OpenAI, Z.ai)
- Integrate AI for text correction

### Documentation

- Add code review infrastructure and declutter CLAUDE.md
- Update CHANGELOG for v0.3.0

### Refactored

- Improve settings panel layout


**[Full Changelog](https://github.com/yourusername/ttsbard/compare/[object]...v0.3.0)**
## [0.2.0] - 2026-03-19

### Added

- Unified theme system - CSS variables refactoring
- Implement theme switching with light/dark modes

### Documentation

- Fix date format in plan naming convention

### Fixed

- Increase tokio runtime stack size to prevent stack overflow


**[Full Changelog](https://github.com/yourusername/ttsbard/compare/[object]...v0.2.0)**
## [0.1.4] - 2026-03-16

### Added

- Add MTProxy support for Telegram and refactor initialization
- Allow proxy selection before Telegram connection

### Documentation

- Update CHANGELOG for v0.1.4

### Deps

- Switch to grammers-mtproxy fork repository

### Style

- Update TelegramAuthModal to dark theme


**[Full Changelog](https://github.com/yourusername/ttsbard/compare/[object]...v0.1.4)**
## [0.1.3] - 2026-03-15

### Added

- Add test sound playback button for audio devices
- Implement proxy settings for OpenAI TTS and Telegram Silero

### Documentation

- Add SSE integration guide

### Fixed

- Improve UI consistency in AudioPanel and Sidebar
- Enable virtual mic button now properly updates state
- Virtual mic device selection persists and UI updates correctly
- Prevent layout overflow in AudioPanel device selectors

### Style

- Update sidebar icons for TTS and Audio sections


**[Full Changelog](https://github.com/yourusername/ttsbard/compare/[object]...v0.1.3)**
## [0.1.2] - 2026-03-14

### Added

- Implement unified settings loading (get_all_app_settings)

### Fixed

- Show add button and stats always in sound panel
- Implement Twitch restart button functionality


**[Full Changelog](https://github.com/yourusername/ttsbard/compare/[object]...v0.1.2)**
## [0.1.1] - 2026-03-13

### Added

- Implement structured logging system with tracing

### Documentation

- Update code review prompt guidelines
- Add guidelines for subagent usage

### Fixed

- Normalize spacing in sound panel bindings section

### Deps

- Update all dependencies


**[Full Changelog](https://github.com/yourusername/ttsbard/compare/[object]...v0.1.1)**
## [0.1.0] - 2026-03-13

### Added

- Implement code review fixes #004 (optimization and improvements)
- Implement text preprocessing pipeline with number conversion

### Documentation

- Add MCP Perplexity and code review guidelines to CLAUDE.md
- Update user guide and unify UI spacing (#41)
- Update CHANGELOG for 0.1.0 release

### Fixed

- Implement code review fixes #003
- Implement code review fixes #005 (critical stability improvements)
- Implement code review fixes #007

### Refactored

- Split lib.rs into modular architecture (240 lines from 1097)
- Modernize UI with lucide icons and improve UX
- Migrate WebView from WebSocket to SSE and simplify UI
- Send TextSentToTts event immediately before audio playback
- Implement tag-based versioning for releases

### Style

- Unify settings panel blocks with info panel styling
- Center quick editor hint text


**[Full Changelog](https://github.com/yourusername/ttsbard/compare/[object]...v0.1.0)**
## [0.1.0-pre] - 2026-03-08

### Added

- Improve UI/UX across multiple panels
- Improve input field UX and fix all clippy warnings
- Update sidebar icons
- Add global settings panel with window capture protection
- Unify Floating and SoundPanel window creation approach
- Add position save/restore for SoundPanel window
- Improve sidebar layout and reduce panel padding
- Improve UI consistency and localize buttons
- Add quick editor mode
- Add close button commands for floating windows

### Documentation

- Add window capture protection analysis
- Add quick start guide and key architecture decisions
- Update README with improved structure and content

### Fixed

- Apply window capture protection after show() and fix SoundPanel CSS
- Prevent SoundPanel flicker and improve window capture protection
- Apply window capture protection after show() to fix blank windows
- Hide scrollbar and prevent overflow in floating windows
- Manage state managers before window creation to fix state errors
- Remove transparent border at bottom of floating windows
- Save selected TTS provider to settings
- Hide console in release builds on Windows
- Load TTS provider from settings manager instead of app state
- Initialize correct TTS provider on startup

### Refactored

- Improve sidebar UX with floating toggle button
- Simplify tray context menu to only show "Quit"
- Remove Esc hint from soundpanel floating window


**[Full Changelog](https://github.com/yourusername/ttsbard/compare/[object]...v0.1.0-pre)**
## [0.1.0-beta] - 2026-03-07

### Added

- Add exclude from screen recording for floating windows
- Implement WebView Source feature (stages 1-4 complete)
- Complete WebView Source testing and refinements
- Implement WebView server for OBS integration
- Add Twitch Chat integration
- Improve Twitch Chat integration
- Implement push-based status updates for Twitch Chat
- Add status refresh button to Twitch panel
- Implement TTSVoiceWizard Local provider with HTTP API

### Documentation

- Add WebView Source design document
- Add WebView Source implementation plan
- Add webview-source idea summary
- Save webview implementation progress checkpoint
- Add WebView Source final completion report
- Add Twitch Chat integration design document
- Update code review prompt with ignore section
- Add code review fixes implementation plan
- Update TTS Provider settings documentation

### Fixed

- Suppress unused methods warning in settings.rs
- Escape Vue template curly braces with v-pre directive
- Properly track Twitch connection status in UI
- Add periodic status polling in TwitchPanel UI
- Fetch initial Twitch status on UI mount for Start on boot
- Use Twitch event loop for auto-start instead of direct client creation
- Implement 21 code review fixes - CRITICAL, SECURITY, OPTIMIZE
- Improve WebView and Twitch settings UX
- Implement 9 code review fixes - CRITICAL, MINOR, OPTIMIZE
- Refactor OpenAI TTS settings and fix voice persistence


**[Full Changelog](https://github.com/yourusername/ttsbard/compare/[object]...v0.1.0-beta)**
## [0.1.0-alpha] - 2026-03-05

### Added

- Initial TTSBard application


**[Full Changelog](https://github.com/yourusername/ttsbard/compare/[object]...v0.1.0-alpha)**
<!-- Generated by git-cliff -->
