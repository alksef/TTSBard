# Third-Party Notices

This document lists the third-party components that are directly bundled into or
vendored with the TTSBard application. It covers directly bundled/vendor assets
only; it is not a generated inventory of every transitive Rust or npm package.

## Russian Hunspell dictionary (ru_RU)

- Component: Russian spell-checking dictionary (`ru.aff`, `ru.dic`)
- Source: <https://github.com/LibreOffice/dictionaries/tree/32b006a2c22a4ac7e8ed3f03346f7b3d85a970a4/ru_RU>
- Revision: `32b006a2c22a4ac7e8ed3f03346f7b3d85a970a4`
- Copyright: Copyright (c) 1997-2008, Alexander I. Lebedev
- License: BSD-style redistribution terms; see the included notice below
- License file: `resources/dict/LICENSE.txt`
- SHA-256 (`ru.aff`): `38CE7D4AF78E211E9BAFE4BF7E3D6A2C420591136CB738EC6648F8FDF6524CD7`
- SHA-256 (`ru.dic`): `F6047416A0204ADBECF3A451B874EC8A97EE37E2CBC714466EF04D8DBCC0D6FC`

## eSpeak NG

- Component: Text-to-phoneme engine data bundled via Piper
- Source: <https://github.com/espeak-ng/espeak-ng>
- Revision: `724808c5a83f9ef95fdd0db886ba7ba537ff224a`
- Bundled via piper-rs revision `346f10f6e8520b2ee21e4d0117c1ca0e38e5cd0f`
- License: GNU GPL version 3
- License file: root `LICENSE` (bundled through `bundle.licenseFile`)

## Signalsmith Stretch

- Component: Audio time-stretch DSP (vendored)
- Source: <https://github.com/Signalsmith-Audio/signalsmith-stretch>
- Copyright: Copyright (c) 2022 Geraint Luff / Signalsmith Audio Ltd.
- License: MIT
- License file (source tree): `src-tauri/vendor/signalsmith-stretch/LICENSE.txt`
- License file (installed): `third-party/licenses/signalsmith-stretch-LICENSE.txt`

## Signalsmith Linear

- Component: Linear DSP helpers (vendored)
- Source: <https://github.com/Signalsmith-Audio/linear>
- Copyright: Copyright (c) 2025 Signalsmith Audio
- License: MIT
- License file (source tree): `src-tauri/vendor/signalsmith-linear/LICENSE.txt`
- License file (installed): `third-party/licenses/signalsmith-linear-LICENSE.txt`
