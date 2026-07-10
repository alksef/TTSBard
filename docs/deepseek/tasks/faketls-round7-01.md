# Задача DeepSeek: FakeTLS — Round 7 (диагностический дамп + сверка sync)

**Рабочая директория:** `D:/RustProjects/grammers`. Round 1-6: handshake проходит (ServerHello OK),
порядок CCS→init исправлен, init-формат `[0:56]plain+[56:64]enc` совпадает с gotd.
**СИМПТОМ прежний:** сервер МОЛЧИТ после init → зависание на чтении. Simple 13370 работает.
Гипотезы формата/порядка исчерпаны — нужен ЭМПИРИЧЕСКИЙ дамп.

## Задача A — добавь hex-дамп проводе (ВРЕМЕННО, для отладки)

В `grammers-mtproto/src/tls/stream.rs::new()` добавь `log::info!` с hex первых байт КАЖДОЙ записи,
уходящей на провод, ПЕРЕД каждым `stream.write_all`:

1. ClientHello: `log::info!("FAKETLS WIRE ClientHello: first 16 = {:02x?}", &client_hello[..16]);`
   и длина.
2. CCS: `log::info!("FAKETLS WIRE CCS: {:02x?}", &ccs_record);`
3. init (App-record): `log::info!("FAKETLS WIRE init rec header={:02x?} frame[0:8]={:02x?} frame[8:16]={:02x?} frame[56:64]={:02x?}", &combined[..5], &frame[..8], &frame[8..16], &frame[56..64]);`
   (покажи, что [0:56] plaintext, [56:64] encrypted).

Эти логи — ВРЕМЕННЫЕ, оставь их (Claude уберёт после отладки). Пометь комментарием `// DEBUG-FAKETLS`.

## Задача B — перепроверь синхронизацию AES-CTR ciphers vs gotd

Это самая вероятная причина молчания сервера. Прочитай и сверь с gotd:

**gotd (`obfuscated2/keys.go`):**
- `createStreams(init, secret)`: encrypt-key = `init[8:40]`, encrypt-iv = `init[40:56]`,
  затем `encryptKey = SHA256(encryptKey ‖ secret)`. iv НЕ хешируется. Создаёт encrypt-CTR.
- decrypt: `getDecryptInit(init)` = reversed `init[8:56]` (48 байт), decrypt-key = rev[0:32],
  decrypt-iv = rev[32:48], `decryptKey = SHA256(decryptKey ‖ secret)`.
- `generateKeys`: createStreams ИЗ PLAINTEXT init, ЗАТЕМ `init[56:60]=protocol, init[60:62]=dc`,
  ЗАТЕМ `encrypt.XORKeyStream(encryptedInit, init)` — шифрует ВЕСЬ init, counter→64.
- header = `init[0:56]` + `encryptedInit[56:64]`.
- **КРИТИЧНО**: encrypt-CTR, которым шифровали init, **он же** шифрует subsequent MTProto
  (продолжается с counter=64). iv остаётся начальный.

**alksev `obfuscator.rs::client_handshake`:**
- `derive_cipher(frame, secret)`: key=`frame[8:40]`, iv=`frame[40:56]`, `derived_key=SHA256(key‖secret)`,
  `Aes256Ctr::new(derived_key, iv)`. ✓ совпадает.
- `send_cipher.apply_keystream(&mut encrypted)` над всем frame → counter→64. ✓
- restore `[0:56]` в plaintext. ✓

**КРИТИЧЕСКАЯ ПРОВЕРКА (возможный баг):** alksev `Aes256Ctr` — какой режим CTR?
gotd `createCTR` → `ctr.NewCtrWithReusedNonce`? Проверь `D:/Projects/td/mtproxy/obfuscated2/keys_util.go` `createCTR`
и `crypto/sha256.go`. AES-CTR для MTProto-obfuscation использует **Ctr128BE** (big-endian counter, 128-bit).
alksev obfuscator.rs использует `ctr::Ctr128BE<aes::Aes256>` (видно в Round 1). ✓ совпадает.

**ЕЩЁ ОДНА ПРОВЕРКА (частый баг):** после Round 5 (restore `[0:56]`), `send_cipher` применён ко всему
64-байтному `encrypted`. НО restore `[0:56]` делается ПОСЛЕ `apply_keystream` — значит cipher уже сдвинут на 64,
это правильно. Убедись, что restore НЕ вызывает повторного `apply_keystream`.

**И САМОЕ ВАЖНОЕ для recv:** alksev возвращает `recv_cipher` СВЕЖИЙ (counter=0). Сервер шифрует свои ответы
СВОИМ encrypt-cipher (= наш recv), counter=0 (с ответа начинается). ✓ Но проверь: в `stream.rs::poll_read`
recv_cipher применяется к данным из framing. framing сначала strip'ит TLS-record, потом obfs2_recv
расшифровывает. Проверь, что framing НЕ съедает байты ответа некорректно (CCS в ответе игнорируется).

## Задача C — если найдёшь расхождение — исправь

Если в задаче B найдёшь, что send/recv cipher, iv, или CTR-режим не совпадает с gotd — исправь.
Если всё совпадает — оставь только дамп (задача A) и сообщи, что формат корректен, нужна отладка по дампу.

## Не трогать
- client_hello, server_hello, record.rs, parse_secret, net/tcp.rs.
- raw-mode НЕ возвращать.

## Проверка (Claude запустит с дампом)
```bash
RUST_LOG=info MTPROXY_HOST=argeiphontes.ru MTPROXY_PORT=13371 \
  MTPROXY_SECRET=ee7b184b3f7c1ace06fa2efbbaa851f1a8617267656970686f6e7465732e7275 MTPROXY_DC_ID=2 \
  cargo run --release --example mtproxy --features "mtproxy grammers-session/sqlite-storage" -p grammers-client
```
НЕ коммить, живые тесты не запускать.
