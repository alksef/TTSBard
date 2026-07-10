# Задача DeepSeek: FakeTLS — Round 4 (фикс skip_tls_records + сверить post-handshake)

**Рабочая директория:** `D:/RustProjects/grammers`. Round 1-3 готовы. ServerHello HMAC теперь проходит.
Новый симптом: `Expected ApplicationData (0x17) at after ChangeCipherSpec, got 0x01`.

## ФИКС #1 (точно) — `skip_tls_records` не пропускает payload CCS

Файл `grammers-mtproto/src/tls/server_hello.rs`, функция `skip_tls_records`, ветка ChangeCipherSpec:

Текущий код (строки ~92-109) делает только `offset += 5;` — пропускает **только 5-байтный header** CCS,
**но не его payload**. У CCS length-field = 1 (payload = байт `0x01`). Поэтому следующий `data[offset]`
читает payload CCS (`0x01`) вместо типа следующего record'а (`0x17`).

`stream.rs` собирает `response` как **полные record'ы** (header + payload) для всех трёх записей.
Значит CCS в буфере = `[0x14, 0x03, 0x03, 0x00, 0x01, 0x01]` (header + 1 байт payload).

**Исправление:** для CCS читать его length-field и пропускать `5 + length`:
```rust
// ChangeCipherSpec (type 0x14)
let ccs_len = u16::from_be_bytes([data[offset+3], data[offset+4]]) as usize;
offset += 5 + ccs_len;   // было offset += 5  — пропускало payload
```
(Для Application-ветки `5 + app_len` уже сделано верно — не трогать.)

## ФИКС #2 (сверка post-handshake-цепочки с эталоном gotd)

После того как ServerHello валиден, handshake продолжается в `stream.rs::new()`. Сверь КАЖДЫЙ шаг с
эталоном `D:/Projects/td/mtproxy/faketls/faketls.go` + `D:/Projects/td/mtproxy/obfuscator/obfuscator.go`
и `obfuscated2/keys.go`. Особенно:

1. **obfs2-init (64 байта) шлётся как TLS-Application-record** (`0x17 0x03 0x03 0x00 0x40` + frame).
   В `stream.rs` это уже так (строки ~100-112). ОК — проверь, что frame = результат `client_handshake`.
2. **CCS-quirk** (gotd `faketls.go:62-75`): перед ПЕРВЫМ пользовательским пакетом (НЕ во время handshake,
   а при первом `poll_write` после конструирования) клиент шлёт CCS-record `[0x14,0x03,0x03,0x00,0x01,0x01]`.
   Сейчас CCS шлётся в `new()` (stream.rs ~119) — это СРАЗУ после obfs2-init, ещё до пользовательских данных.
   Проверь по gotd: CCS должен идти **между** obfs2-init и первым MTProto-пакетом. Если в `new()` ты шлёшь
   CCS сразу после init (до любых MTProto-данных) — это эквивалентно gotd (т.к. obfs2-init тоже не MTProto).
   **Оставь как есть, ЕСИ gotd-порядок соблюдён:** ClientHello → ServerHello → obfs2-init → CCS → [MTProto...].
3. **obfs2-init framing**: gotd шлёт init **внутри** FakeTLS.Write (т.е. обёрнутым в TLS-record). alksev шлёт
   руками `[0x17,...]+frame` — эквивалентно. ОК.
4. **send_cipher уже сдвинут на 64 байта** (применён к init в `client_handshake`) — НЕ переинициализировать,
   НЕ применять повторно. recv_cipher — свежий. Проверь в `poll_write`/`poll_read` (stream.rs) —
   там `obfs2_send.apply_keystream` / `obfs2_recv.apply_keystream`. ОК.

Если все шаги эквивалентны gotd — фикса #1 достаточно. Если найдёшь расхождение — исправь.

## Что НЕ трогать
- `validate_server_hello` (HMAC) — работает (round 3).
- `build_client_hello`, `client_handshake`, `record.rs`, `parse_secret`, `net/tcp.rs` — работают.
- raw-mode НЕ возвращать.

## Проверка (Claude запустит)
```bash
cargo test -p grammers-mtproto --features mtproxy
MTPROXY_HOST=argeiphontes.ru MTPROXY_PORT=13371 \
  MTPROXY_SECRET=ee7b184b3f7c1ace06fa2efbbaa851f1a8617267656970686f6e7465732e7275 MTPROXY_DC_ID=2 \
  cargo run --release --example mtproxy --features "mtproxy grammers-session/sqlite-storage" -p grammers-client
```
Цель: `Ping result: Pong(...)`. Simple `13370` тоже не сломать. НЕ коммить, не запускай живые тесты сам.
