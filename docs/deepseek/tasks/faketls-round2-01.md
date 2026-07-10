# Задача DeepSeek: FakeTLS — Round 2 (2 точечных фикса)

**Рабочая директория:** `D:/RustProjects/grammers`. Round 1 готов (код на месте, 67 unit-тестов зелёные).
Тут — два конкретных бага, найденных живым тестом. НЕ переписывай архитектуру.

## Контекст (что уже работает / не работает)

- `cargo test -p grammers-mtproto --features mtproxy` — 67/67 ок.
- Живой тест **Simple** `argeiphontes.ru:13370`: ✅ работает (Pong получен) в **release**.
- Живой тест **FakeTLS** `argeiphontes.ru:13371`: ❌ падает с ошибкой ниже.
- **Debug-сборка обоих ключей:** ❌ STATUS_STACK_OVERFLOW (см. фикс #2).

Команда проверки (release — обязательна для обоих фиксов):
```bash
cd D:/RustProjects/grammers
# Simple (регрессия — должна работать):
MTPROXY_HOST=argeiphontes.ru MTPROXY_PORT=13370 MTPROXY_SECRET=4758456789abcdef0123456789abcdef MTPROXY_DC_ID=2 \
  cargo run --release --example mtproxy --features "mtproxy grammers-session/sqlite-storage" -p grammers-client
# FakeTLS (цель):
MTPROXY_HOST=argeiphontes.ru MTPROXY_PORT=13371 MTPROXY_SECRET=ee7b184b3f7c1ace06fa2efbbaa851f1a8617267656970686f6e7465732e7275 MTPROXY_DC_ID=2 \
  cargo run --release --example mtproxy --features "mtproxy grammers-session/sqlite-storage" -p grammers-client
```

## ФИКС #1 — `validate_server_hello` видит `0x00` (BUG: начальные нули в буфере ответа)

**Симптом:** `Error: Not a ServerHello (type: 0x00)`.

**Причина** — в `grammers-mtproto/src/tls/stream.rs`, функция `new()`, чтение ответа сервера:

```rust
let mut response = vec![0u8; 8192];   // ← 8192 НУЛЕЙ в начале!
loop {
    // ...
    response.extend_from_slice(&header);   // ← данные дописываются В КОНЕЦ, после 8192 нулей
    // ...
}
// ...
validate_server_hello(&response, &client_random, secret)  // ← читает response[0] = 0x00
```

`validate_server_hello` смотрит `server_hello[0]` как record_type, но первые 8192 байта `response` —
это нули от инициализации `vec![0u8; 8192]`. Реальные record'ы лежат дальше.

**Исправление:** инициализировать `response = Vec::new()` (или `Vec::with_capacity(8192)`), НЕ заполнять
нулевой префикс. Все данные ServerHello/CCS/Application должны идти с `response[0]`.

Дополнительно проверь: после фикса ServerHello (0x16), CCS (0x14), Application (0x17) должны идти подряд
с начала `response`. Если `validate_server_hello` ожидает только ServerHello-record (а не все три) —
сверься с её реализацией в `server_hello.rs` (`skip_tls_records` пропускает все три). Буфер должен
содержать ровно `[ServerHello-record | CCS-record | Application-record]`.

## ФИКС #2 — STATUS_STACK_OVERFLOW в debug (BUG: 16 КБ массив в async-структуре)

**Симптом:** `cargo run` (debug) → `STATUS_STACK_OVERFLOW` (0xc00000fd). В release работает.

**Причина** — в `grammers-mtproto/src/tls/framing.rs`:
```rust
pub struct FakeTlsFraming<S> {
    inner: S,
    read_state: ReadState,
    read_buf: [u8; MAX_TLS_CIPHERTEXT_SIZE as usize],  // ← [u8; 16640] в async-структуре!
    write_state: WriteState,
}
```
`FakeTlsFraming` (а значит и `FakeTlsStream`, и `NetStream::MtProxyFakeTls`) держит 16 КБ на **стеке**.
В async/future (tokio) это раздувает state-machine future так, что debug-сборка переполняет стек.

**Исправление:** заменить `read_buf: [u8; 16640]` на **heap**-выделенный буфер. Варианты:
- `read_buf: Box<[u8; MAX_TLS_CIPHERTEXT_SIZE as usize]>` (минимальное изменение, аллоцируется в `new`), ИЛИ
- `read_buf: Vec<u8>` с `with_capacity(MAX_TLS_CIPHERTEXT_SIZE)`.
Проверь, что вся логика `poll_read` (Payload/Buffered ветки, `copy_within`) корректна после замены
(`Vec` — индексируется так же; `Box<[u8;N]>` — тоже).

После фикса: `cargo run` (БЕЗ `--release`) для обоих ключей НЕ должен падать со stack overflow.

## Что НЕ трогать

- Не возвращай raw-mode. Не добавляй handshake.rs. Не меняй crypto-файлы
  (client_hello/server_hello/obfuscator/record). Не меняй parse_secret по длине.
- `validate_server_hello` правь ТОЛЬКО если фикс #1 выявит, что она сама ждёт лишнее
  (но судя по логу Round 1 — она корректна, проблема в буфере вызывающего).

## Критерий готовности (Claude проверит, не отмечай сам)

1. `cargo test -p grammers-mtproto --features mtproxy` — зелёный.
2. `cargo run` (debug, оба ключа) — без stack overflow.
3. `cargo run --release` (FakeTLS 13371) — `Ping result: Pong(...)`, как у Simple 13370.
4. Simple 13370 всё ещё работает (регрессия).

НЕ делай git commit, не запускай живые тесты сам — это сделает Claude.
