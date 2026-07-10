# Задача DeepSeek: FakeTLS — Round 14 (tdesktop prefix-модель для init)

**Рабочая директория:** `D:/RustProjects/grammers`.

## Контекст (важно!)

После 13 раундов: faketls-**алгоритм** alksev криптографически корректен (obfs2-crypto байт-идентичен
gotd, framing-запись корректна). НО Telegram-сервер **молчит** на alksev-пакеты, тогда как
**официальный клиент tdesktop работает**. Анализ `D:/Projects/tdesktop/Telegram/SourceFiles/mtproto/details/mtproto_tls_socket.cpp`
показал **архитектурное отличие**, которое, вероятно, и есть корень проблемы.

## Каноническая модель tdesktop (читать ОБЯЗАТЕЛЬНО)

`D:/Projects/tdesktop/Telegram/SourceFiles/mtproto/details/mtproto_tls_socket.cpp`, метод `TlsSocket::write(prefix, buffer)`:

```cpp
const auto kClientPrefix = "\x14\x03\x03\x00\x01\x01";   // CCS record
const auto kClientHeader = "\x17\x03\x03";                 // Application record header

void TlsSocket::write(prefix, buffer) {
    if (!prefix.empty())
        _socket.write(kClientPrefix);                       // CCS — только когда есть prefix
    while (!buffer.empty()) {
        const auto write = min(kClientPartSize - prefix.size(), buffer.size());  // kClientPartSize=2878
        _socket.write(kClientHeader);                        // 0x17 0x03 0x03
        const auto size = qToBigEndian(uint16(prefix.size() + write));
        _socket.write(&size, 2);                             // длина = prefix.size + chunk
        if (!prefix.empty()) { _socket.write(prefix); prefix = {}; }   // init = ПЕРВЫЕ байты первого record
        _socket.write(buffer[..write]);                      // obfuscated MTProto (нет доп. шифра в TlsSocket)
        buffer = buffer[write..];
    }
}
```

**Ключевые моменты tdesktop:**
1. **`prefix`** = transport-tag obfs-init (первые байты obfuscated-рукопожатия, несущие AES-keys/IV).
2. **CCS (`kClientPrefix`) шлётся ОДИН раз**, непосредственно перед первым record'ом (который содержит prefix).
3. **obfs-init встраивается ВНУТРЬ первого Application-record**, как его первые байты, **вместе с первыми
   байтами MTProto-данных**. НЕ отдельным record'ом!
4. Длина первого record'а = `prefix.size() + write_chunk`.
5. TlsSocket сам AES-CTR не делает — данные уже obfuscated вышележащим слоем.

## Текущая (НЕправильная) модель alksev

`grammers-mtproto/src/tls/stream.rs::FakeTlsStream::new()`:
- Шлёт obfs-init **отдельным** Application-record (`[0x17 0x03 0x03 0x00 0x40] + frame`).
- Затем CCS отдельной записью.
- Затем MTProto через framing.

**Это рассинхронизирует Telegram-сервер**, который (по tdesktop) ожидает init как prefix первого
MTProto-record, а не отдельным record'ом.

## Задача — реализовать tdesktop-prefix-модель

### Изменить `FakeTlsStream::new()` в `stream.rs`

После handshake (ClientHello→ServerHello→HMAC verify):
1. **НЕ посылать** init отдельным record и **НЕ посылать** CCS в `new()`.
2. Сохранить `frame` (64-байтный obfs-init из `client_handshake`) как **pending prefix** в структуре
   `FakeTlsStream` (новое поле `first_prefix: Option<[u8;64]>`, или `Option<Vec<u8>>`).
3. Сохранить send/recv ciphers как сейчас.

### Изменить `FakeTlsStream::poll_write` (через framing)

В `poll_write(buf)`:
- **Первый** вызов (когда `first_prefix` есть): построить payload = `prefix(64) || buf[..chunk]`,
  где chunk учитывает `kClientPartSize` (2878) — но для первого record'а полезная часть =
  `2878 - 64`. Перед отправкой первого record'а послать CCS-record `[0x14 0x03 0x03 0x00 0x01 0x01]`
  в сокет (сырыми байтами через framing.inner_mut(), как раньше CCS шёл). Затем Application-record с
  payload `prefix||chunk` (framing сам добавит `0x17 0x03 0x03 <len>`). Очистить `first_prefix`.
- Последующие `poll_write`: обычный record (payload = buf[..chunk], framing добавит заголовок).

Это значит framing's `poll_write` должен уметь принять payload, который **уже включает prefix**.
Проще всего: в `FakeTlsStream::poll_write` собрать `payload = prefix_iter().chain(buf[..chunk])`
в один `Vec`, отдать framing. framing оборачивает в record как обычно.

### obfs2-шифрование

Внимание: в alksev **obfs2 уже шифрует MTProto** (это правильно и байт-идентично gotd, R8/R10 — НЕ
трогай этот crypto). Но prefix (init) шифровать obfs2 **НЕ надо** — он уже зашифрован внутри
`client_handshake` (init[56:64] encrypted, init[0:56] plain, R8/R10).

Значит в `poll_write` для первого record'а:
- payload = `prefix(64, уже готовый из client_handshake)` || `obfs2_send.apply_keystream(buf[..chunk])`.
- send_cipher применяется ТОЛЬКО к MTProto-части (chunk), не к prefix. (prefix уже «прошёл» cipher
  при генерации в client_handshake — там cipher сдвинут на 64. Если ты сейчас применяешь
  send_cipher ко всему buf — оставь как есть, просто prefix дописывай ПЕРЕД obfs2-обработкой buf.)

### Модель чтения (poll_read) — НЕ менять

На чтение framing strip'ит TLS-record заголовки как обычно; obfs2_recv расшифровывает. Это уже
работает (R12). НЕ трогать raw-mode и read-логику.

## Что НЕ трогать
- `client_handshake` в `obfuscator.rs` (crypto корректен).
- `client_hello.rs`, `server_hello.rs`, `record.rs`.
- `parse_secret`, `net/tcp.rs` routing.
- raw-mode НЕ возвращать.

## Проверка (Claude запустит)
```bash
cargo test -p grammers-mtproto --features mtproxy
# живой тест:
RUST_LOG=warn MTPROXY_HOST=argeiphontes.ru MTPROXY_PORT=16000 \
  MTPROXY_SECRET=ee7b184b3f7c1ace06fa2efbbaa851f1a8617267656970686f6e7465732e7275 MTPROXY_DC_ID=2 \
  cargo run --release --example mtproxy --features "mtproxy grammers-session/sqlite-storage" -p grammers-client
```
Цель: `Ping result: Pong(...)` — впервые получить ответ от Telegram через faketls.
Simple `13370` не сломать. НЕ коммить, живые тесты не запускать.
