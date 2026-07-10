# Задача DeepSeek: FakeTLS — Round 3 (1 точечный фикс: ServerHello HMAC)

**Рабочая директория:** `D:/RustProjects/grammers`. Round 1+2 готовы. Simple `13370` работает в debug И release.
FakeTLS `13371` теперь доходит до проверки ServerHello, но падает: `ServerHello HMAC verification failed`.

Тут — ОДИН фикс в `grammers-mtproto/src/tls/server_hello.rs`.

## Симптом

```
Error: ServerHello HMAC verification failed
```
Сервер присылает настоящий ServerHello (round 2 это починил), но alksev-реализация HMAC
считает его НЕ ТАК, как ждёт сервер (mtg), поэтому не сходится.

## Точная причина (сверено с эталоном gotd/td)

Эталон gotd: `D:/Projects/td/mtproxy/faketls/server_hello.go` (readServerHello). Алгоритм:

```go
const serverRandomOffset = 11
packet := packetBuf.Bytes()                       // ВСЕ 3 record'а: ServerHello + CCS + Application
var originalDigest [32]byte
copy(originalDigest[:], packet[11:43])             // 1) сохранить ServerRandom
var zeros [32]byte
copy(packet[11:43], zeros[:])                      // 2) ЗАНУЛИТЬ ServerRandom в пакете
mac := hmac.New(sha256.New, secret)
mac.Write(clientRandom[:])                         // 3) HMAC = SHA256(clientRandom ‖ packet_zeroed, secret)
mac.Write(packet)
bytes.Equal(mac.Sum(nil), originalDigest[:])       // 4) сравнить с сохранённым ServerRandom
```

**alksev-реализация в `server_hello.rs` (validate_server_hello, строки ~51-55) НЕ зануляет ServerRandom:**
```rust
let mut mac = HmacSha256::new_from_slice(secret).unwrap();
mac.update(client_random);
mac.update(server_hello);     // ← server_hello с ServerRandom НА МЕСТЕ (не занулён!)
let expected = mac.finalize().into_bytes();
// потом expected сравнивается с server_digest, который ВНУТРИ server_hello → гарантированно не совпадёт
```

Дополнительно alksev `extract`-ит `server_digest` из `server_hello[TLS_DIGEST_POS..]` (верно, смещение 11),
но при вычислении `expected` передаёт в HMAC тот же `server_hello` **с непочищенным ServerRandom** — это и есть баг.

## Исправление

В `validate_server_hello`:

1. `server_digest = server_hello[11..43]` (уже есть).
2. Создать **копию** `server_hello` (чтобы не мутировать вход): `let mut buf = server_hello.to_vec();`
3. **Занулить** `buf[11..43]` (ServerRandom, 32 байта): `buf[11..43].fill(0);`
4. `mac = HMAC-SHA256(secret)`, `mac.update(client_random)`, `mac.update(&buf)` (с занулённым ServerRandom).
5. Сравнить `mac.finalize()` с `server_digest` (constant-time, как сейчас).
6. `skip_tls_records(server_hello)` — вызывать на **исходном** `server_hello` (с настоящим ServerRandom), не на `buf`.

Важно: смещение 11 = `TLS_DIGEST_POS` (= 5 record-header + 6 handshake-header = 11) — то же, что у gotd.
`client_random` уже корректен (передаётся из stream.rs как `client_hello[11..43]` с XOR-timestamp — так и надо).

## Юнит-тест

Существующий `test_server_hello_validation_success` — **самообманный**: он кладёт HMAC `HMAC(client_random‖response)`
БЕЗ зануления (строки ~206-212) и поэтому проходит под сломанную реализацию. Перепиши его под корректный
алгоритм: при генерации мока тоже **зануляй ServerRandom** перед HMAC, иначе тест не покрывает реальный кейс.

Добавь тест с настоящим hex-дампом, если возможно (не обязательно).

## Проверка (Claude запустит, не ты)

```bash
cargo test -p grammers-mtproto --features mtproxy
# затем живой тест (release):
MTPROXY_HOST=argeiphontes.ru MTPROXY_PORT=13371 \
  MTPROXY_SECRET=ee7b184b3f7c1ace06fa2efbbaa851f1a8617267656970686f6e7465732e7275 MTPROXY_DC_ID=2 \
  cargo run --release --example mtproxy --features "mtproxy grammers-session/sqlite-storage" -p grammers-client
```
Ожидаемый результат: `Ping result: Pong(...)`, как у Simple 13370.

## Не трогать
- crypto-файлы (client_hello/obfuscator/record), stream.rs, framing.rs, parse_secret, net/tcp.rs — всё работает.
- Только `server_hello.rs::validate_server_hello` (+ его юнит-тест).
- НЕ делай git commit, не запускай живые тесты.
