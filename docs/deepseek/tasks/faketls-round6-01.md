# Задача DeepSeek: FakeTLS — Round 6 (ПОРЯДОК: CCS перед init)

**Рабочая директория:** `D:/RustProjects/grammers`. Round 1-5 готовы, init-формат корректен.
**Симптом:** handshake проходит (ServerHello OK, init отправлен), клиент шлёт `invokeWithLayer`,
сервер **молчит** → зависание (`step: reading bytes and sending up to 0 bytes`). Simple 13370 работает.

## Точная причина (сверено с gotd построчно)

gotd порядок post-ServerHello на проводе определяется тем, что **CCS-quirk срабатывает на ПЕРВОМ
`FakeTLS.Write` — а им оказывается `obfs2.Handshake` (запись init)**, а НЕ первый MTProto-пакет.

Поток вызовов gotd:
1. `tls.Handshake` → `ftls.Handshake` (ClientHello↔ServerHello). `FakeTLS.firstPacket == false`.
2. `tls.Handshake` → `obfs2.Handshake` → `o.conn.Write(o.header)` где `conn` = FakeTLS.
   Это **первый** вызов `FakeTLS.Write`. Внутри (`mtproxy/faketls/faketls.go:61-85`):
   ```go
   if !o.firstPacket {
       writeRecord(o.conn, record{Type: RecordTypeChangeCipherSpec, Data: []byte("\x01")})  // CCS FIRST
       o.firstPacket = true
   }
   writeRecord(o.conn, record{Type: RecordTypeApplication, Data: b})  // THEN the data (init)
   ```
   Т.е. на провод уходит: **CCS-record, ЗАТЕМ TLS-App-record(init)** — в ОДНОЙ первой записи в FakeTLS.

Следовательно gotd порядок на проводе после ServerHello:
```
[CCS-record: 0x14 0x03 0x03 0x00 0x01 0x01]
[TLS-App-record: 0x17 0x03 0x03 0x00 0x40 <64-byte init>]
[дальше MTProto — БЕЗ ещё одного CCS, firstPacket уже true]
```

## alksev-реализация (БАГ порядка)

`grammers-mtproto/src/tls/stream.rs::new()`:
- Step 4: шлёт TLS-App-record(init) — **ПЕРВЫМ**.
- Step 6: шлёт CCS-record — **ПОСЛЕ** init.

Получается порядок: `App(init) | CCS | MTProto` — **CCS не первым**. Сервер ждёт CCS первым post-ServerHello
байтом, получает init → десинхронизация → молчит → зависание.

## Исправление

В `stream.rs::new()` поменяй порядок: **СНАЧАЛА CCS-record, ЗАТЕМ init**.
Оба идут в TCP ДО создания `FakeTlsFraming` / до первого пользовательского `poll_write`
(т.к. это всё ещё handshake-фаза). Конкретно:

```rust
// Шаг A: CCS-record ПЕРВЫМ (gotd: CCS срабатывает на первом FakeTLS.Write)
let ccs_record = [TLS_RECORD_CHANGE_CIPHER, 0x03, 0x03, 0x00, 0x01, 0x01];
stream.write_all(&ccs_record).await?;

// Шаг B: ЗАТЕМ init как TLS-Application-record
let app_header = [TLS_RECORD_APPLICATION, 0x03, 0x03, 0x00, 64];
let mut combined = Vec::with_capacity(5 + 64);
combined.extend_from_slice(&app_header);
combined.extend_from_slice(&frame);
stream.write_all(&combined).await?;

// Шаг C: framing для последующего трафика. ВАЖНО: последующие poll_write НЕ должны слать ещё CCS —
// он уже отправлен. Убери CCS-quirk из конструктора framing (если он там дублировался).
let framing = FakeTlsFraming::new(stream);
```

Т.е. **убери** CCS-отправку через `framing.inner_mut()` в конце `new()` — CCS уже послан в шаге A,
сырыми байтами прямо в stream (как и init в шаге B), это правильно: и CCS, и init не шифруются obfs2
и идут как готовые TLS-record'ы.

**Финальный порядок на проводе** должен быть:
```
ClientHello(0x16...) [send] → ServerHello+CCS+App [recv, HMAC verified]
→ CCS-record(0x14...) → TLS-App-record(init 64B) → [MTProto: каждый пакет обёрнут framing+obfs2]
```

## Сверь с gotd ещё раз

После фикса воспроизведи ровно gotd-порядок:
- `faketls.go` Handshake = ClientHello + readServerHello (CCS тут НЕТ — это ответ серверера).
- первый `FakeTLS.Write` (это init из obfs2.Handshake) = CCS-record + App-record(init).
- последующие `Write` (MTProto) = только App-record (CCS больше не шлётся).

## Не трогать
- crypto (obfuscator.rs init-формат, client_hello, server_hello HMAC, record.rs), parse_secret, net/tcp.rs.
- raw-mode НЕ возвращать.

## Проверка (Claude запустит)
```bash
cargo test -p grammers-mtproto --features mtproxy
MTPROXY_HOST=argeiphontes.ru MTPROXY_PORT=13371 \
  MTPROXY_SECRET=ee7b184b3f7c1ace06fa2efbbaa851f1a8617267656970686f6e7465732e7275 MTPROXY_DC_ID=2 \
  cargo run --release --example mtproxy --features "mtproxy grammers-session/sqlite-storage" -p grammers-client
```
Цель: `Ping result: Pong(...)`. Simple 13370 не сломать. НЕ коммить, живые тесты не запускать.
