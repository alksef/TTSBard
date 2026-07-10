# Задача DeepSeek: FakeTLS — Round 5 (фикс формата obfs2-init на проводе)

**Рабочая директория:** `D:/RustProjects/grammers`. Round 1-4 готовы.
**Текущее состояние:** handshake полностью проходит (ServerHello HMAC OK), клиент шлёт `invokeWithLayer`,
но сервер **НЕ отвечает** → зависание на чтении (`step: reading bytes and sending up to 0 bytes`).
Simple `13370` работает.

Причина: формат 64-байтного obfs2-init **на проводе** не совпадает с эталоном → сервер не может
восстановить AES-CTR ключи → не расшифровывает пользовательские данные → молчит.

## Эталон gotd (точно)

`D:/Projects/td/mtproxy/obfuscated2/keys.go:54-72` + `obfuscated2.go:27-46`:

```go
// generateKeys:
init := generateInit(rand)          // 64 байта plaintext (с фильтром запрещённых первых байт)
k.createStreams(init[:], secret)    // encrypt/decrypt ciphers из ПЛАЙНТЕКСТ init
init[56:60] = protocol              // embed protocol tag (plaintext)
init[60:62] = dc                    // embed dc (plaintext, little-endian)
k.encrypt.XORKeyStream(encryptedInit[:], init[:])   // шифрует ВЕСЬ init → продвигает encrypt-CTR на 64
// header = init[0:56] (PLAINTEXT) + encryptedInit[56:64]   ← ТО, ЧТО ИДЁТ НА ПРОВОД
// (init[0:56] plaintext, init[56:64] encrypted)
// затем шлёт header через conn (=FakeTLS), всё дальнейшее шифруется encrypt (уже сдвинутым на 64)
```

**На провод после TLS-record framing:** `[0:56] PLAINTEXT | [56:64] ENCRYPTED`.

## alksev-реализация (БАГ)

`grammers-mtproto/src/tls/obfuscator.rs`, `client_handshake` (строки ~175-184):
```rust
let mut encrypted = frame;
send_cipher.apply_keystream(&mut encrypted);              // шифрует весь frame
encrypted[8..40].copy_from_slice(&frame[8..40]);          // restore key [8:40] в plaintext
encrypted[40..56].copy_from_slice(&frame[40..56]);        // restore iv [40:56] в plaintext
// НО [0:8] ОСТАЁТСЯ ЗАШИФРОВАННЫМ, а [56:64] зашифровано (верно)
```

alksev шлёт на провод: `[0:8] ENCRYPTED | [8:56] PLAINTEXT | [56:64] ENCRYPTED`.
Эталон ждёт:       `[0:8] PLAINTEXT  | [8:56] PLAINTEXT | [56:64] ENCRYPTED`.

Байт `[0:8]` зашифрован вместо plaintext → сервер не может валидировать init и/или восстановить cipher.

## Исправление

В `client_handshake`, после шифрования, восстановить **весь диапазон `[0:56]`** в plaintext
(а не только `[8:56]`):

```rust
let mut encrypted = frame;
send_cipher.apply_keystream(&mut encrypted);
// Восстановить [0:56] в plaintext — сервер читает первые 56 байт как plaintext (gotd keys.go).
encrypted[0..56].copy_from_slice(&frame[0..56]);
// [56:64] остаются зашифрованными (protocol + dc + padding tail) — как в gotd.
```

send_cipher уже сдвинут на 64 байта — НЕ трогать (он синхронизирован с сервером для последующих данных).
recv_cipher — свежий, оставить как есть.

**Важно:** `generate_frame` уже фильтрует запрещённые первые байты на plaintext `[0:8]` (через `is_frame_valid`),
так что plaintext `[0:56]` на проводе валиден (как в gotd `generateInit`).

## Проверка рассинхрона (заодно, если фикс не поможет)

После фикса `invokeWithLayer` должен расшифровываться сервером. Если зависание останется —
проверь, что `obfs2_send` в `stream.rs::poll_write` **не применяется повторно** к уже зашифрованным данным,
и что `obfs2_recv` в `poll_read` расшифровывает ответ сервера с **нуля** (recv_cipher свежий).
CCS-record (`0x14...`) шлётся сырыми байтами в обход obfs2 — это правильно (как gotd).

## Юнит-тест

Обнови тест `client_handshake` в obfuscator.rs так, чтобы он проверял: первые 56 байт возвращённого
frame = plaintext (совпадают с frame ДО шифрования), и только `[56:64]` зашифрованы.

## Не трогать
- server_hello.rs, client_hello.rs, record.rs, stream.rs framing, parse_secret, net/tcp.rs — работают.
- raw-mode НЕ возвращать.

## Проверка (Claude запустит)
```bash
cargo test -p grammers-mtproto --features mtproxy
MTPROXY_HOST=argeiphontes.ru MTPROXY_PORT=13371 \
  MTPROXY_SECRET=ee7b184b3f7c1ace06fa2efbbaa851f1a8617267656970686f6e7465732e7275 MTPROXY_DC_ID=2 \
  cargo run --release --example mtproxy --features "mtproxy grammers-session/sqlite-storage" -p grammers-client
```
Цель: `Ping result: Pong(...)`. Simple 13370 не сломать. НЕ коммить, живые тесты не запускать.
